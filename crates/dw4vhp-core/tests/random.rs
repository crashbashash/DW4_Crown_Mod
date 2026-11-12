//! Optional full-disc check of the experimental enemy randomiser. No game data is included.
use dw4vhp_core::crown::{CrownPlan, RandomOptions};
use std::path::PathBuf;

fn options(seed: u64) -> RandomOptions {
    RandomOptions {
        picks_only: false,
        seed,
        donors: 3,
        warp: None,
        only: None,
        random_bosses: false,
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
    // Each image (~3.7 GB) is hashed and removed before the next build, so the
    // test needs scratch space for one image at a time.
    let tmp = tempfile::tempdir().unwrap();
    let build = |seed: Option<u32>| {
        let out = tmp.path().join("random.iso");
        let plan = PatchPlan {
            random_enemies: true,
            enemy_seed: seed,
            ..PatchPlan::default()
        };
        let outcome = patch_file(
            &path,
            &out,
            &plan,
            &PatchOptions { overwrite: false },
            &mut |_| {},
        )
        .unwrap();
        let signed = signature::read_raw(&out).unwrap().expect("signed image");
        std::fs::remove_file(&out).unwrap();
        (outcome.md5, signed)
    };
    let (a, sa) = build(Some(1234));
    let (b, sb) = build(Some(1234));
    let (_, sc) = build(None);
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
    assert_eq!(a, b, "same seed, same bytes");
    // An unseeded patch picks and records its own seed.
    let drawn = sc["options"]["enemySeed"].as_u64().expect("seed recorded");
    assert_eq!(sc["randomEnemies"]["seed"].as_u64(), Some(drawn));
    assert_ne!(sc["randomEnemies"]["planDigest"], r["planDigest"]);
}

/// Every supported release builds with random enemies and records its plan, and the
/// analysis estimate covers the built image without overstating it by more than 3%.
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
        let outcome = patch_file(
            &path,
            &out,
            &plan,
            &PatchOptions { overwrite: false },
            &mut |_| {},
        )
        .unwrap();
        if dw4vhp_core::container::detect(&path).unwrap() == dw4vhp_core::container::Format::Raw {
            let selection = dw4vhp_core::options::PatchSelection {
                crown_mod: true,
                random_enemies: true,
                enemy_seed: Some(1234),
                ..Default::default()
            };
            let report =
                dw4vhp_core::disc::inspect_retail_for_plan(&path, &selection.analysis_plan())
                    .unwrap();
            let estimate = selection.estimated_output_size(&report);
            assert!(
                outcome.bytes <= estimate && estimate * 100 <= outcome.bytes * 103,
                "{variable}: estimate {estimate}, built {}",
                outcome.bytes
            );
        }
        let signed = signature::read_raw(&out).unwrap().expect("signed image");
        let r = &signed["randomEnemies"];
        assert_eq!(signed["platform"], platform, "{variable}");
        assert_eq!(r["seed"], 1234, "{variable}");
        assert!(
            r["stages"].as_u64().unwrap() >= 69,
            "retain at least the previous stage coverage: {variable}: {r}"
        );
        assert_eq!(r["undrawnEffects"], 0, "{variable}: {r}");
        std::fs::remove_file(&out).unwrap();
        ran += 1;
    }
    if ran == 0 {
        eprintln!("SKIP: no release images configured");
    }
}

/// Analysis picks must equal the choices a full image build makes for the same seed.
#[cfg(feature = "devtools")]
#[test]
fn picks_only_matches_the_full_build() {
    let Some(path) = std::env::var_os("DW4_RETAIL_USA_ISO").map(PathBuf::from) else {
        eprintln!("SKIP: DW4_RETAIL_USA_ISO not configured");
        return;
    };
    let iso = std::fs::read(path).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let full_picks = tmp.path().join("full.json");
    for (seed, bosses) in [(24, true), (99, false)] {
        let options = |picks_only| RandomOptions {
            seed,
            donors: dw4vhp_core::crown::RANDOM_DONORS,
            warp: None,
            only: None,
            random_bosses: bosses,
            picks_only,
        };
        // DW4_PICKS_OUT is read only by this test; the other tests do not set it.
        std::env::set_var("DW4_PICKS_OUT", &full_picks);
        CrownPlan::build_random(&iso, options(false)).unwrap();
        std::env::remove_var("DW4_PICKS_OUT");
        let full: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&full_picks).unwrap()).unwrap();
        let picks =
            CrownPlan::random_picks(&iso, false, false, false, 0, options(true), true).unwrap();
        assert_eq!(full, picks, "seed {seed}");
        assert!(CrownPlan::build_random(&iso, options(true)).is_err());
    }
}
