//! Optional full-disc regression tests. No game data is included in this repo.
use dw4vhp_core::{
    disc::inspect_retail,
    error::Error,
    patch::{patch_file, PatchOptions},
    plan::PatchPlan,
    release::Release,
};
use md5::{Digest, Md5};
use memmap2::Mmap;
use std::{fs::File, path::PathBuf};

fn check(key: &str, release: Release, input_md5: &str, output_md5: &str) {
    let Some(path) = std::env::var_os(key).map(PathBuf::from) else {
        eprintln!("SKIP: {key} not configured");
        return;
    };
    let file = File::open(&path).unwrap();
    // Safety: pristine input is read-only throughout this test.
    let original = unsafe { Mmap::map(&file).unwrap() };
    assert_eq!(format!("{:x}", Md5::digest(&original)), input_md5);
    let report = inspect_retail(&path).unwrap();
    assert_eq!(report.release, Some(release));
    assert_eq!(report.copies, [665; 3]);
    assert_eq!(report.rarity_nonzero, 60);

    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("patched.iso");
    let outcome = patch_file(
        &path,
        &output,
        &PatchPlan::default(),
        &PatchOptions { overwrite: false },
        &mut |_| {},
    )
    .unwrap();
    // Independent Python prototype outputs, preserving the ISO terminator
    // reserved bytes (the prototype wrote its size into all descriptors).
    assert_eq!(outcome.md5, output_md5);
    assert_eq!(outcome.bytes, report.output_size);
    assert!(outcome.bytes > original.len() as u64);
    let out_file = File::open(&output).unwrap();
    // Safety: completed output is read-only and no longer being patched.
    let patched = unsafe { Mmap::map(&out_file).unwrap() };
    assert_eq!(format!("{:x}", Md5::digest(&patched)), output_md5);

    let layout = release.layout();
    let offsets = [layout.hp_off, layout.stat_off, layout.byte_off];
    let mut ranges = Vec::new();
    let shipped_hashes = [
        "56885ddab3be87476c0810abf6b6f152",
        "42df4de8f3afc8f032a16e293cc1e3ab",
        "3ce7d29cd12e93d05eb201356b231288",
    ];
    for ((block, offset), hash) in report
        .authored
        .blocks()
        .iter()
        .zip(offsets)
        .zip(shipped_hashes)
    {
        let expected = &patched[offset as usize..offset as usize + block.len()];
        assert_eq!(format!("{:x}", Md5::digest(expected)), hash);
        let mut count = 0;
        for start in memchr::memmem::find_iter(&original, block) {
            let end = start + block.len();
            assert_eq!(&patched[start..end], expected);
            ranges.push((start, end));
            count += 1;
        }
        assert_eq!(count, 665);
    }
    let crowns = dw4vhp_core::crown::CrownPlan::build(&original).unwrap();
    crowns.verify(&original, &patched).unwrap();
    ranges.extend(
        crowns
            .regions
            .iter()
            .map(|r| (r.offset as usize, r.offset as usize + r.bytes.len())),
    );
    ranges.sort_unstable();
    let mut cursor = 0;
    for (start, end) in ranges {
        assert!(start >= cursor, "table ranges must not overlap");
        assert!(
            original[cursor..start] == patched[cursor..start],
            "changed bytes outside table ranges"
        );
        cursor = end;
    }
    assert!(original[cursor..] == patched[cursor..original.len()]);
    assert!(matches!(
        inspect_retail(&output),
        Err(Error::AlreadyModded { .. })
    ));
    let second_output = tmp.path().join("twice.iso");
    assert!(matches!(
        patch_file(
            &output,
            &second_output,
            &PatchPlan::default(),
            &PatchOptions { overwrite: false },
            &mut |_| {}
        ),
        Err(Error::AlreadyModded { .. })
    ));
    assert!(!second_output.exists());
    assert_eq!(
        format!("{:x}", Md5::digest(&original)),
        input_md5,
        "original must be unchanged"
    );
    eprintln!(
        "PASS: {} {} → {output_md5}; all 1995 table blocks checked; original archive/ELF and unrelated bytes unchanged; expanded archive verified",
        release.label(),
        release.serial()
    );
}

#[test]
fn usa_retail() {
    check(
        "DW4_RETAIL_USA_ISO",
        Release::Usa,
        "3185f04230b1dabd853db750e4b51108",
        "9ba8432d2237330a687d4ac1a0203d68",
    );
}
#[test]
fn europe_retail() {
    check(
        "DW4_RETAIL_EUROPE_ISO",
        Release::Europe,
        "bd95f00a8efa2ab6b502b7aade37f143",
        "0dfd365ae96ef0ab30e326b5096e0f46",
    );
}
#[test]
fn japan_retail() {
    check(
        "DW4_RETAIL_JAPAN_ISO",
        Release::Japan,
        "78543a26a08d82fb32f1f3b5035d7e3b",
        "4a6815c3c7e5e4ee7f793b04b82eda52",
    );
}
