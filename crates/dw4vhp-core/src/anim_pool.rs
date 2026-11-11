//! Allocate animation decoder contexts from the general arena.
//!
//! The animation context pool (`igEnbayaContextPool`) keeps one decoder context
//! per simultaneously decoding animation and grows by one context whenever all
//! are in use. Each context is `(tracks - 1) * 96 + 272` bytes. The pool object
//! lives in the small system pool, so with many animated enemies on screen a
//! growth request can find no free system-pool block. The grow routine passes
//! the result straight to the context constructor, whose null path still writes
//! the context header: "Invalid write to 0x000000a9" at the constructor. A
//! skipped context is not an option either: the pool then returns the element
//! past the end of its list.
//!
//! Contexts are created only by the grow routine and freed only by the pool's
//! destructor, and both look the allocator up from the pool object's encoded
//! pool index. Both lookups now resolve encoded index 10, the general arena
//! (the same arena the system-pool fallback already borrows from), so contexts
//! never compete for system-pool memory and are always freed where they were
//! allocated. Context contents and the pool's bookkeeping are unchanged.
use crate::{
    elf::ElfInfo,
    error::{Error, Result},
    region::WriteRegion,
    release::Release,
};

/// Encoded allocator index of the general arena on both platforms.
const GENERAL: u32 = 10;

fn bad() -> Error {
    Error::CrownData("unrecognised animation context pool code".into())
}

/// One lookup: `(site, original, replacement)` words.
type Word = (u32, u32, u32);

/// PowerPC: `mr r3,<owner>` becomes `li r3,10`, and the call to the
/// object-to-allocator helper becomes a call to the encoded-index resolver.
fn gc_words(release: Release) -> Option<Vec<Word>> {
    let (grow, free, object, resolve) = match release {
        Release::GcUsa => (0x801e_5fb0, 0x801e_5b48, 0x8006_8430, 0x8005_929c),
        Release::GcJapan => (0x801e_5f50, 0x801e_5ae8, 0x8006_83d0, 0x8005_923c),
        _ => return None,
    };
    let bl = |from: u32, to: u32| 0x4800_0001 | (to.wrapping_sub(from) & 0x03ff_fffc);
    let li_r3 = 0x3860_0000 | GENERAL;
    Some(vec![
        (grow, 0x7f23_cb78, li_r3),
        (grow + 8, bl(grow + 8, object), bl(grow + 8, resolve)),
        (free, 0x7ee3_bb78, li_r3),
        (free + 8, bl(free + 8, object), bl(free + 8, resolve)),
    ])
}

/// MIPS: `jal igObject_GetAllocator; move a0,<owner>` becomes
/// `jal igAllocator_ResolveEncodedIndex; li a0,10`.
fn ps2_words(release: Release) -> Option<Vec<Word>> {
    let (grow, free, object, resolve) = match release {
        Release::Usa => (0x1c_19f0, 0x1c_14e8, 0x30_1000, 0x2f_0700),
        Release::Europe => (0x1c_1a60, 0x1c_1558, 0x30_10f0, 0x2f_07f0),
        Release::Japan => (0x1c_30c0, 0x1c_2bb8, 0x30_26d0, 0x2f_1dd0),
        _ => return None,
    };
    let jal = |to: u32| 0x0c00_0000 | (to >> 2);
    let li_a0 = 0x2404_0000 | GENERAL;
    Some(vec![
        (grow, jal(object), jal(resolve)),
        (grow + 4, 0x0280_202d, li_a0), // move a0,s4
        (free, jal(object), jal(resolve)),
        (free + 4, 0x02a0_202d, li_a0), // move a0,s5
    ])
}

fn regions(
    iso: &[u8],
    words: Vec<Word>,
    at: impl Fn(u32) -> Result<usize>,
    big_endian: bool,
) -> Result<Vec<WriteRegion>> {
    let bytes = |w: u32| {
        if big_endian {
            w.to_be_bytes()
        } else {
            w.to_le_bytes()
        }
    };
    words
        .into_iter()
        .map(|(site, original, replacement)| {
            let offset = at(site)?;
            if iso.get(offset..offset + 4) != Some(bytes(original).as_slice()) {
                return Err(bad());
            }
            Ok(WriteRegion {
                offset: offset as u64,
                bytes: bytes(replacement).to_vec(),
            })
        })
        .collect()
}

pub(crate) fn gc_regions(
    iso: &[u8],
    release: Release,
    map: impl Fn(&[u8], u32, usize) -> Result<usize>,
) -> Result<Vec<WriteRegion>> {
    let Some(words) = gc_words(release) else {
        return Ok(Vec::new());
    };
    regions(iso, words, |a| map(iso, a, 4), true)
}

pub(crate) fn ps2_regions(iso: &[u8], release: Release, elf: &ElfInfo) -> Result<Vec<WriteRegion>> {
    let Some(words) = ps2_words(release) else {
        return Ok(Vec::new());
    };
    let at = |a| {
        elf.vaddr_to_iso_offset(a)
            .map(|v| v as usize)
            .ok_or_else(bad)
    };
    regions(iso, words, at, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_target_the_resolver_with_the_general_index() {
        let gc = gc_words(Release::GcUsa).unwrap();
        // li r3,10 and `bl 0x8005929c` from 0x801e5fb8.
        assert_eq!(gc[0].2, 0x3860_000a);
        assert_eq!(gc[1], (0x801e_5fb8, 0x4be8_2479, 0x4be7_32e5));
        let ps2 = ps2_words(Release::Usa).unwrap();
        assert_eq!(ps2[0], (0x1c_19f0, 0x0c0c_0400, 0x0c0b_c1c0));
        assert_eq!(ps2[1].2, 0x2404_000a);
        assert!(gc_words(Release::Usa).is_none() && ps2_words(Release::GcUsa).is_none());
    }

    /// Optional retail check: every site holds the expected original code.
    #[test]
    fn retail_sites_match_on_every_release() {
        for variable in [
            "DW4_RETAIL_USA_ISO",
            "DW4_RETAIL_EUROPE_ISO",
            "DW4_RETAIL_JAPAN_ISO",
            "DW4_RETAIL_GC_USA_ISO",
            "DW4_RETAIL_GC_JAPAN_ISO",
        ] {
            let Some(path) = std::env::var_os(variable) else {
                continue;
            };
            let iso = std::fs::read(path).unwrap();
            let release = crate::release::detect(&iso).unwrap();
            let regions = if release.is_gamecube() {
                gc_regions(&iso, release, crate::gc_memory::map_address).unwrap()
            } else {
                let boot = crate::iso9660::find_file(&iso, release.serial()).unwrap();
                let elf = ElfInfo::read(&iso, &boot).unwrap();
                ps2_regions(&iso, release, &elf).unwrap()
            };
            assert_eq!(regions.len(), 4, "{variable}");
            let mut changed = iso.clone();
            for r in &regions {
                changed[r.offset as usize..r.offset as usize + 4].copy_from_slice(&r.bytes);
            }
            assert!(
                gc_regions(&changed, release, crate::gc_memory::map_address).is_err()
                    || !release.is_gamecube()
            );
        }
    }
}
