use crate::error::{Error, Result};
use crate::iso9660::{self, IsoFile};
use crate::layout::Layout;
use crate::release::{self, Release};
use crate::table::EnemyTable;
use md5::{Digest, Md5};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

/// The boot ELF's ISO9660 name, without its `;1` version suffix.
const BOOT_ELF_NAME: &str = "SLUS_208.36";

/// The boot configuration file's ISO9660 name, without its `;1` version suffix.
const CNF_NAME: &str = "SYSTEM.CNF";

/// Counts the non-overlapping occurrences of `needle` in `iso`.
pub fn count_copies(iso: &[u8], needle: &[u8]) -> usize {
    memchr::memmem::find_iter(iso, needle).count()
}

/// The three blocks, in the order of the [`Layout`] offset fields: HP first,
/// then the twelve stats, then chargen.
const BLOCK_NAMES: [&str; 3] = ["HPMAX", "stats", "chargen"];

/// Authored discs crown 60 rows and patched discs crown 638, so more than this
/// many crowned rows can only mean the disc was patched already.
const ALREADY_MODDED_ROWS: usize = 100;

/// What [`inspect`] found on a disc: its size, how many copies of each authored
/// block it carries, how many rows already carry a crown, the table itself, and
/// the two files the later steps need.
///
/// A report only exists for a disc that passed every check, so `already_modded`
/// is always `false` here; the field is kept because the GUI shows it.
///
/// `boot_elf` and `cnf` are looked up best-effort: an image with no volume
/// descriptor, or one that simply carries neither file, reports `None` and still
/// inspects `Ok`. The crown data patch needs no ELF, so the disc is only refused
/// for a missing boot ELF when a plan actually forces Very Hard, at plan time.
#[derive(PartialEq, Debug, Clone)]
pub struct DiscReport {
    pub size: u64,
    /// Present for auto-detected retail discs; explicit-layout fixtures use None.
    pub release: Option<Release>,
    pub copies: [usize; 3],
    pub rarity_nonzero: usize,
    pub already_modded: bool,
    pub authored: EnemyTable,
    pub boot_elf: Option<IsoFile>,
    pub cnf: Option<IsoFile>,
}

/// Reads the disc at `path` and refuses it unless it is an unpatched copy of
/// the release `layout` describes. Nothing is written.
pub fn inspect(path: &Path, layout: &Layout) -> Result<DiscReport> {
    let file = File::open(path)?;
    // Safety: the mapping is read-only and `path` is never written while mapped.
    // `Mmap::map` refuses an empty file, so hand that empty input to
    // `inspect_bytes` directly: it produces the same refusal `std::fs::read`
    // did before this was memory-mapped.
    if file.metadata()?.len() == 0 {
        return inspect_bytes(&[], layout);
    }
    let mmap = unsafe { Mmap::map(&file)? };
    inspect_bytes(&mmap[..], layout)
}

/// The checks [`inspect`] applies, in order, against an image already in memory.
///
/// 1. the three authored blocks parse, else [`Error::NotThisDisc`];
/// 2. each block occurs exactly [`Layout::expected_copies`] times, else
///    [`Error::WrongRevision`] naming the block;
/// 3. more than 100 rows carry a crown, then [`Error::AlreadyModded`],
///    otherwise the report is returned.
pub fn inspect_bytes(iso: &[u8], layout: &Layout) -> Result<DiscReport> {
    let authored = EnemyTable::read(iso, layout)?;

    let blocks = authored.blocks();
    let mut copies = [0usize; 3];
    for (found, block) in copies.iter_mut().zip(blocks.iter()) {
        *found = count_copies(iso, block);
    }
    for (&found, block) in copies.iter().zip(BLOCK_NAMES) {
        if found != layout.expected_copies {
            return Err(Error::WrongRevision {
                block,
                found,
                expected: layout.expected_copies,
            });
        }
    }

    let rarity_nonzero = authored
        .rarity
        .iter()
        .filter(|&&rarity| rarity != 0)
        .count();
    let already_modded = rarity_nonzero > ALREADY_MODDED_ROWS;
    if already_modded {
        return Err(Error::AlreadyModded { rarity_nonzero });
    }

    Ok(DiscReport {
        size: iso.len() as u64,
        release: None,
        copies,
        rarity_nonzero,
        already_modded,
        authored,
        boot_elf: iso9660::find_file(iso, BOOT_ELF_NAME).ok(),
        cnf: iso9660::find_file(iso, CNF_NAME).ok(),
    })
}

/// Inspect any supported retail PS2 release, using its own table anchors.
pub fn inspect_retail(path: &Path) -> Result<DiscReport> {
    let file = File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Err(Error::NotThisDisc);
    }
    // Safety: input is opened read-only and is never modified by the patcher.
    let mmap = unsafe { Mmap::map(&file)? };
    inspect_retail_bytes(&mmap)
}

pub fn inspect_retail_bytes(iso: &[u8]) -> Result<DiscReport> {
    let release = release::detect(iso)?;
    let mut report = inspect_bytes(iso, &release.layout())?;
    // All three retail releases carry identical tables. Copy counts alone do
    // not prove that an arbitrary/repacked table is safe to scale by row index.
    let fingerprints = [
        "99d6f13977284dff4b19f90dc46ea78e",
        "7d8043fa225ccf25d2d83736d6c0fcc9",
        "de9b8db0994cbb43cb67baf71bf1e998",
    ];
    for (block, expected) in report.authored.blocks().iter().zip(fingerprints) {
        if format!("{:x}", Md5::digest(block)) != expected {
            return Err(Error::NotThisDisc);
        }
    }
    report.release = Some(release);
    report.boot_elf = Some(iso9660::find_file(iso, release.serial())?);
    Ok(report)
}
