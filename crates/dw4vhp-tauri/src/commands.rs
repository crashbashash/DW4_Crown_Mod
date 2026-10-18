//! The IPC surface: every command the frontend can invoke, and the three
//! patch events it listens to. Engine calls never run on the main thread:
//! Tauri runs synchronous commands on the main thread, so [`analyze`],
//! [`plan_summary`] and [`start_patch`] are `async` and hand their work to
//! `spawn_blocking`. Progress and results come back through events.
//!
//! The shell maps inspection failures to actionable disc notices and preserves
//! the output-name rule. A structured reason identifies already-modded discs
//! without asking the frontend to interpret error-message text.

use crate::state::AppState;
use dw4vhp_core::disc::{inspect_retail_with_options, DiscReport};
use dw4vhp_core::error::Error;
use dw4vhp_core::patch::{patch_file, PatchOptions};
use dw4vhp_core::plan::{transform, PatchPlan, PlanSummary};
use dw4vhp_core::ratio::RatioVector;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

/// The marker a patched image's name carries (old `form::OUTPUT_SUFFIX`).
const OUTPUT_MARKER: &str = " [CrownMod]";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiscStatusPayload {
    kind: &'static str,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    size: u64,
    output_size: u64,
    copies: Option<[usize; 3]>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummaryPayload {
    rows_changed: usize,
    live_rows: usize,
    attack_pinned: usize,
    hp_capped: usize,
    crit_pinned: usize,
    practice_rows: usize,
    collapsed_rows: usize,
    elf_step: bool,
    serial_step: bool,
}

impl PlanSummaryPayload {
    fn of(summary: &PlanSummary) -> Self {
        Self {
            rows_changed: summary.rows_changed,
            live_rows: summary.live_rows,
            attack_pinned: summary.attack_pinned,
            hp_capped: summary.hp_capped,
            crit_pinned: summary.crit_pinned,
            practice_rows: summary.practice_rows,
            collapsed_rows: summary.collapsed_rows,
            elf_step: summary.elf_step,
            serial_step: summary.serial_step,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PatchDonePayload {
    md5: String,
    bytes: u64,
    summary: PlanSummaryPayload,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    phase: &'static str,
    done: u64,
    total: u64,
}

/// The native open dialog for the player's disc image. `None` when cancelled.
///
/// Async on purpose: a synchronous command runs on the main thread, and the
/// blocking dialog calls wait for that same thread to draw the dialog, so the
/// window froze and the dialog never appeared.
#[tauri::command]
pub async fn choose_iso(window: tauri::Window) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("PS2 or GameCube disc image", &["iso", "gcm", "ciso", "rvz"])
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.display().to_string())
    })
    .await
    .ok()
    .flatten()
}

/// The native save dialog for the patched output. `None` when cancelled.
#[tauri::command]
pub async fn choose_output(window: tauri::Window, default_name: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_parent(&window)
            .set_file_name(&default_name)
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.display().to_string())
    })
    .await
    .ok()
    .flatten()
}

/// The output name an input suggests: same directory, stem plus the marker
/// (old `form::default_output_path`), so the default never targets the input.
#[tauri::command]
pub fn default_output(input: String) -> String {
    let input = std::path::Path::new(&input);
    let mut name = input
        .file_stem()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(OUTPUT_MARKER);
    // CISO and RVZ inputs stay in their own format; everything else is a plain .iso.
    let extension = match dw4vhp_core::container::detect(input) {
        Ok(format) => format.extension(),
        Err(_) => "iso",
    };
    name.push(format!(".{extension}"));
    input.with_file_name(name).display().to_string()
}

/// Checks whether patching needs the user's overwrite confirmation.
#[tauri::command]
pub fn output_exists(output: String) -> Result<bool, String> {
    std::path::Path::new(&output)
        .try_exists()
        .map_err(|e| e.to_string())
}

/// One inspection verdict: successful disc metadata or an actionable refusal.
/// The structured reason selects the already-modded popup title.
fn verdict(result: &Result<DiscReport, Error>) -> DiscStatusPayload {
    match result {
        Ok(report) => DiscStatusPayload {
            kind: "ok",
            text: format!(
                "clean {} {}: {} HPMAX, {} stats and {} chargen copies",
                report.release.expect("retail report").label(),
                report.release.expect("retail report").serial(),
                report.copies[0],
                report.copies[1],
                report.copies[2]
            ),
            reason: None,
            size: report.size,
            output_size: report.output_size,
            copies: Some(report.copies),
        },
        Err(error) => DiscStatusPayload {
            kind: match error {
                Error::AlreadyModded { .. }
                | Error::WrongRevision { .. }
                | Error::NotThisDisc
                | Error::CrownData(_) => "refused",
                _ => "unknown",
            },
            reason: matches!(error, Error::AlreadyModded { .. }).then_some("already-modded"),
            text: match error {
                Error::AlreadyModded { .. } => "This disc is already modded. Choose a clean, original disc image.".into(),
                Error::NotThisDisc | Error::FileNotFound { .. } | Error::ElfNotFound => "This file is not a supported disc image, or it is incomplete. Choose a clean Digimon World 4 or Digimon World X disc from a supported release.".into(),
                Error::WrongRevision { .. } | Error::CrownData(_) => "This disc image is not a supported original revision. Choose a clean, original copy of a supported release.".into(),
                _ => error.to_string(),
            },
            size: 0,
            output_size: 0,
            copies: None,
        },
    }
}

