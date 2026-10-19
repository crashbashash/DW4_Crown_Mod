use dw4vhp_core::disc::{count_copies, inspect};
use dw4vhp_core::error::Error;
use dw4vhp_core::layout::Layout;
use dw4vhp_core::testkit;

#[test]
fn counts_every_occurrence() {
    let hay = b"abcabcabc";
    assert_eq!(count_copies(hay, b"abc"), 3);
    assert_eq!(count_copies(hay, b"zzz"), 0);
}

#[test]
fn accepts_a_clean_fixture() {
    let (dir, path, layout) = testkit::clean_disc_tempfile();
    let r = inspect(&path, &layout).unwrap();
    assert_eq!(r.copies, [3, 3, 3]);
    assert!(!r.already_modded);
    assert_eq!(r.authored.len(), 649);
    drop(dir);
}

#[test]
fn an_empty_file_is_not_this_disc_not_a_mmap_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.iso");
    std::fs::File::create(&path).unwrap();
    let err = inspect(&path, &Layout::mini(1, 1)).unwrap_err();
    assert!(matches!(err, Error::NotThisDisc), "{err:?}");
}

#[test]
fn a_wrong_copy_count_is_a_revision_mismatch() {
    let (dir, path, _) = testkit::clean_disc_tempfile(); // the fixture writes 3 copies
    let err = inspect(&path, &Layout::mini(649, 665)).unwrap_err();
    assert!(matches!(err, Error::WrongRevision { .. }), "{err:?}");
    drop(dir);
}

#[test]
fn a_disc_with_crowns_already_written_is_already_modded() {
    let (dir, path, layout) = testkit::modded_disc_tempfile();
    let err = inspect(&path, &layout).unwrap_err();
    assert!(matches!(err, Error::AlreadyModded { .. }), "{err:?}");
    drop(dir);
}

#[test]
fn signature_is_checked_before_tables_and_legacy_images_still_use_the_scan() {
    let block = dw4vhp_core::signature::encode(&dw4vhp_core::signature::metadata(
        dw4vhp_core::release::Release::Usa,
        &dw4vhp_core::plan::PatchPlan::default(),
    ))
    .unwrap();
    // This is too short to contain even the table. A signature refusal proves
    // the expensive/legacy scan was never entered.
    let error = dw4vhp_core::disc::inspect_bytes(&block, &Layout::retail()).unwrap_err();
    assert!(matches!(error, Error::AlreadyCrownMod { .. }));
    let (directory, path, layout) = testkit::modded_disc_tempfile();
    assert!(matches!(
        inspect(&path, &layout),
        Err(Error::AlreadyModded { .. })
    ));
    drop(directory);
}
