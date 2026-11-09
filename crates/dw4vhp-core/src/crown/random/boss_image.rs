//! Full-image boss regression checks through the public patch pipeline.
use super::boss::*;
use super::*;
use serde_json::json;

/// Build through the ordinary checked writer, including its retail signature,
/// localized vanilla stat variants and complete archive byte verification.
#[test]
#[ignore]
fn build_boss_test_image() {
    use crate::{
        patch::{patch_file, PatchOptions},
        plan::PatchPlan,
    };
    let input = std::path::PathBuf::from(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO"));
    let output = std::path::PathBuf::from(
        env::var_os("DW4_BOSS_TEST_IMAGE").expect("test image destination"),
    );
    assert!(env::var_os("DW4_BOSS_TEST_ROSTER").is_some());
    let seed = env::var("DW4_BOSS_TEST_SEED")
        .ok()
        .map(|s| s.parse().unwrap())
        .unwrap_or(1520);
    let plan = PatchPlan {
        crown_mod: false,
        random_enemies: true,
        random_bosses: true,
        enemy_seed: Some(seed),
        ..PatchPlan::default()
    };
    let mut last = None;
    let outcome = patch_file(
        &input,
        &output,
        &plan,
        &PatchOptions { overwrite: false },
        &mut |p| {
            let update = (p.phase, p.done * 10 / p.total.max(1));
            if last != Some(update) {
                eprintln!("BUILD {:?}: {}/{}", p.phase, p.done, p.total);
                last = Some(update);
            }
        },
    )
    .unwrap();
    let signed = crate::signature::read_raw(&output).unwrap().unwrap();
    assert_eq!(signed["options"]["crownMod"], false);
    assert_eq!(signed["options"]["randomEnemies"], true);
    assert_eq!(signed["options"]["randomBosses"], true);
    assert_eq!(signed["randomEnemies"]["bosses"]["encounters"], 9);
    let info = json!({"md5": outcome.md5, "bytes": outcome.bytes, "signature": signed});
    std::fs::write(
        output.with_extension("iso.audit.json"),
        serde_json::to_vec_pretty(&info).unwrap(),
    )
    .unwrap();
    eprintln!(
        "BOSS IMAGE VERIFIED {} ({} bytes)",
        output.display(),
        outcome.bytes
    );
}

#[test]
#[ignore]
fn boss_retirement_preserves_shared_draw_scenes() {
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO")).unwrap();
    let release = crate::release::detect(&iso).unwrap();
    let (at, size) = if release.is_gamecube() {
        let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
        (f.offset as usize, f.size as usize)
    } else {
        let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        (f.lba as usize * 2048, f.size as usize)
    };
    let afs = &iso[at..at + size];
    let entry = |i| {
        let at = u32at(afs, 8 + 8 * i).unwrap() as usize;
        let size = u32at(afs, 12 + 8 * i).unwrap() as usize;
        &afs[at..at + size]
    };
    // These three substitutions produced live actors and shadows while their
    // model switches lost the scene that was named after the original boss.
    for (destination, donor) in [(187, 265), (210, 425), (265, 187)] {
        let arena = ARENAS.iter().find(|a| a.entry == destination).unwrap();
        let to = ARENAS.iter().find(|a| a.entry == donor).unwrap().model;
        let out = prepare_model(entry(destination), entry(donor), arena, to, 1).unwrap();
        let p = Pack::parse(&out).unwrap();
        if destination == 210 && to == "e_blosso" {
            let constants = lua_names::constants(script(&p, arena.intro).unwrap()).unwrap();
            assert!(constants.iter().any(|c| c.text == b"SQ_ENTR"));
            assert!(!constants
                .iter()
                .any(|c| c.text == b"SQ_NENTR" || c.text == b"SQ_WALKIN"));
        }
        assert!(
            graft::find_named(&p, "beLuaData", &format!("ded_{}.lua", arena.model))
                .unwrap()
                .is_some(),
            "native arena callback alias must remain registered"
        );
        for (kind, name) in hud_names(&p, arena.model).unwrap() {
            assert_eq!(hud_names(&p, to).unwrap().get(kind), Some(&name));
        }
        let retired = super::super::reclaim::unused(&out, arena.model)
            .unwrap()
            .unwrap_or(out);
        let p = Pack::parse(&retired).unwrap();
        assert!(
            graft::model_is_drawn(&p, to).unwrap(),
            "{destination}: {to} lost its draw scene"
        );
    }
}

#[test]
#[ignore]
fn probe_boss_image_compatibility() {
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let afs = &iso[file.lba as usize * 2048..file.lba as usize * 2048 + file.size as usize];
    let data = |e| {
        let at = u32at(afs, 8 + 8 * e).unwrap() as usize;
        let len = u32at(afs, 12 + 8 * e).unwrap() as usize;
        &afs[at..at + len]
    };
    for arena in ARENAS {
        for donor in ARENAS {
            if arena.model == donor.model {
                continue;
            }
            let result = prepare_model(
                data(arena.entry),
                data(donor.entry),
                arena,
                donor.model,
                if arena.entry == 319 { 3 } else { 1 },
            );
            eprintln!(
                "BOSS FIT {} <- {}: {:?}",
                arena.entry,
                donor.model,
                result.as_ref().map(|b| b.len())
            );
        }
    }
}

#[test]
#[ignore]
fn probe_final_boss_resources() {
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").unwrap()).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let afs = &iso[file.lba as usize * 2048..file.lba as usize * 2048 + file.size as usize];
    for arena in ARENAS {
        let at = u32at(afs, 8 + 8 * arena.entry).unwrap() as usize;
        let len = u32at(afs, 12 + 8 * arena.entry).unwrap() as usize;
        let p = Pack::parse(&afs[at..at + len]).unwrap();
        eprintln!(
            "RESOURCE {} IDs {} dirs {} native scene {:?}",
            arena.entry,
            p.ids.len(),
            p.dirs.len(),
            graft::model_scene(&p, arena.model)
        );
        for id in 0..p.ids.len() as u32 {
            if p.class(id, "igGroup").is_ok()
                && graft::name_of(&p, p.rec(id).unwrap()) == Some(b"EM01")
            {
                eprintln!(
                    " EM01 {id} {:?}",
                    p.rec(id)
                        .unwrap()
                        .fields
                        .keys()
                        .map(|f| (*f, p.val(id, *f).ok()))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    if let Some(clean) = env::var_os("DW4_BOSS_CLEAN_ISO") {
        let clean = std::fs::read(clean).unwrap();
        let file = crate::iso9660::find_file(&clean, "NDMW.AFS").unwrap();
        let source =
            &clean[file.lba as usize * 2048..file.lba as usize * 2048 + file.size as usize];
        let data = |bytes: &[u8], index: usize| {
            let offset = u32at(bytes, 8 + 8 * index).unwrap() as usize;
            let size = u32at(bytes, 12 + 8 * index).unwrap() as usize;
            (offset, size)
        };
        let (offset, size) = data(afs, 187);
        let (src, len) = data(source, 210);
        let result = prepare_model(
            &afs[offset..offset + size],
            &source[src..src + len],
            &ARENAS[0],
            "e_mammos",
            1,
        );
        eprintln!("FINAL PACK GRAFT {:?}", result.as_ref().map(|b| b.len()));
        assert!(result.is_ok());
    }
}

// Resolve the written MODEL database independently of the planner's selected rows.
// This checks patrol exclusion across every floor, including newly allocated variants.
fn verify_dungeon_patrols(source: &[u8], written: &[u8], roster: &serde_json::Value) {
    fn file<'a>(iso: &'a [u8], name: &str) -> &'a [u8] {
        let (at, size) = if crate::release::detect(iso).unwrap().is_gamecube() {
            let f = crate::gcm::find_file(iso, name).unwrap();
            (f.offset as usize, f.size as usize)
        } else {
            let f = crate::iso9660::find_file(iso, name).unwrap();
            (f.lba as usize * 2048, f.size as usize)
        };
        &iso[at..at + size]
    }
    fn entry(afs: &[u8], index: usize) -> &[u8] {
        let at = u32at(afs, 8 + 8 * index).unwrap() as usize;
        let size = u32at(afs, 12 + 8 * index).unwrap() as usize;
        &afs[at..at + size]
    }
    let manifest = Pack::parse(file(source, "NDMW.IGB")).unwrap();
    let mut files = Vec::new();
    for id in 0..manifest.ids.len() as u32 {
        if manifest.class(id, "igAdxAfsFile").is_err() || manifest.val(id, 3).unwrap() == NULL {
            continue;
        }
        let path = manifest
            .array(manifest.val(id, 3).unwrap())
            .unwrap()
            .into_iter()
            .map(|node| {
                graft::name_of(&manifest, manifest.rec(node).unwrap())
                    .unwrap()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        files.push((
            manifest.val(id, 4).unwrap(),
            manifest.val(id, 5).unwrap() as usize,
            graft::name_of(&manifest, manifest.rec(id).unwrap())
                .unwrap()
                .to_vec(),
            path,
        ));
    }
    let original = intro::archive(source).unwrap();
    let output = intro::archive(written).unwrap();
    let area = file(written, "AREA.AFS");
    let mut verified = 0;
    for (tier, report) in roster["tiers"].as_array().unwrap().iter().enumerate() {
        for arena in ARENAS {
            let selection = report["bosses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["stage"] == arena.entry)
                .unwrap();
            assert_eq!(selection["patrolFloors"], serde_json::json!(arena.floors));
            let mut actual = BTreeSet::new();
            for &floor in arena.floors {
                let gens = (floor + 1..=floor + 7)
                    .filter(|&i| {
                        let b = entry(original, i);
                        b.len() < 1_500_000
                            && memchr::memmem::find(b, b"beGeneraterData\0").is_some()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(gens.len(), 3);
                let paths = files
                    .iter()
                    .filter(|f| f.0 == 0 && f.1 == gens[1])
                    .map(|f| &f.3)
                    .collect::<BTreeSet<_>>();
                assert_eq!(paths.len(), 1);
                let path = *paths.first().unwrap();
                let language = files
                    .iter()
                    .filter(|f| {
                        f.0 == 1 && &f.3 == path && f.2.eq_ignore_ascii_case(b"usa_pack.igb")
                    })
                    .collect::<Vec<_>>();
                assert_eq!(language.len(), 1);
                let p = Pack::parse(entry(area, language[0].1)).unwrap();
                let mut models = Vec::new();
                for id in 0..p.ids.len() as u32 {
                    if p.class(id, "beDataBase").is_err() {
                        continue;
                    }
                    let columns = p.array(p.val(id, 12).unwrap()).unwrap();
                    if columns.len() == 1
                        && graft::name_of(&p, p.rec(columns[0]).unwrap()) == Some(b"MODEL")
                    {
                        let rows = p.array(p.val(id, 13).unwrap()).unwrap();
                        if rows.len() == 649 {
                            models.push(rows);
                        }
                    }
                }
                assert_eq!(models.len(), 1);
                let pack = gens[[1, 0, 2][tier]];
                for g in generators(&Pack::parse(entry(output, pack)).unwrap()).unwrap() {
                    if floor == arena.entry && g.name == arena.generator {
                        continue;
                    }
                    if let Some(&model) = models[0].get(g.row as usize) {
                        if model == NULL {
                            continue;
                        }
                        let name = graft::name_of(&p, p.rec(model).unwrap()).unwrap();
                        let name = std::str::from_utf8(name).unwrap();
                        if ROW_GROUPS.iter().any(|g| g.model == name) {
                            actual.insert(name.to_string());
                        }
                    }
                }
            }
            let claimed = selection["patrols"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                actual, claimed,
                "dungeon patrol roster {} tier {tier}",
                arena.entry
            );
            if selection["changed"] == true {
                assert!(!actual.contains(selection["boss"].as_str().unwrap()));
            }
            verified += 1;
        }
    }
    assert_eq!(verified, 27);
    eprintln!(
        "BOSS DUNGEON PATROLS: all 27 choices checked against every floor's written MODEL database"
    );
}

#[test]
#[ignore]
fn verify_boss_test_image_stats_and_rosters() {
    let source = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO")).unwrap();
    let written = std::fs::read(env::var_os("DW4_BOSS_TEST_IMAGE").expect("test image")).unwrap();
    let roster: serde_json::Value = serde_json::from_slice(
        &std::fs::read(env::var_os("DW4_BOSS_TEST_ROSTER").expect("boss roster")).unwrap(),
    )
    .unwrap();
    intro::verify_image(&source, &written, &roster).unwrap();
    if crate::signature::from_image(&written).unwrap().unwrap()["options"]["randomBosses"] == true {
        verify_dungeon_patrols(&source, &written, &roster);
    }
    let release = crate::release::detect(&source).unwrap();
    let before = EnemyTable::read(&source, &release.layout_in(&source).unwrap()).unwrap();
    // Original anchors remain in the retained AREA bytes. The relocated active
    // boss databases are independently compared above after message-bank growth.
    let after = EnemyTable::read(&written, &release.layout_in(&source).unwrap()).unwrap();
    let metadata = crate::signature::from_image(&written).unwrap().unwrap();
    let balance = crate::plan::PatchPlan {
        crown_mod: metadata["options"]["crownMod"].as_bool().unwrap(),
        random_crown_colours: metadata["options"]["randomCrownColours"].as_bool().unwrap(),
        crown_colour_seed: metadata["options"]["crownColourSeed"]
            .as_u64()
            .map(|v| v as u32),
        ..Default::default()
    };
    let expected = crate::plan::transform(
        &before,
        &balance,
        &crate::ratio::RatioVector::derive(&before),
    )
    .0;
    let native_report = crate::disc::inspect_retail(&std::path::PathBuf::from(
        env::var_os("DW4_BOSS_AUDIT_ISO").unwrap(),
    ))
    .unwrap();
    let base_regions = crate::patch::plan_regions(
        &source,
        &native_report,
        &balance,
        &release.layout_in(&source).unwrap(),
    )
    .unwrap()
    .0;
    let live = (0..before.len())
        .filter(|&row| before.hp[row] > 0)
        .collect::<Vec<_>>();
    for &row in &live {
        assert_eq!(expected.hp[row], after.hp[row], "native HP row {row}");
        assert_eq!(expected.stat[row], after.stat[row], "native stat row {row}");
        assert_eq!(expected.crit[row], after.crit[row]);
        assert_eq!(expected.para[row], after.para[row]);
        assert_eq!(expected.rarity[row], after.rarity[row]);
    }
    let configured = env::var_os("DW4_BOSS_COMPARISON_IMAGE").map(|p| std::fs::read(p).unwrap());
    let original_afs = intro::archive(configured.as_deref().unwrap_or(&source)).unwrap();
    let output_afs = intro::archive(&written).unwrap();
    fn entry(afs: &[u8], index: usize) -> &[u8] {
        let offset = u32at(afs, 8 + 8 * index).unwrap() as usize;
        let size = u32at(afs, 12 + 8 * index).unwrap() as usize;
        &afs[offset..offset + size]
    }
    let mut protected_packs = 0;
    for world in [730, 737, 744, 751, 758, 765] {
        for index in world + 1..=world + 7 {
            let raw = entry(original_afs, index);
            if raw.len() < 1_500_000 && memchr::memmem::find(raw, b"beGeneraterData\0").is_some() {
                assert_eq!(
                    raw,
                    entry(output_afs, index),
                    "world boss generator pack {index}"
                );
                protected_packs += 1;
            }
        }
    }
    let mut boss_rows = 0;
    let mut block_checks = 0;
    for arena in ARENAS {
        let packs = (arena.entry + 1..=arena.entry + 7)
            .filter(|&index| {
                let raw = entry(original_afs, index);
                raw.len() < 1_500_000 && memchr::memmem::find(raw, b"beGeneraterData\0").is_some()
            })
            .collect::<Vec<_>>();
        assert_eq!(packs.len(), 3);
        let mut request = super::super::rewards::StageRewards::new(packs.clone(), Vec::new());
        for (difficulty, pack) in [packs[1], packs[0], packs[2]].into_iter().enumerate() {
            let selected = roster["tiers"][difficulty]["bosses"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["stage"].as_u64() == Some(arena.entry as u64))
                .unwrap();
            let to = selected["boss"].as_str().unwrap();
            let patrols = selected["patrols"].as_array().unwrap();
            let original_pack = Pack::parse(entry(original_afs, pack)).unwrap();
            let output_pack = Pack::parse(entry(output_afs, pack)).unwrap();
            let id = original_pack
                .named("beGeneraterData", arena.generator)
                .unwrap();
            let old_row = original_pack.val(id, graft::GENERATOR_ROW).unwrap();
            let new_row = output_pack.val(id, graft::GENERATOR_ROW).unwrap() as usize;
            let expected_generator = original_pack
                .rec(id)
                .unwrap()
                .patch(original_pack.b, &[(graft::GENERATOR_ROW, new_row as u32)])
                .unwrap();
            assert_eq!(
                expected_generator,
                output_pack.b[output_pack.rec(id).unwrap().range.clone()],
                "boss generator callbacks, drops or counts changed: {} tier {difficulty}",
                arena.entry
            );
            let mut queue = vec![id];
            let mut seen = BTreeSet::new();
            while let Some(dependency) = queue.pop() {
                if !seen.insert(dependency) {
                    continue;
                }
                let (original_range, output_range) =
                    if original_pack.ids[dependency as usize].is_some() {
                        let record = original_pack.rec(dependency).unwrap();
                        for (field, _) in graft::ref_slots(&original_pack, record) {
                            let target = record.val(original_pack.b, field).unwrap();
                            if target != NULL {
                                queue.push(target);
                            }
                        }
                        // Generator item/player lists contain object IDs. The
                        // adjacent position/rotation lists contain raw vectors
                        // and floats and must never be interpreted as IDs.
                        let class = &original_pack.classes[record.class].name;
                        if class.starts_with("beGenerater") && class.ends_with("List") {
                            queue.extend(
                                original_pack
                                    .array(dependency)
                                    .unwrap()
                                    .into_iter()
                                    .filter(|&v| v != NULL),
                            );
                        }
                        (
                            record.range.clone(),
                            output_pack.rec(dependency).unwrap().range.clone(),
                        )
                    } else {
                        (
                            original_pack.memory[dependency as usize].clone().unwrap(),
                            output_pack.memory[dependency as usize].clone().unwrap(),
                        )
                    };
                if dependency == id {
                    continue;
                }
                assert_eq!(
                    original_pack.b[original_range], output_pack.b[output_range],
                    "boss drop/callback dependency changed: {} tier {difficulty} ID {dependency}",
                    arena.entry
                );
            }
            if to == arena.model {
                if !metadata["options"]["randomBosses"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    assert_eq!(family(new_row as u32), Some(arena.model));
                }
                assert_eq!(old_row as usize, new_row);
                continue;
            }
            assert!(!patrols.iter().any(|m| m.as_str() == Some(to)));
            assert_eq!(
                before.hp[new_row], 0,
                "boss variants must use unfilled rows"
            );
            let combat = nearest_row(&before, old_row, to).unwrap();
            request.rows.insert(
                new_row,
                super::super::rewards::Variant {
                    combat: combat as usize,
                    reward: old_row as usize,
                },
            );
            boss_rows += 1;
        }
        if request.rows.is_empty() {
            continue;
        }
        let requests = BTreeMap::from([(arena.entry, request.clone())]);
        let plan = super::super::rewards::Plan::build(&source, &before, &requests).unwrap();
        let mut balanced_regions = base_regions.clone();
        for region in plan.apply(&mut balanced_regions, &expected).unwrap() {
            let stride = region.bytes.len() / before.len();
            assert!([3, 4, 24].contains(&stride));
            for &row in request.rows.keys() {
                let start = row * stride;
                let target = region.offset as usize + start;
                assert_eq!(
                    region.bytes[start..start + stride],
                    written[target..target + stride],
                    "boss stats must match the configured encounter: stage {} row {row}",
                    arena.entry
                );
                block_checks += 1;
            }
        }
        for region in plan.models {
            for &row in request.rows.keys() {
                let start = row * 4;
                let target = region.offset as usize + start;
                assert_eq!(
                    region.bytes[start..start + 4],
                    written[target..target + 4],
                    "boss model row differs"
                );
            }
        }
    }
    assert!(protected_packs >= 18);
    if metadata["options"]["randomBosses"]
        .as_bool()
        .unwrap_or(false)
    {
        assert!(boss_rows >= 6);
    } else {
        assert_eq!(boss_rows, 0);
    }
    for tier in roster["tiers"].as_array().unwrap() {
        let bosses = tier["bosses"].as_array().unwrap();
        let original = bosses
            .iter()
            .map(|b| b["original"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        let selected = bosses
            .iter()
            .map(|b| b["boss"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(original, selected);
        assert_eq!(bosses.len(), 9);
    }
    eprintln!("BOSS VERIFY: {} native stat rows match selected balance; {boss_rows} boss swaps; {block_checks} localized stat checks; {protected_packs} fixed world-boss generator packs", live.len());
    let temp = tempfile::tempdir().unwrap();
    let restored = temp.path().join("restored.iso");
    crate::update::restore(&written, &metadata, &restored, &mut |_, _| {}).unwrap();
    use std::io::Read;
    let mut restored = std::fs::File::open(restored).unwrap();
    assert_eq!(restored.metadata().unwrap().len(), source.len() as u64);
    let mut buffer = vec![0; 8 << 20];
    for chunk in source.chunks(buffer.len()) {
        restored.read_exact(&mut buffer[..chunk.len()]).unwrap();
        assert_eq!(
            &buffer[..chunk.len()],
            chunk,
            "recovery differs from the source disc"
        );
    }
    eprintln!(
        "BOSS RECOVERY IMAGE: restored source disc byte-for-byte, including both archive extents"
    );
}
