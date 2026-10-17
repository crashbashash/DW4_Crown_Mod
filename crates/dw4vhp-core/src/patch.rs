//! The application entry point: inspect the regional disc, transform the enemy
//! tables, rebuild the linked crown resources, stream a separate ISO and verify
//! it before the final rename. Input is always read-only and mmap-backed.

use crate::container::{self, Format};
use crate::disc::{count_copies, inspect_bytes, inspect_retail_planned, DiscReport};
use crate::elf::{difficulty_region, ElfInfo};
use crate::error::{Error, Result};
use crate::layout::Layout;
use crate::plan::{transform, PatchPlan, PlanSummary};
use crate::ratio::RatioVector;
use crate::region::WriteRegion;
use crate::serial::serial_regions;
use crate::table::EnemyTable;
use crate::writer::{
    canonical_output, md5_hex, part_path, write_output_extended, Extension, Phase, Progress,
    WriteOptions,
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
/// layout detected from its boot configuration.
pub fn patch_file(
    input: &Path,
    output: &Path,
    plan: &PatchPlan,
    opts: &PatchOptions,
    progress: &mut dyn FnMut(Progress),
) -> Result<PatchOutcome> {
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
        let step = |phase| move |done: u64, total: u64| Progress { phase, done, total };
        let unpacking = step(Phase::Copying);
        container::unpack(format, input, &plain_in, &mut |d, t| {
            progress(unpacking(d, t))
        })?;
        let inner = PatchOptions { overwrite: true };
        let outcome = patch_file_internal(&plain_in, &plain_out, plan, &inner, None, progress)?;
        std::fs::remove_file(&plain_in)?;
        let packing = step(Phase::Writing);
        container::pack(format, &plain_out, &part, &mut |d, t| {
            progress(packing(d, t))
        })?;

        // Round trip: the packed file must expand to the image that was patched.
        let check = work.path().join("check.iso");
        let verifying = step(Phase::Verifying);
        container::unpack(format, &part, &check, &mut |d, t| progress(verifying(d, t)))?;
        if !same_image(&plain_out, &check)? {
            return Err(Error::VerificationFailed(
                "the packed disc does not match the patched image".into(),
            ));
        }
        std::fs::rename(&part, output)?;
        Ok(PatchOutcome {
            md5: md5_hex(output)?,
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
    let file = File::open(input)?;
    if file.metadata()?.len() == 0 {
        return Err(Error::NotThisDisc);
    }
    // Safety: the mapping is read-only and `input` is never written.
    let mmap = unsafe { Mmap::map(&file)? };

    let (report, crowns) = match layout {
        Some(layout) => (inspect_bytes(&mmap[..], layout)?, None),
        None => {
            let (report, crowns) = inspect_retail_planned(&mmap)?;
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

    if let Some(crowns) = &crowns {
        regions.extend(crowns.regions.clone());
    }
    let write_opts = WriteOptions {
        overwrite: opts.overwrite,
    };
    let verify = |path: &Path| {
        verify_output(path, &blocks, layout)?;
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
        progress,
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
fn verify_output(path: &Path, blocks: &[Vec<u8>; 3], layout: &Layout) -> Result<()> {
    let file = File::open(path)?;
    // Safety: the mapping is read-only and `path` is not modified while mapped.
    let mmap = unsafe { Mmap::map(&file)? };
    verify_blocks(&mmap[..], blocks, layout)
}

/// The block-level checks [`verify_output`] applies against an image in memory.
fn verify_blocks(iso: &[u8], blocks: &[Vec<u8>; 3], layout: &Layout) -> Result<()> {
    let anchors = [layout.hp_off, layout.stat_off, layout.byte_off];
    for (index, (block, &anchor)) in blocks.iter().zip(anchors.iter()).enumerate() {
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
        if found != layout.expected_copies {
            return Err(Error::VerificationFailed(format!(
                "found {found} copies of the {name} block in the written file, expected {}",
                layout.expected_copies
            )));
        }
    }

    Ok(())
}

/// True when `unpacked` holds exactly the bytes of `patched`, followed by
/// nothing but zeros (CISO pads its last block).
fn same_image(patched: &Path, unpacked: &Path) -> Result<bool> {
    use std::io::Read;
    let (mut a, mut b) = (File::open(patched)?, File::open(unpacked)?);
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
    }
    loop {
        let n = b.read(&mut right)?;
        if n == 0 {
            return Ok(true);
        }
        if right[..n].iter().any(|&byte| byte != 0) {
            return Ok(false);
        }
    }
}
