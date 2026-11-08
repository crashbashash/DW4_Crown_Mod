//! The application entry point: inspect the regional disc, transform the enemy
//! tables, rebuild the linked crown resources, stream a separate ISO and verify
//! it before the final rename. Input is always read-only and mmap-backed.

use crate::container::{self, Format};
use crate::disc::{
    count_copies, inspect_bytes, inspect_retail_planned_for_plan_with_progress, DiscReport,
};
use crate::elf::{difficulty_region, ElfInfo};
use crate::error::{Error, Result};
use crate::layout::Layout;
use crate::plan::{transform, PatchPlan, PlanSummary};
use crate::ratio::RatioVector;
use crate::region::WriteRegion;
use crate::serial::serial_regions;
use crate::table::EnemyTable;
use crate::writer::{
    canonical_output, md5_hex_with_progress, part_path, write_output_extended, Extension, Phase,
    Progress, WriteOptions,
};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

/// The three blocks, in the order of the [`Layout`] offset fields: HP first,
/// then the twelve stats, then chargen. Mirrors [`crate::disc`]'s names so the
/// refusals name the same block the inspection did.
const BLOCK_NAMES: [&str; 3] = ["HPMAX", "stats", "chargen"];

/// Controls whether an existing output may be replaced.
#[derive(PartialEq, Debug, Clone)]
pub struct PatchOptions {
    pub overwrite: bool,
}

/// The result of a successful patch: the final file's MD5, its length and the
/// work the transform reported.
#[derive(PartialEq, Debug, Clone)]
pub struct PatchOutcome {
    pub md5: String,
    pub bytes: u64,
    pub summary: PlanSummary,
}

/// Applies `plan` to the disc's authored table and returns the transformed
/// table with the transform's summary.
fn derive_patch(report: &DiscReport, plan: &PatchPlan) -> (EnemyTable, PlanSummary) {
    let ratio = RatioVector::derive(&report.authored);
    transform(&report.authored, plan, &ratio)
}

/// Turns an inspected disc into the write regions a patch applies: one region
/// per authored block copy (carrying the computed replacement bytes), plus the
/// ELF difficulty word when `plan.force_very_hard` is set and the two serial
/// rewrites when `plan.serial` is set.
///
/// The authored block copies are located by searching `iso` for the authored
/// blocks, so the retail disc's scattered copies are all found without
/// hardcoding their offsets. A block that does not occur exactly
/// [`Layout::expected_copies`] times is [`Error::WrongRevision`].
pub fn plan_regions(
    iso: &[u8],
    report: &DiscReport,
    plan: &PatchPlan,
    layout: &Layout,
) -> Result<(Vec<WriteRegion>, PlanSummary)> {
    let (new_table, summary) = derive_patch(report, plan);
    let new_blocks = new_table.blocks();
    let old_blocks = report.authored.blocks();

    let mut regions = Vec::new();
    for (index, (new_block, old_block)) in new_blocks.iter().zip(old_blocks.iter()).enumerate() {
        let occurrences: Vec<u64> = memchr::memmem::find_iter(iso, old_block)
            .map(|offset| offset as u64)
            .collect();
        if occurrences.len() != layout.expected_copies {
            return Err(Error::WrongRevision {
                block: BLOCK_NAMES[index],
                found: occurrences.len(),
                expected: layout.expected_copies,
            });
        }
        for offset in occurrences {
            regions.push(WriteRegion {
                offset,
                bytes: new_block.clone(),
            });
        }
    }

    if plan.random_crown_colours {
        regions.extend(crate::crown_colours::regions(
            iso,
            report,
            plan.crown_colour_seed.ok_or_else(|| {
                Error::CrownData("colour seed must be resolved before planning".into())
            })?,
        )?);
    }

    let gamecube = report.release.is_some_and(|release| release.is_gamecube());
    if (plan.force_very_hard || plan.serial.is_some()) && gamecube {
        return Err(Error::UnsupportedDifficulty);
    }
    if plan.force_very_hard {
        if matches!(
            report.release,
            Some(crate::release::Release::Europe | crate::release::Release::Japan)
        ) {
            return Err(Error::UnsupportedDifficulty);
        }
        let boot_elf = report.boot_elf.as_ref().ok_or(Error::ElfNotFound)?;
        let elf = ElfInfo::read(iso, boot_elf)?;
        regions.push(difficulty_region(iso, &elf)?);
    }

    if let Some(serial) = &plan.serial {
        let boot_elf = report.boot_elf.as_ref().ok_or(Error::FileNotFound {
            name: "boot ELF".to_string(),
        })?;
        regions.extend(serial_regions(iso, boot_elf, serial)?);
    }

    Ok((regions, summary))
}

