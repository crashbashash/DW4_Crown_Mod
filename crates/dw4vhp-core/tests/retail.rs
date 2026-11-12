//! Optional full-disc regression tests. No game data is included in this repo.
use dw4vhp_core::{
    disc::{inspect_retail, inspect_retail_planned_for_plan},
    error::Error,
    patch::{patch_file, PatchOptions},
    plan::{transform, PatchPlan},
    ratio::RatioVector,
    release::Release,
};
use md5::{Digest, Md5};
use memmap2::Mmap;
use std::{fs::File, path::PathBuf};

/// The default build writes the computed table into every authored copy,
/// matches the app's own archive plan, changes nothing else and refuses to
/// patch its own output. Expectations are computed from the input, so
/// intended balance or archive changes do not need re-pinned checksums.
fn check(key: &str, release: Release, input_md5: &str) {
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

    let plan = PatchPlan::default();
    let tmp = tempfile::tempdir().unwrap();
    let output = tmp.path().join("patched.iso");
    let outcome = patch_file(
        &path,
        &output,
        &plan,
        &PatchOptions { overwrite: false },
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(outcome.bytes, report.output_size);
    assert!(outcome.bytes > original.len() as u64);
    let out_file = File::open(&output).unwrap();
    // Safety: completed output is read-only and no longer being patched.
    let patched = unsafe { Mmap::map(&out_file).unwrap() };
    assert_eq!(format!("{:x}", Md5::digest(&patched)), outcome.md5);
    let signature = dw4vhp_core::signature::from_image(&patched)
        .unwrap()
        .unwrap();
    let mut identity = signature.clone();
    identity.as_object_mut().unwrap().remove("recovery");
    assert_eq!(identity, dw4vhp_core::signature::metadata(release, &plan));

    let layout = release.layout();
    let offsets = [layout.hp_off, layout.stat_off, layout.byte_off];
    let mut ranges = Vec::new();
    let (expected_table, _) = transform(
        &report.authored,
        &plan,
        &RatioVector::derive(&report.authored),
    );
    for ((block, offset), computed) in report
        .authored
        .blocks()
        .iter()
        .zip(offsets)
        .zip(expected_table.blocks())
    {
        let expected = &patched[offset as usize..offset as usize + block.len()];
        assert_eq!(expected, computed.as_slice());
        let mut count = 0;
        for start in memchr::memmem::find_iter(&original, block) {
            let end = start + block.len();
            assert_eq!(&patched[start..end], expected);
            ranges.push((start, end));
            count += 1;
        }
        assert_eq!(count, 665);
    }
    // The app's own plan (archive, recovery and signature) for the same input.
    let (_, crowns) = inspect_retail_planned_for_plan(&original, &plan).unwrap();
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
        Err(Error::AlreadyCrownMod { .. })
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
        Err(Error::AlreadyCrownMod { .. })
    ));
    assert!(!second_output.exists());
    assert_eq!(
        format!("{:x}", Md5::digest(&original)),
        input_md5,
        "original must be unchanged"
    );
    eprintln!(
        "PASS: {} {} → {}; all 1995 table blocks checked; original archive/ELF and unrelated bytes unchanged; expanded archive and signature verified",
        release.label(),
        release.serial(),
        outcome.md5
    );
}

#[test]
fn usa_retail() {
    check(
        "DW4_RETAIL_USA_ISO",
        Release::Usa,
        "3185f04230b1dabd853db750e4b51108",
    );
}
#[test]
fn europe_retail() {
    check(
        "DW4_RETAIL_EUROPE_ISO",
        Release::Europe,
        "bd95f00a8efa2ab6b502b7aade37f143",
    );
}
#[test]
fn japan_retail() {
    check(
        "DW4_RETAIL_JAPAN_ISO",
        Release::Japan,
        "78543a26a08d82fb32f1f3b5035d7e3b",
    );
}
