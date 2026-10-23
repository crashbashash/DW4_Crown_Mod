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
    // The unsigned default-build fingerprints remain an independent
    // oracle after reversing only the footer/volume-length metadata below.
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
    assert_eq!(
        identity,
        dw4vhp_core::signature::metadata(release, &PatchPlan::default())
    );
    // Check the independent unsigned golden checksum: remove only the
    // footer and reverse the volume-size update in this hashing view.
    let unsigned_len = signature["recovery"]["archiveEnd"].as_u64().unwrap() as usize;
    let mut head = patched[..64 * 2048].to_vec();
    for sector in 16..64 {
        let at = sector * 2048;
        if head[at] == 255 {
            break;
        }
        if head[at] == 1 || head[at] == 2 {
            let sectors = (unsigned_len / 2048) as u32;
            head[at + 80..at + 88]
                .copy_from_slice(&[sectors.to_le_bytes(), sectors.to_be_bytes()].concat());
        }
    }
    let mut legacy = Md5::new();
    legacy.update(&head);
    legacy.update(&patched[head.len()..unsigned_len]);
    assert_eq!(format!("{:x}", legacy.finalize()), output_md5);

    let layout = release.layout();
    let offsets = [layout.hp_off, layout.stat_off, layout.byte_off];
    let mut ranges = Vec::new();
    let shipped_hashes = [
        "56885ddab3be87476c0810abf6b6f152",
        "c2cdbb0ca237b6619d55374bd4ed86e5",
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
    // Verify the historical archive independently of the new recovery footer.
    let mut archive_view = patched[..unsigned_len].to_vec();
    archive_view[..head.len()].copy_from_slice(&head);
    crowns.verify(&original, &archive_view).unwrap();
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
        "PASS: {} {} → {actual} (unsigned {output_md5}); all 1995 table blocks checked; original archive/ELF and unrelated bytes unchanged; expanded archive and signature verified",
        release.label(),
        release.serial(), actual = outcome.md5
    );
}

#[test]
fn usa_retail() {
    check(
        "DW4_RETAIL_USA_ISO",
        Release::Usa,
        "3185f04230b1dabd853db750e4b51108",
        "25215189ba7f252e15beb9dd3ea27ced",
    );
}
#[test]
fn europe_retail() {
    check(
        "DW4_RETAIL_EUROPE_ISO",
        Release::Europe,
        "bd95f00a8efa2ab6b502b7aade37f143",
        "e2c59ac2b1e98f266ae97d4cdf9a8a4e",
    );
}
#[test]
fn japan_retail() {
    check(
        "DW4_RETAIL_JAPAN_ISO",
        Release::Japan,
        "78543a26a08d82fb32f1f3b5035d7e3b",
        "df9b8f3a15fde1eade95f19fe7ca01f2",
    );
}
