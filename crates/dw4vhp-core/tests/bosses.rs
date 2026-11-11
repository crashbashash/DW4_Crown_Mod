//! Opt-in full-image checks use the same selection and production library as the desktop.
use dw4vhp_core::{
    options::PatchSelection,
    patch::{patch_file, PatchOptions},
};
use std::path::PathBuf;

#[test]
#[ignore]
fn build_optional_boss_image_through_app_selection() {
    let input = PathBuf::from(std::env::var_os("DW4_APP_TEST_INPUT").expect("source image"));
    let output = PathBuf::from(std::env::var_os("DW4_APP_TEST_OUTPUT").expect("test destination"));
    let selected = |name| std::env::var(name).as_deref() == Ok("1");
    let bosses = !selected("DW4_APP_TEST_BOSSES_OFF");
    let selection = PatchSelection {
        crown_mod: selected("DW4_APP_TEST_CROWN"),
        crown_drops: selected("DW4_APP_TEST_DROPS"),
        four_player_spawns: selected("DW4_APP_TEST_FOUR_PLAYERS"),
        random_crown_colours: selected("DW4_APP_TEST_COLOURS"),
        random_enemies: true,
        random_bosses: bosses,
        enemy_seed: Some(1520),
        solo_buttons: selected("DW4_APP_TEST_SOLO_BUTTONS"),
        fix_death_softlock: !selected("DW4_APP_TEST_DEATH_FIX_OFF"),
    };
    let mut plan = selection.into_plan().unwrap();
    if plan.random_crown_colours {
        plan.crown_colour_seed = Some(1337);
    }
    let outcome = patch_file(
        &input,
        &output,
        &plan,
        &PatchOptions { overwrite: false },
        &mut |_| {},
    )
    .unwrap();
    let metadata = dw4vhp_core::signature::read_raw(&output).unwrap().unwrap();
    assert_eq!(metadata["options"]["randomBosses"], bosses);
    assert_eq!(metadata["options"]["crownMod"], plan.crown_mod);
    if bosses {
        assert_eq!(metadata["randomEnemies"]["bosses"]["encounters"], 20);
    } else {
        assert!(metadata["randomEnemies"]["bosses"].is_null());
    }
    std::fs::write(
        output.with_extension("iso.audit.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":outcome.bytes,"md5":outcome.md5,"signature":metadata}),
        )
        .unwrap(),
    )
    .unwrap();
    eprintln!("APP IMAGE VERIFIED {} {}", output.display(), outcome.md5);
}

#[test]
fn library_refuses_bosses_without_enemies_before_opening_or_writing_files() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.iso");
    let error = patch_file(
        &dir.path().join("absent.iso"),
        &output,
        &dw4vhp_core::plan::PatchPlan {
            random_bosses: true,
            ..Default::default()
        },
        &PatchOptions { overwrite: false },
        &mut |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("requires Random enemies"));
    assert!(!output.exists());
}
