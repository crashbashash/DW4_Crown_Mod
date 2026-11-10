//! Opt-in retail checks compare updates against fresh builds byte for byte.
use dw4vhp_core::{
    patch::{patch_file, PatchOptions},
    plan::PatchPlan,
    update::{metadata, update_image},
};
use std::{fs, path::Path};
fn hash(path: &Path) -> String {
    use md5::{Digest, Md5};
    use std::io::Read;
    let mut file = fs::File::open(path).unwrap();
    let mut hash = Md5::new();
    let mut buffer = vec![0; 8 << 20];
    loop {
        let size = file.read(&mut buffer).unwrap();
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    format!("{:x}", hash.finalize())
}
fn check_in_place_update(key: &str) {
    let Some(original) = std::env::var_os(key) else {
        eprintln!("SKIP: {key} not configured");
        return;
    };
    let original = Path::new(&original);
    let original_hash = hash(original);
    let work = tempfile::tempdir().unwrap();
    let target = work.path().join("selected.iso");
    let expected = work.path().join("expected.iso");
    let base = PatchPlan::default();
    let all = PatchPlan {
        four_player_spawns: true,
        crown_drops: true,
        random_crown_colours: true,
        crown_colour_seed: Some(123),
        ..base.clone()
    };
    let opts = PatchOptions { overwrite: true };
    patch_file(original, &target, &base, &opts, &mut |_| {}).unwrap();
    assert!(metadata(&target).unwrap().unwrap()["recovery"].is_object());
    let mut progress = Vec::new();
    update_image(&target, &all, &mut |p| progress.push(p)).unwrap();
    assert!(progress.windows(2).all(|p| p[0].done <= p[1].done));
    assert!(progress.iter().all(|p| p.total == 10000));
    assert_eq!(progress.last().unwrap().done, 10000);
    assert!(
        progress
            .iter()
            .filter(|p| p.done > 2625 && p.done < 3000)
            .count()
            > 4,
        "recovery preparation must report incremental progress before copying"
    );
    patch_file(original, &expected, &all, &opts, &mut |_| {}).unwrap();
    assert_eq!(
        hash(&target),
        hash(&expected),
        "adding features must equal a fresh build"
    );
    let keep_seed = PatchPlan {
        crown_colour_seed: None,
        ..all.clone()
    };
    update_image(&target, &keep_seed, &mut |_| {}).unwrap();
    assert_eq!(
        hash(&target),
        hash(&expected),
        "repeated update must not stack or grow"
    );
    update_image(&target, &base, &mut |_| {}).unwrap();
    patch_file(original, &expected, &base, &opts, &mut |_| {}).unwrap();
    assert_eq!(
        hash(&target),
        hash(&expected),
        "removing features must restore executable hooks and resources"
    );
    use std::io::{Read, Seek, SeekFrom, Write};
    // A modified untouched byte must fail recovery without replacing the file.
    let pristine = work.path().join("pristine.iso");
    fs::copy(&target, &pristine).unwrap();
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .unwrap();
    file.seek(SeekFrom::Start(0x1000)).unwrap();
    let mut byte = [0];
    file.read_exact(&mut byte).unwrap();
    byte[0] ^= 1;
    file.seek(SeekFrom::Start(0x1000)).unwrap();
    file.write_all(&byte).unwrap();
    drop(file);
    let damaged_hash = hash(&target);
    assert!(update_image(&target, &base, &mut |_| {}).is_err());
    assert_eq!(
        hash(&target),
        damaged_hash,
        "failed recovery preserves the selected file"
    );
    fs::rename(&pristine, &target).unwrap();
    // A signed image without recovery data is refused, not migrated.
    let mut legacy = metadata(&target).unwrap().unwrap();
    legacy.as_object_mut().unwrap().remove("recovery");
    let mut file = fs::OpenOptions::new().write(true).open(&target).unwrap();
    file.seek(SeekFrom::End(-2048)).unwrap();
    file.write_all(&dw4vhp_core::signature::encode(&legacy).unwrap())
        .unwrap();
    drop(file);
    let before = hash(&target);
    let error = update_image(&target, &base, &mut |_| {}).unwrap_err();
    assert!(error.to_string().contains("v0.11.1"), "{error}");
    assert_eq!(hash(&target), before, "a refused update preserves the file");
    // So is an unsigned image from before Crown Mod v0.11.1.
    let file = fs::OpenOptions::new().write(true).open(&target).unwrap();
    file.set_len(file.metadata().unwrap().len() - 2048).unwrap();
    drop(file);
    assert!(metadata(&target).unwrap().is_none());
    let before = hash(&target);
    assert!(update_image(&target, &base, &mut |_| {}).is_err());
    assert_eq!(hash(&target), before, "a refused update preserves the file");
    assert_eq!(
        hash(original),
        original_hash,
        "original disc must remain unchanged"
    );
    eprintln!("PASS: {key}: add, repeat, remove, seed preservation, damaged recovery refusal, pre-v0.11.1 images refused unchanged, original unchanged");
}

#[test]
fn ps2_usa_updates_without_stacking() {
    check_in_place_update("DW4_UPDATE_PS2");
}

#[test]
fn ps2_europe_updates_without_stacking() {
    check_in_place_update("DW4_UPDATE_PS2_EUROPE");
}

#[test]
fn ps2_japan_updates_without_stacking() {
    check_in_place_update("DW4_UPDATE_PS2_JAPAN");
}

#[test]
fn gamecube_usa_updates_without_stacking() {
    check_in_place_update("DW4_UPDATE_GC");
}

#[test]
fn gamecube_japan_updates_without_stacking() {
    check_in_place_update("DW4_UPDATE_GC_JAPAN");
}

#[test]
fn compressed_image_is_replaced_in_its_original_format() {
    let Some(original) = std::env::var_os("DW4_UPDATE_COMPRESSED") else {
        return;
    };
    let original = Path::new(&original);
    let format = dw4vhp_core::container::detect(original).unwrap();
    assert_ne!(format, dw4vhp_core::container::Format::Raw);
    let work = tempfile::tempdir().unwrap();
    let selected = work.path().join(format!("selected.{}", format.extension()));
    let expected = work.path().join(format!("expected.{}", format.extension()));
    let all = PatchPlan {
        four_player_spawns: true,
        crown_drops: true,
        random_crown_colours: true,
        crown_colour_seed: Some(123),
        ..PatchPlan::default()
    };
    let opts = PatchOptions { overwrite: false };
    patch_file(original, &selected, &all, &opts, &mut |_| {}).unwrap();
    let mut progress = Vec::new();
    update_image(&selected, &PatchPlan::default(), &mut |p| progress.push(p)).unwrap();
    assert!(progress.windows(2).all(|pair| pair[0].done <= pair[1].done));
    assert!(progress.iter().all(|p| p.total == 10000));
    assert_eq!(progress.last().unwrap().done, 10000);
    patch_file(
        original,
        &expected,
        &PatchPlan::default(),
        &opts,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(dw4vhp_core::container::detect(&selected).unwrap(), format);
    assert_eq!(hash(&selected), hash(&expected));
    eprintln!("PASS: compressed {format:?} update matches fresh build, retains format, monotonic progress");
}
