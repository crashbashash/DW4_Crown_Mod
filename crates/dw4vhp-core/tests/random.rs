//! Optional full-disc check of the experimental enemy randomiser. No game data is included.
use dw4vhp_core::crown::{CrownPlan, RandomOptions};
use std::path::PathBuf;

fn options(seed: u64) -> RandomOptions {
    RandomOptions {
        seed,
        donors: 3,
        warp: None,
        only: None,
    }
}

#[test]
fn randomised_image_is_deterministic_and_verifies() {
    let Some(path) = std::env::var_os("DW4_RETAIL_USA_ISO").map(PathBuf::from) else {
        eprintln!("SKIP: DW4_RETAIL_USA_ISO not configured");
        return;
    };
    let iso = std::fs::read(path).unwrap();
    let a = CrownPlan::build_random(&iso, options(7)).unwrap();
    let b = CrownPlan::build_random(&iso, options(7)).unwrap();
    let c = CrownPlan::build_random(&iso, options(8)).unwrap();
    assert_eq!(a.output_size, b.output_size, "same seed, same image");
    assert_ne!(a.output_size, c.output_size, "another seed, another image");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("random.iso");
    let mut file = std::fs::File::create_new(&out).unwrap();
    a.write_image(&iso, &mut file).unwrap();
    drop(file);
    let written = std::fs::read(&out).unwrap();
    assert_eq!(written.len() as u64, a.output_size);
    a.verify(&iso, &written).unwrap();
}

#[test]
fn signed_patch_records_the_seed_and_reproduces() {
    use dw4vhp_core::{
        patch::{patch_file, PatchOptions},
        plan::PatchPlan,
        signature,
    };
    let Some(path) = std::env::var_os("DW4_RETAIL_USA_ISO").map(PathBuf::from) else {
        eprintln!("SKIP: DW4_RETAIL_USA_ISO not configured");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let build = |name: &str, seed: Option<u32>| {
        let out = tmp.path().join(name);
        let plan = PatchPlan {
            random_enemies: true,
            enemy_seed: seed,
            ..PatchPlan::default()
        };
        patch_file(
            &path,
            &out,
            &plan,
            &PatchOptions { overwrite: false },
            &mut |_| {},
        )
        .unwrap();
        let signed = signature::read_raw(&out).unwrap().expect("signed image");
        (out, signed)
    };
    let (a, sa) = build("a.iso", Some(1234));
    let (b, sb) = build("b.iso", Some(1234));
    let (_, sc) = build("c.iso", None);
    assert_eq!(sa["options"]["randomEnemies"], true);
    assert_eq!(sa["options"]["enemySeed"], 1234);
    let r = &sa["randomEnemies"];
    assert_eq!(r["seed"], 1234);
    assert_eq!(r["rulesVersion"], dw4vhp_core::crown::RANDOM_RULES_VERSION);
    assert!(r["stages"].as_u64().unwrap() >= 60, "{r}");
    assert!(r["generators"].as_u64().unwrap() > 1000, "{r}");
    assert_eq!(r["undrawnEffects"], 0, "{r}");
    assert_eq!(
        sa["randomEnemies"], sb["randomEnemies"],
        "same seed, same plan"
    );
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "same seed, same bytes"
    );
    // An unseeded patch picks and records its own seed.
    let drawn = sc["options"]["enemySeed"].as_u64().expect("seed recorded");
    assert_eq!(sc["randomEnemies"]["seed"].as_u64(), Some(drawn));
    assert_ne!(sc["randomEnemies"]["planDigest"], r["planDigest"]);
}

/// Every supported release builds with random enemies, records its plan, and releases that
/// share game data share a plan: the three PS2 discs agree, as do the two GameCube discs.
/// Point the variables at the player's own images (CISO and RVZ are read directly).
#[test]
fn every_release_builds_and_records_its_plan() {
    use dw4vhp_core::{
        patch::{patch_file, PatchOptions},
        plan::PatchPlan,
        signature,
    };
    let releases = [
        ("DW4_UPDATE_PS2", "ps2"),
        ("DW4_UPDATE_PS2_EUROPE", "ps2"),
        ("DW4_UPDATE_PS2_JAPAN", "ps2"),
        ("DW4_UPDATE_GC", "gc"),
        ("DW4_UPDATE_GC_JAPAN", "gc"),
    ];
    let tmp = tempfile::tempdir().unwrap();
    let mut digests: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    let mut ran = 0;
    for (variable, platform) in releases {
        let Some(path) = std::env::var_os(variable).map(PathBuf::from) else {
            eprintln!("SKIP: {variable} not configured");
            continue;
        };
        let out = tmp.path().join(format!("{variable}.out"));
        let plan = PatchPlan {
            random_enemies: true,
            enemy_seed: Some(1234),
            ..PatchPlan::default()
        };
        patch_file(
            &path,
            &out,
            &plan,
            &PatchOptions { overwrite: false },
            &mut |_| {},
        )
        .unwrap();
        let signed = signature::read_raw(&out).unwrap().expect("signed image");
        let r = &signed["randomEnemies"];
        assert_eq!(signed["platform"], platform, "{variable}");
        assert_eq!(r["seed"], 1234, "{variable}");
        assert_eq!(r["stages"], 72, "{variable}: {r}");
        assert_eq!(r["undrawnEffects"], 0, "{variable}: {r}");
        digests
            .entry(platform)
            .or_default()
            .push(r["planDigest"].as_str().unwrap().to_owned());
        std::fs::remove_file(&out).unwrap();
        ran += 1;
    }
    if ran == 0 {
        eprintln!("SKIP: no release images configured");
    }
    for (platform, list) in digests {
        assert!(
            list.windows(2).all(|w| w[0] == w[1]),
            "{platform} releases disagree: {list:?}"
        );
    }
}