/// The app's entry point: patches `input` into `output` against the retail
/// layout detected from its boot configuration. Progress counters represent
/// estimated overall work out of 10,000, including container round trips.
pub fn patch_file(
    input: &Path,
    output: &Path,
    plan: &PatchPlan,
    opts: &PatchOptions,
    progress: &mut dyn FnMut(Progress),
) -> Result<PatchOutcome> {
    if plan.crown_drops && !plan.crown_mod {
        return Err(Error::CrownData("Crown Drops requires Crown Mod.".into()));
    }
    let mut resolved = plan.clone();
    if plan.random_crown_colours && plan.crown_colour_seed.is_none() {
        let mut seed = [0u8; 4];
        getrandom::fill(&mut seed)
            .map_err(|e| Error::CrownData(format!("could not seed crown colours: {e}")))?;
        resolved.crown_colour_seed = Some(u32::from_le_bytes(seed));
    }
    if plan.random_enemies && plan.enemy_seed.is_none() {
        let mut seed = [0u8; 4];
        getrandom::fill(&mut seed)
            .map_err(|e| Error::CrownData(format!("could not seed enemies: {e}")))?;
        resolved.enemy_seed = Some(u32::from_le_bytes(seed));
    }
    let plan = &resolved;
    match container::detect(input)? {
        Format::Raw => patch_file_internal(input, output, plan, opts, None, progress),
        format => patch_container(format, input, output, plan, opts, progress),
    }
}