/// Inspects the disc off the main thread, stores a clean report for later
/// [`plan_summary`] and [`start_patch`] calls, and returns the verdict.
#[tauri::command]
pub async fn analyze(
    path: String,
    four_player_spawns: Option<bool>,
    crown_drops: Option<bool>,
    app: AppHandle,
) -> Result<DiscStatusPayload, String> {
    let epoch = app
        .state::<AppState>()
        .analysis_epoch
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    // The path is needed both inside the worker (inspect) and after it (the
    // input stored for the patch), so the worker gets a clone.
    let inspect_path = std::path::PathBuf::from(&path);
    let result = tauri::async_runtime::spawn_blocking(move || {
        inspect_retail_with_options(
            &inspect_path,
            four_player_spawns.unwrap_or(false),
            crown_drops.unwrap_or(false),
        )
    })
    .await
    .map_err(|e| format!("analyze worker failed: {e}"))?;

    let payload = verdict(&result);
    let state = app.state::<AppState>();
    let mut input = state.input.lock().expect("input lock");
    let mut report = state.report.lock().expect("report lock");
    // Rapid option/input changes can finish out of order. An older inspection
    // must never replace the input associated with the current GUI verdict.
    if state.analysis_epoch.load(Ordering::SeqCst) != epoch {
        return Ok(payload);
    }
    match result {
        Ok(clean) => {
            *input = Some(std::path::PathBuf::from(&path));
            *report = Some(clean);
        }
        Err(_) => {
            *input = None;
            *report = None;
        }
    }
    Ok(payload)
}

/// The base-mode summary, using the disc table stored by [`analyze`].
#[tauri::command]
pub async fn plan_summary(app: AppHandle) -> Result<PlanSummaryPayload, String> {
    let plan = PatchPlan::default();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let report = state.report.lock().expect("report lock");
        let report = report.as_ref().ok_or_else(|| {
            "choose a disc first — the summary needs the analysed enemy table".to_string()
        })?;
        let ratio = RatioVector::derive(&report.authored);
        let (_, summary) = transform(&report.authored, &plan, &ratio);
        Ok(PlanSummaryPayload::of(&summary))
    })
    .await
    .map_err(|e| format!("summary worker failed: {e}"))?
}

/// Runs the whole patch on a blocking thread: copy, write regions, verify,
/// atomic rename. Progress streams as `patch-progress` events; exactly one
/// terminal event follows — `patch-done` with the verified outcome, or
/// `patch-failed` with the engine's fix-naming message.
///
/// Returns `Err` synchronously only when no disc has been analysed yet.
#[tauri::command]
pub async fn start_patch(
    output: String,
    overwrite: bool,
    four_player_spawns: Option<bool>,
    crown_drops: Option<bool>,
    random_crown_colours: Option<bool>,
    app: AppHandle,
) -> Result<(), String> {
    let permit = app.state::<AppState>().begin_operation()?;
    let plan = PatchPlan {
        four_player_spawns: four_player_spawns.unwrap_or(false),
        crown_drops: crown_drops.unwrap_or(false),
        random_crown_colours: random_crown_colours.unwrap_or(false),
        ..PatchPlan::default()
    };
    let input = {
        let state = app.state::<AppState>();
        let stored = state.input.lock().expect("input lock");
        stored.clone().ok_or_else(|| {
            "choose a disc first — nothing to patch until a disc is analysed".to_string()
        })?
    };
    let output = std::path::PathBuf::from(output);
    let opts = PatchOptions { overwrite };

    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let mut progress = |p: dw4vhp_core::writer::Progress| {
            // The window may be closed mid-patch; a dropped listener is not
            // worth stopping the write for.
            let _ = app.emit(
                "patch-progress",
                ProgressPayload {
                    phase: phase_name(p.phase),
                    done: p.done,
                    total: p.total,
                },
            );
        };
        let outcome = patch_file(&input, &output, &plan, &opts, &mut progress);
        match outcome {
            Ok(outcome) => {
                let _ = app.emit(
                    "patch-done",
                    PatchDonePayload {
                        md5: outcome.md5,
                        bytes: outcome.bytes,
                        summary: PlanSummaryPayload::of(&outcome.summary),
                    },
                );
            }
            Err(error) => {
                let _ = app.emit("patch-failed", error.to_string());
            }
        }
    })
    .await
    .map_err(|e| format!("patch worker failed: {e}"))?;
    Ok(())
}

fn phase_name(phase: dw4vhp_core::writer::Phase) -> &'static str {
    match phase {
        dw4vhp_core::writer::Phase::Inspecting => "inspecting",
        dw4vhp_core::writer::Phase::Copying => "copying",
        dw4vhp_core::writer::Phase::Writing => "writing",
        dw4vhp_core::writer::Phase::Extending => "writing",
        dw4vhp_core::writer::Phase::Verifying => "verifying",
        dw4vhp_core::writer::Phase::Hashing => "verifying",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn already_modded_verdict_is_structured_and_explains_how_to_continue() {
        let result = verdict(&Err(Error::AlreadyModded {
            rarity_nonzero: 600,
        }));
        assert_eq!(result.kind, "refused");
        assert_eq!(result.reason, Some("already-modded"));
        assert!(result.text.contains("already modded") && result.text.contains("original"));
        assert!(!result.text.contains("600"));
    }
    #[test]
    fn invalid_disc_message_covers_both_supported_consoles() {
        for error in [
            Error::NotThisDisc,
            Error::FileNotFound {
                name: "SYSTEM.CNF".into(),
            },
        ] {
            let result = verdict(&Err(error));
            assert!(result.reason.is_none());
            assert!(result.text.contains("supported disc image"));
            assert!(!result.text.contains("PS2"));
        }
    }
}