/// Patches a CISO or RVZ: unpack to a plain image next to the output, run the
/// normal pipeline on it, pack the result in the same format and prove the
/// packed file unpacks to exactly the patched image before it is renamed.
fn patch_container(
    format: Format,
    input: &Path,
    output: &Path,
    plan: &PatchPlan,
    opts: &PatchOptions,
    progress: &mut dyn FnMut(Progress),
) -> Result<PatchOutcome> {
    let input_canon = std::fs::canonicalize(input)?;
    let output_canon = canonical_output(output)?;
    if input_canon == output_canon {
        return Err(Error::OutputIsInput);
    }
    if !opts.overwrite && output.try_exists()? {
        return Err(Error::OutputExists);
    }
    let parent = output_canon.parent().unwrap_or_else(|| Path::new("."));
    let work = tempfile::Builder::new()
        .prefix(".dw4vhp-")
        .tempdir_in(parent)?;
    let (plain_in, plain_out) = (work.path().join("in.iso"), work.path().join("out.iso"));
    let part = part_path(output);
    let result = (|| {
        progress(Progress {
            phase: Phase::Copying,
            done: 0,
            total: 10000,
        });
        container::unpack(format, input, &plain_in, &mut |d, t| {
            progress(scaled(Phase::Copying, d, t, 0, 1500))
        })?;
        let inner = PatchOptions { overwrite: true };
        let outcome = patch_file_internal(&plain_in, &plain_out, plan, &inner, None, &mut |p| {
            progress(scaled(p.phase, p.done, p.total, 1500, 6500))
        })?;
        std::fs::remove_file(&plain_in)?;
        container::pack(format, &plain_out, &part, &mut |d, t| {
            progress(scaled(Phase::Writing, d, t, 6500, 8500))
        })?;

        // Round trip: the packed file must expand to the image that was patched.
        let check = work.path().join("check.iso");
        container::unpack(format, &part, &check, &mut |d, t| {
            progress(scaled(Phase::Verifying, d, t, 8500, 9200))
        })?;
        if !same_image(&plain_out, &check, &mut |d, t| {
            progress(scaled(Phase::Verifying, d, t, 9200, 9800))
        })? {
            return Err(Error::VerificationFailed(
                "the packed disc does not match the patched image".into(),
            ));
        }
        std::fs::rename(&part, output)?;
        Ok(PatchOutcome {
            md5: md5_hex_with_progress(output, &mut |d, t| {
                progress(scaled(Phase::Hashing, d, t, 9800, 10000))
            })?,
            bytes: std::fs::metadata(output)?.len(),
            summary: outcome.summary,
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// The explicit-layout seam used by generated tests and low-level callers. `patch_file`
/// uses the same table/writer checks. This seam deliberately omits archive
/// expansion for generated fixtures; retail/application callers use `patch_file`.
/// Progress uses the same overall-work counters as `patch_file`.
pub fn patch_file_with_layout(
    input: &Path,
    output: &Path,
    plan: &PatchPlan,
    opts: &PatchOptions,
    layout: &Layout,
    progress: &mut dyn FnMut(Progress),
) -> Result<PatchOutcome> {
    patch_file_internal(input, output, plan, opts, Some(layout), progress)
}

fn patch_file_internal(
    input: &Path,
    output: &Path,
    plan: &PatchPlan,
    opts: &PatchOptions,
    layout: Option<&Layout>,
    progress: &mut dyn FnMut(Progress),
) -> Result<PatchOutcome> {
    progress(Progress {
        phase: Phase::Inspecting,
        done: 0,
        total: 10000,
    });
    if layout.is_some()
        && (plan.four_player_spawns
            || plan.crown_drops
            || plan.random_crown_colours
            || plan.random_enemies)
    {
        return Err(Error::CrownData(
            "optional enemy resources require complete retail archives".into(),
        ));
    }
    let file = File::open(input)?;
    if file.metadata()?.len() == 0 {
        return Err(Error::NotThisDisc);
    }
    // Safety: the mapping is read-only and `input` is never written.
    let mmap = unsafe { Mmap::map(&file)? };

    let (report, crowns) = match layout {
        Some(layout) => (inspect_bytes(&mmap[..], layout)?, None),
        None => {
            let (report, crowns) =
                inspect_retail_planned_for_plan_with_progress(&mmap, plan, &mut |d, t| {
                    progress(scaled(Phase::Inspecting, d, t, 0, 1000))
                })?;
            (report, Some(crowns))
        }
    };
    let detected_layout = report
        .release
        .map(|release| release.layout_in(&mmap[..]))
        .transpose()?;
    let layout = layout
        .or(detected_layout.as_ref())
        .ok_or(Error::NotThisDisc)?;
    let (new_table, _) = derive_patch(&report, plan);
    let blocks = new_table.blocks();
    let (mut regions, summary) = plan_regions(&mmap[..], &report, plan, layout)?;
    let reward_tables = crowns
        .as_ref()
        .map(|c| c.rewards.apply(&mut regions, &new_table))
        .transpose()?
        .unwrap_or_default();

    if let Some(crowns) = &crowns {
        regions.extend(crowns.regions.clone());
    }
    let write_opts = WriteOptions {
        overwrite: opts.overwrite,
    };
    let verify = |path: &Path| {
        verify_output(path, &blocks, layout, &reward_tables)?;
        if let Some(crowns) = &crowns {
            let file = File::open(path)?;
            // Safety: the completed private output is read-only during verification.
            let out = unsafe { Mmap::map(&file)? };
            crowns.verify(&mmap, &out)?;
        }
        Ok(())
    };
    let append = |file: &mut File, progress: &mut dyn FnMut(Progress)| {
        crowns
            .as_ref()
            .expect("extension present")
            .append(&mmap, file, &mut |done, total| {
                progress(Progress {
                    phase: Phase::Writing,
                    done,
                    total,
                })
            })
    };
    let extension = crowns.as_ref().map(|c| Extension {
        output_size: c.output_size,
        append: &append as _,
    });
    let outcome = write_output_extended(
        input,
        output,
        &regions,
        &write_opts,
        &mut |p| {
            let (start, end) = match p.phase {
                Phase::Inspecting => (1000, 1000),
                Phase::Copying => (1000, 4000),
                Phase::Extending => (4000, 6500),
                Phase::Writing => (6500, 7000),
                Phase::Verifying => (7000, 9000),
                Phase::Hashing => (9000, 10000),
            };
            progress(scaled(p.phase, p.done, p.total, start, end));
        },
        &verify,
        extension,
    )?;
    Ok(PatchOutcome {
        md5: outcome.md5,
        bytes: outcome.bytes,
        summary,
    })
}

/// Re-reads the written file and confirms the computed table blocks are exactly
/// what landed: the anchors hold them byte-for-byte and each occurs
/// [`Layout::expected_copies`] times.
fn verify_output(
    path: &Path,
    blocks: &[Vec<u8>; 3],
    layout: &Layout,
    rewards: &[WriteRegion],
) -> Result<()> {
    let file = File::open(path)?;
    // Safety: the mapping is read-only and `path` is not modified while mapped.
    let mmap = unsafe { Mmap::map(&file)? };
    verify_blocks(&mmap[..], blocks, layout, rewards)
}

/// The block-level checks [`verify_output`] applies against an image in memory.
fn verify_blocks(
    iso: &[u8],
    blocks: &[Vec<u8>; 3],
    layout: &Layout,
    rewards: &[WriteRegion],
) -> Result<()> {
    let mut offsets = std::collections::BTreeSet::new();
    for reward in rewards {
        if !offsets.insert(reward.offset)
            || !blocks
                .iter()
                .any(|b| b.len() == reward.bytes.len() && *b != reward.bytes)
        {
            return Err(Error::VerificationFailed(
                "invalid local reward table override".into(),
            ));
        }
        let start = usize::try_from(reward.offset)
            .map_err(|_| Error::VerificationFailed("invalid reward offset".into()))?;
        if iso.get(start..start.saturating_add(reward.bytes.len())) != Some(reward.bytes.as_slice())
        {
            return Err(Error::VerificationFailed(format!(
                "local reward table at {start:#X} differs from plan"
            )));
        }
    }
    let anchors = [layout.hp_off, layout.stat_off, layout.byte_off];
    for (index, (block, &anchor)) in blocks.iter().zip(anchors.iter()).enumerate() {
        let block = rewards
            .iter()
            .find(|r| r.offset == anchor)
            .map(|r| &r.bytes)
            .unwrap_or(block);
        let name = BLOCK_NAMES[index];
        let start = usize::try_from(anchor).map_err(|_| {
            Error::VerificationFailed(format!("the {name} block offset {anchor:#X} is too large"))
        })?;
        let end = start.checked_add(block.len()).ok_or_else(|| {
            Error::VerificationFailed(format!(
                "the {name} block at {anchor:#X} overflows the file offset range"
            ))
        })?;
        let actual = iso.get(start..end).ok_or_else(|| {
            Error::VerificationFailed(format!(
                "the {name} block at {anchor:#X} runs past the end of the written file"
            ))
        })?;
        if actual != block.as_slice() {
            return Err(Error::VerificationFailed(format!(
                "the {name} block at {anchor:#X} does not match the computed patch"
            )));
        }
    }

    for (index, block) in blocks.iter().enumerate() {
        let name = BLOCK_NAMES[index];
        let found = count_copies(iso, block);
        let expected = layout
            .expected_copies
            .checked_sub(
                rewards
                    .iter()
                    .filter(|r| r.bytes.len() == block.len())
                    .count(),
            )
            .ok_or_else(|| Error::VerificationFailed("too many local reward tables".into()))?;
        if found != expected {
            return Err(Error::VerificationFailed(format!(
                "found {found} copies of the {name} block in the written file, expected {}",
                expected
            )));
        }
    }

    Ok(())
}

/// True when `unpacked` holds exactly the bytes of `patched`, followed by
/// nothing but zeros (CISO pads its last block).
pub(crate) fn same_image(
    patched: &Path,
    unpacked: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<bool> {
    use std::io::Read;
    let (mut a, mut b) = (File::open(patched)?, File::open(unpacked)?);
    let total = b.metadata()?.len();
    let mut done = 0;
    progress(done, total);
    if b.metadata()?.len() < a.metadata()?.len() {
        return Ok(false);
    }
    let (mut left, mut right) = (vec![0u8; 8 << 20], vec![0u8; 8 << 20]);
    loop {
        let n = a.read(&mut left)?;
        if n == 0 {
            break;
        }
        b.read_exact(&mut right[..n])?;
        if left[..n] != right[..n] {
            return Ok(false);
        }
        done += n as u64;
        progress(done, total);
    }
    loop {
        let n = b.read(&mut right)?;
        if n == 0 {
            return Ok(true);
        }
        if right[..n].iter().any(|&byte| byte != 0) {
            return Ok(false);
        }
        done += n as u64;
        progress(done, total);
    }
}

// Fixed work ranges keep repeated low-level phases from resetting overall
// progress. These weights estimate work, rather than promising elapsed time.
fn scaled(phase: Phase, done: u64, total: u64, start: u64, end: u64) -> Progress {
    let fraction = if total == 0 {
        0
    } else {
        ((end - start) as u128 * done.min(total) as u128 / total as u128) as u64
    };
    Progress {
        phase,
        done: start + fraction,
        total: 10000,
    }
}

#[cfg(test)]
mod reward_verification_tests {
    use super::*;

    #[test]
    fn local_rewards_verify_at_the_anchor_and_other_copies_and_reject_tampering() {
        let blocks = [vec![0x11; 8], vec![0x22; 48], vec![0x33; 6]];
        let layout = Layout {
            hp_off: 0,
            stat_off: 8,
            byte_off: 56,
            rows: 2,
            expected_copies: 2,
        };
        for (index, relative) in [0, 8, 56].into_iter().enumerate() {
            for copy in [0, 64] {
                let changed = copy + relative;
                let mut image = vec![0xff; 128];
                for offset in [0, 64] {
                    image[offset..offset + 8].copy_from_slice(&blocks[0]);
                    image[offset + 8..offset + 56].copy_from_slice(&blocks[1]);
                    image[offset + 56..offset + 62].copy_from_slice(&blocks[2]);
                }
                let mut reward = WriteRegion {
                    offset: changed as u64,
                    bytes: blocks[index].clone(),
                };
                reward.bytes[0] ^= 1;
                image[changed..changed + reward.bytes.len()].copy_from_slice(&reward.bytes);
                let overrides = [reward];
                verify_blocks(&image, &blocks, &layout, &overrides).unwrap();
                for at in [
                    changed,
                    changed + blocks[index].len() - 1,
                    64 - copy + relative,
                ] {
                    let mut damaged = image.clone();
                    damaged[at] ^= 1;
                    assert!(verify_blocks(&damaged, &blocks, &layout, &overrides).is_err());
                }
                let other = 64 - copy + relative;
                image[other..other + blocks[index].len()].fill(0xaa);
                assert!(
                    verify_blocks(&image, &blocks, &layout, &overrides).is_err(),
                    "an unmodified table must still be verified"
                );
            }
        }
    }
}
