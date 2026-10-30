use super::*;

fn table() -> EnemyTable {
    let rows = 650;
    let mut t = EnemyTable {
        hp: vec![0; rows],
        stat: vec![[0; 12]; rows],
        crit: vec![0; rows],
        para: vec![0; rows],
        rarity: vec![0; rows],
    };
    // goburi tier rows 0..3 and a crown row 379; mummy tier rows 114..117 and a crown row 394
    for (r, hp, atk) in [
        (0, 50, 16),
        (1, 58, 18),
        (2, 74, 54),
        (3, 77, 56),
        (114, 200, 66),
        (115, 229, 78),
        (116, 241, 84),
        (117, 268, 98),
    ] {
        t.hp[r] = hp;
        t.stat[r][0] = atk;
    }
    t.hp[379] = 250;
    t.stat[379][0] = 78;
    t.rarity[379] = 1;
    t.hp[394] = 1000;
    t.stat[394][0] = 332;
    t.rarity[394] = 1;
    t
}

#[test]
fn shared_generators_need_the_sum_and_tiers_are_alternatives() {
    let demand = shared_capacity(
        [
            vec![
                ("e_goburi", "e_tyrano", 4),
                ("e_goburi", "e_tyrano", 4),
                ("e_ogre", "e_tyrano", 1),
            ],
            vec![("e_goburi", "e_tyrano", 2), ("e_goburi", "e_tyrano", 2)],
        ],
        &BTreeMap::new(),
    );
    assert_eq!(
        demand["e_tyrano"], 9,
        "two four-enemy waves plus the nearby singleton"
    );
    let native = BTreeMap::from([("e_mummy".to_string(), 12)]);
    let demand = shared_capacity(
        [vec![("e_goburi", "e_mummy", 4), ("e_mummy", "e_mummy", 1)]],
        &native,
    );
    assert_eq!(
        demand["e_mummy"], 16,
        "keep native capacity for script summons as well as new generators"
    );
}

#[test]
fn cannon_and_train_entrances_survive_randomisation() {
    let Some(path) = env::var_os("DW4_RETAIL_USA_ISO") else {
        return;
    };
    let iso = std::fs::read(path).unwrap();
    let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let base = file.lba as usize * 2048;
    let afs = &iso[base..base + file.size as usize];
    let entries = (0..u32at(afs, 4).unwrap() as usize)
        .map(|i| {
            (
                u32at(afs, 8 + i * 8).unwrap() as usize,
                u32at(afs, 12 + i * 8).unwrap() as usize,
            )
        })
        .collect::<Vec<_>>();
    let data = |e: usize| {
        let (o, l) = entries[e];
        &afs[o..o + l]
    };
    let layout = crate::release::detect(&iso)
        .unwrap()
        .layout_in(&iso)
        .unwrap();
    let mut r = Randomiser::scan(
        afs,
        &entries,
        EnemyTable::read(&iso, &layout).unwrap(),
        Options {
            seed: 7,
            donors: 0,
            warp: None,
            only: None,
        },
    )
    .unwrap();
    let train = *r
        .stages
        .iter()
        .find(|(_, s)| s.gens.contains(&659))
        .unwrap()
        .0;
    let mut protected = 0;
    let mut changed = 0;
    for e in [614, train] {
        r.stage(e, data(e).to_vec(), afs, &entries).unwrap();
        for &pack in &r.stages[&e].gens {
            let original = Pack::parse(data(pack)).unwrap();
            let out = r
                .retarget(pack, data(pack))
                .unwrap()
                .unwrap_or_else(|| data(pack).to_vec());
            let out = Pack::parse(&out).unwrap();
            let after = generators(&out).unwrap();
            for g in generators(&original).unwrap() {
                let new = after.iter().find(|n| n.name == g.name).unwrap();
                if g.seq {
                    assert_eq!(new.row, g.row, "entrance {} in {pack}", g.name);
                    assert!(new.seq);
                    protected += 1;
                } else if new.row != g.row {
                    changed += 1;
                }
            }
        }
    }
    assert!(
        protected > 50,
        "cover the cannon and train entrances on every difficulty"
    );
    assert!(changed > 0, "ordinary spawns still randomise");
}

#[test]
fn mangrove_randomises_intro_and_boss_waves_with_matching_scripts() {
    for variable in [
        "DW4_RETAIL_USA_ISO",
        "DW4_RETAIL_EUROPE_ISO",
        "DW4_RETAIL_JAPAN_ISO",
        "DW4_RETAIL_GC_USA_ISO",
        "DW4_RETAIL_GC_JAPAN_ISO",
    ] {
        let Some(path) = env::var_os(variable) else {
            continue;
        };
        let iso = std::fs::read(path).unwrap();
        let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
            let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
            (f.offset as usize, f.size as usize)
        } else {
            let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
            (f.lba as usize * 2048, f.size as usize)
        };
        let afs = &iso[base..base + size];
        let entries: Vec<_> = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect();
        let data = |i: usize| {
            let (o, n) = entries[i];
            &afs[o..o + n]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        for seed in [1278675365, 1520] {
            let mut r = Randomiser::scan(
                afs,
                &entries,
                EnemyTable::read(&iso, &layout).unwrap(),
                Options {
                    seed,
                    donors: DONORS,
                    warp: None,
                    only: Some(vec![686]),
                },
            )
            .unwrap();
            let stage = r.stages[&686].clone();
            let initial = materialize(data(686), &expand(data(686)).unwrap()).unwrap();
            let out = r.stage(686, initial, afs, &entries).unwrap();
            let intro_model = family(r.plans[&stage.gens[0]].rows["GENE-165"])
                .expect("introduction has an enemy model");
            assert_ne!(intro_model, "e_torta", "{variable}, seed {seed}");
            assert!(!stage.pinned.contains("GENE-165"));
            let waves: BTreeMap<String, String> = MANGROVE_WAVES
                .iter()
                .map(|&(generator, original)| {
                    let selected = family(r.plans[&stage.gens[0]].rows[generator]).unwrap();
                    assert_ne!(selected, original, "{variable}, seed {seed}, {generator}");
                    assert!(!stage.pinned.contains(generator));
                    (original.into(), selected.into())
                })
                .collect();
            let mut changed = 0;
            let mut protected = BTreeSet::new();
            for pack in &stage.gens {
                assert_eq!(family(r.plans[pack].rows["GENE-165"]), Some(intro_model));
                for (name, row) in &r.plans[pack].rows {
                    if name != "GENE-165" {
                        assert_ne!(
                            family(*row),
                            Some(intro_model),
                            "intro cleanup must not erase patrol {name}"
                        );
                    }
                }
                for &(generator, original) in MANGROVE_WAVES {
                    assert_eq!(
                        family(r.plans[pack].rows[generator]),
                        Some(waves[original].as_str())
                    );
                }
                let source = Pack::parse(data(*pack)).unwrap();
                let patched = r
                    .retarget(*pack, data(*pack))
                    .unwrap()
                    .unwrap_or_else(|| data(*pack).to_vec());
                let result = Pack::parse(&patched).unwrap();
                for g in generators(&source).unwrap() {
                    let id = source.named("beGeneraterData", &g.name).unwrap();
                    let new = result.val(id, graft::GENERATOR_ROW).unwrap();
                    if MANGROVE_WAVES.iter().any(|(name, _)| *name == g.name) {
                        // Counts, positions, refill modes, drops and completion scripts
                        // remain authored; only the enemy/optional native entrance change.
                        for (&field, a) in &source.rec(id).unwrap().fields {
                            if field != graft::GENERATOR_ROW && field != graft::GENERATOR_SEQ {
                                let b = result.rec(id).unwrap().field(field).unwrap();
                                assert_eq!(source.b[a.clone()], result.b[b], "wave field {field}");
                            }
                        }
                        let players = source
                            .array(source.val(id, GENERATOR_PLAYERS).unwrap())
                            .unwrap();
                        for pd in players {
                            assert_eq!(
                                source.b[source.rec(pd).unwrap().range.clone()],
                                result.b[result.rec(pd).unwrap().range.clone()]
                            );
                        }
                    }
                    if stage.pinned.contains(&g.name) || protected_entrance(&g) {
                        assert_eq!(g.row, new, "{variable}: scripted group {}", g.name);
                        let a = source.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                        let b = result.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                        assert_eq!(source.b[a], result.b[b]);
                        protected.insert(g.name);
                    } else if g.row != new {
                        changed += 1;
                    }
                }
            }
            assert!(
                changed > 0,
                "{variable}: Mangrove ordinary enemies all remained vanilla"
            );
            for name in ["GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"] {
                assert!(protected.contains(name), "missing protected group {name}");
            }
            let native = Pack::parse(data(686)).unwrap();
            let result = Pack::parse(&out).unwrap();
            for model in ["e_geko", "e_otama", "e_torta", "e_tonogeko"] {
                // Tortamon can be loaded from its separate model file.
                if graft::find_named(&native, "beModelCtrlInfo", model)
                    .unwrap()
                    .is_some()
                {
                    assert!(
                        graft::find_named(&result, "beModelCtrlInfo", model)
                            .unwrap()
                            .is_some(),
                        "native scripted model {model} must remain loaded"
                    );
                }
            }
            for id in 0..native.ids.len() as u32 {
                if native.class(id, "beLuaData").is_ok() {
                    let object = native.val(id, 3).unwrap();
                    let mem = native.val(object, 3).unwrap();
                    let name = graft::name_of(&native, native.rec(id).unwrap()).unwrap();
                    let source = &native.b[native.memory[mem as usize].clone().unwrap()];
                    let result_object = result.val(id, 3).unwrap();
                    let result_mem = result.val(result_object, 3).unwrap();
                    let actual = &result.b[result.memory[result_mem as usize].clone().unwrap()];
                    if matches!(name, b"first.lua" | b"mate00.lua" | b"tim_g_orbz03.lua") {
                        let size = native.val(object, 2).unwrap() as usize;
                        let names = if name == b"first.lua" {
                            BTreeMap::from([("e_torta".into(), intro_model.into())])
                        } else if name == b"mate00.lua" {
                            BTreeMap::from([("e_otama".into(), waves["e_otama"].clone())])
                        } else {
                            waves.clone()
                        };
                        let expected = lua_names::rename(&source[..size], &names).unwrap().unwrap();
                        assert_eq!(
                            actual[..result.val(result_object, 2).unwrap() as usize],
                            expected
                        );
                        assert!(graft::find_named(&result, "beModelCtrlInfo", intro_model)
                            .unwrap()
                            .is_some());
                    } else {
                        assert_eq!(source, actual, "unrelated stage script changed");
                    }
                }
            }
            eprintln!("MANGROVE {variable}, seed {seed}: intro -> {intro_model}; waves {waves:?}; {changed} rows changed; {} scripted groups and unrelated scripts preserved", protected.len());
        }
    }
}

#[test]
fn seed_1520_gamecube_humid_cave_omits_unused_imports() {
    let Some(path) = env::var_os("DW4_RETAIL_GC_USA_ISO") else {
        return;
    };
    let iso = std::fs::read(path).unwrap();
    let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
    let afs = &iso[f.offset as usize..(f.offset + f.size) as usize];
    let entries = (0..u32at(afs, 4).unwrap() as usize)
        .map(|i| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        })
        .collect::<Vec<_>>();
    let data = |e: usize| {
        let (o, l) = entries[e];
        &afs[o..o + l]
    };
    let layout = crate::release::detect(&iso)
        .unwrap()
        .layout_in(&iso)
        .unwrap();
    let mut r = Randomiser::scan(
        afs,
        &entries,
        EnemyTable::read(&iso, &layout).unwrap(),
        Options {
            seed: 1520,
            donors: DONORS,
            warp: None,
            only: Some(vec![179, 187]),
        },
    )
    .unwrap();
    let original = materialize(data(187), &expand(data(187)).unwrap()).unwrap();
    let out = r.stage(187, original.clone(), afs, &entries).unwrap();
    let p = Pack::parse(&out).unwrap();
    let stage = &r.stages[&187];
    let used: BTreeSet<_> = stage
        .gens
        .iter()
        .flat_map(|pack| r.plans[pack].rows.values())
        .filter_map(|row| family(*row))
        .collect();
    let mut imports = 0;
    for &model in REGULAR {
        let present = graft::find_named(&p, "beModelCtrlInfo", model)
            .unwrap()
            .is_some();
        if stage.residents.contains_key(model) {
            assert!(present, "native model {model} must remain loaded");
        } else {
            assert_eq!(
                present,
                used.contains(model),
                "unused import {model} must be omitted; used imports must remain"
            );
            imports += usize::from(present);
        }
    }
    assert!(imports > 0);
    assert_eq!(r.stats.copied, imports);
    for &pack in &stage.gens {
        let bytes = r.retarget(pack, data(pack)).unwrap().unwrap();
        Pack::parse(&bytes).unwrap();
    }
    // Optional private fixtures for replaying a user-supplied savestate.
    // Keep the export directory outside the source checkout.
    if let Some(dir) = env::var_os("DW4_TEST_STAGE_EXPORT") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("187.igb"), &out).unwrap();
        std::fs::write(dir.join("186.igb"), &r.sounds[&186]).unwrap();
        let first = materialize(data(179), &expand(data(179)).unwrap()).unwrap();
        let first = r.stage(179, first, afs, &entries).unwrap();
        std::fs::write(dir.join("179.igb"), first).unwrap();
        std::fs::write(dir.join("178.igb"), &r.sounds[&178]).unwrap();
    }
    eprintln!(
        "Humid Cave B2: {} -> {} model bytes; only assigned import families retained",
        original.len(),
        out.len()
    );
}

#[test]
fn seed_1520_gamecube_shared_capacity_covers_every_generator() {
    let Some(path) = env::var_os("DW4_RETAIL_GC_USA_ISO") else {
        return;
    };
    let iso = std::fs::read(path).unwrap();
    let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
    let afs = &iso[f.offset as usize..(f.offset + f.size) as usize];
    let entries = (0..u32at(afs, 4).unwrap() as usize)
        .map(|i| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        })
        .collect::<Vec<_>>();
    let data = |e: usize| {
        let (o, l) = entries[e];
        &afs[o..o + l]
    };
    let layout = crate::release::detect(&iso)
        .unwrap()
        .layout_in(&iso)
        .unwrap();
    // Preserve the original three-import Tyrannomon regression and also check
    // the currently shipped import limit; changing it changes this seed's choices.
    for donors in [3, DONORS] {
        let mut r = Randomiser::scan(
            afs,
            &entries,
            EnemyTable::read(&iso, &layout).unwrap(),
            Options {
                seed: 1520,
                donors,
                warp: None,
                only: Some(vec![614]),
            },
        )
        .unwrap();
        let initial = materialize(data(614), &expand(data(614)).unwrap()).unwrap();
        let before = Pack::parse(&initial).unwrap();
        let scene_blocks: Vec<_> = (0..before.ids.len() as u32)
            .filter(|&id| before.class(id, "igNodeList").is_ok())
            .map(|id| (id, before.val(id, 4).unwrap()))
            .collect();
        let out = r.stage(614, initial, afs, &entries).unwrap();
        let p = Pack::parse(&out).unwrap();
        for (list, block) in scene_blocks {
            assert_eq!(
                p.val(list, 4).unwrap(),
                block,
                "scene list {list}: imported enemies must not orphan the original backing block"
            );
        }
        let mut checked = 0;
        for &pack in &r.stages[&614].gens {
            let retargeted = r.retarget(pack, data(pack)).unwrap().unwrap();
            let after = generators(&Pack::parse(&retargeted).unwrap()).unwrap();
            let mut demand = BTreeMap::<&str, usize>::new();
            for g in &after {
                // Private EXP rows carry the replacement's combat family.
                let combat = r.rewards[&614]
                    .rows
                    .get(&(g.row as usize))
                    .map_or(g.row, |variant| variant.combat as u32);
                if let Some(m) = family(combat) {
                    if REGULAR.contains(&m) {
                        *demand.entry(m).or_default() += g.most as usize;
                    }
                }
            }
            for (model, count) in demand {
                if r.plans[&pack]
                    .rows
                    .values()
                    .any(|&row| family(row) == Some(model))
                {
                    assert!(
                        super::super::capacity::slots(&p, model).unwrap() >= count,
                        "{pack} {model}: need {count}"
                    );
                    checked += 1;
                }
            }
            // Maxima must still be the authored maxima, for all four party sizes.
            let original = Pack::parse(data(pack)).unwrap();
            let patched = Pack::parse(&retargeted).unwrap();
            for (id, ix) in original.ids.iter().enumerate() {
                if let Some(ix) = ix {
                    if original.classes[original.objects[*ix].class].name == "beGeneraterPlayerData"
                    {
                        assert_eq!(
                            original.val(id as u32, 3).unwrap(),
                            patched.val(id as u32, 3).unwrap()
                        );
                    }
                }
            }
        }
        assert!(checked >= r.stages[&614].gens.len());
        if donors == 3 {
            assert!(checked >= 9);
            assert!(
                super::super::capacity::slots(&p, "e_tyrano").unwrap() > 4,
                "the imported four-slot Tyrannomon pool must grow"
            );
        }
        let mut total = 4;
        for (id, ix) in p.ids.iter().enumerate() {
            if let Some(ix) = ix {
                let rec = &p.objects[*ix];
                if p.classes[rec.class].name == "beModelCtrlInfo"
                    && rec
                        .field(7)
                        .is_ok_and(|f| p.b[f.start + 4..f.end].starts_with(b"KIND_ENEMY\0"))
                {
                    total += p.array(p.val(id as u32, 6).unwrap()).unwrap().len();
                }
            }
        }
        assert!(super::super::capacity::slots(&p, "i_rareicon").unwrap() >= total);
        eprintln!(
        "seed 1520, donors {donors}: {checked} family/tier capacity checks; Tyrannomon slots {}, crown slots {}",
        super::super::capacity::slots(&p, "e_tyrano").unwrap(),
        super::super::capacity::slots(&p, "i_rareicon").unwrap()
    );
    }
}

#[test]
fn every_pool_model_has_an_effect_number() {
    for m in REGULAR {
        assert!(
            graft::EFFECT_NUMBERS.iter().any(|(n, _)| n == m),
            "{m} has no effect number"
        );
    }
}

#[test]
fn nearest_row_scales_by_hp_and_attack() {
    let t = table();
    assert_eq!(nearest_row(&t, 0, "e_mummy"), Some(114));
    assert_eq!(nearest_row(&t, 3, "e_mummy"), Some(114));
    assert_eq!(nearest_row(&t, 3, "e_goburi"), Some(3));
}

#[test]
fn combat_matching_ignores_exp_and_breaks_ties_by_row() {
    let mut t = table();
    t.hp[0] = 100;
    t.stat[0][0] = 100;
    t.stat[0][crate::table::EXP_COLUMN] = 100;
    for row in 114..=117 {
        t.hp[row] = 100;
        t.stat[row][0] = 100;
        t.stat[row][crate::table::EXP_COLUMN] = 1;
    }
    t.stat[115][crate::table::EXP_COLUMN] = 100;
    assert_eq!(nearest_row(&t, 0, "e_mummy"), Some(114));
    t.hp[114] = 1000;
    assert_eq!(nearest_row(&t, 0, "e_mummy"), Some(115));
    t.hp[115] = 0;
    assert_eq!(nearest_row(&t, 0, "e_mummy"), Some(116));
}

#[test]
fn nearest_row_keeps_the_crown_rarity() {
    let t = table();
    assert_eq!(nearest_row(&t, 379, "e_mummy"), Some(394));
    assert_eq!(nearest_row(&t, 379, "e_goburi"), Some(379));
    assert_eq!(nearest_row(&t, 379, "e_ogre"), None);
}

#[test]
fn the_stream_is_deterministic_and_shuffles() {
    let mut a: Vec<u32> = (0..20).collect();
    let mut b = a.clone();
    Rng::new(7, 3).shuffle(&mut a);
    Rng::new(7, 3).shuffle(&mut b);
    assert_eq!(a, b);
    let mut c: Vec<u32> = (0..20).collect();
    Rng::new(8, 3).shuffle(&mut c);
    assert_ne!(a, c);
    a.sort();
    assert_eq!(a, (0..20).collect::<Vec<_>>());
}

#[test]
fn sukamon_intro_retargets_wait_and_control_on_every_release() {
    for variable in [
        "DW4_RETAIL_USA_ISO",
        "DW4_RETAIL_EUROPE_ISO",
        "DW4_RETAIL_JAPAN_ISO",
        "DW4_RETAIL_GC_USA_ISO",
        "DW4_RETAIL_GC_JAPAN_ISO",
    ] {
        let Some(path) = env::var_os(variable) else {
            continue;
        };
        let iso = std::fs::read(path).unwrap();
        let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
            let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
            (f.offset as usize, f.size as usize)
        } else {
            let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
            (f.lba as usize * 2048, f.size as usize)
        };
        let afs = &iso[base..base + size];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let data = |i: usize| {
            let (o, n) = entries[i];
            &afs[o..o + n]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        for seed in [1520, 2162128323] {
            let mut r = Randomiser::scan(
                afs,
                &entries,
                EnemyTable::read(&iso, &layout).unwrap(),
                Options {
                    seed,
                    donors: DONORS,
                    warp: None,
                    only: Some(vec![187]),
                },
            )
            .unwrap();
            let stage = r.stages[&187].clone();
            let intro = INTRO_STAGES.iter().find(|s| s.model_entry == 187).unwrap();
            assert_eq!(
                stage.gens, intro.generators,
                "{variable}: authored generator packs"
            );
            assert!(!stage.pinned.contains("GENE-01"));
            assert!(!stage.pinned_families.contains("e_scum"));
            let initial = materialize(data(187), &expand(data(187)).unwrap()).unwrap();
            let out = r.stage(187, initial, afs, &entries).unwrap();
            let replacement = family(r.plans[&stage.gens[0]].rows["GENE-01"]).unwrap();
            assert_ne!(replacement, "e_scum", "{variable}, seed {seed}");
            for pack in &stage.gens {
                assert_eq!(family(r.plans[pack].rows["GENE-01"]), Some(replacement));
                for (name, row) in &r.plans[pack].rows {
                    if name != "GENE-01" {
                        assert_ne!(
                            family(*row),
                            Some(replacement),
                            "intro freeze must not affect patrol {name}"
                        );
                    }
                }
                let source = Pack::parse(data(*pack)).unwrap();
                let changed = r.retarget(*pack, data(*pack)).unwrap().unwrap();
                let result = Pack::parse(&changed).unwrap();
                let boss = source.named("beGeneraterData", "GENE-08").unwrap();
                assert_eq!(
                    source.b[source.rec(boss).unwrap().range.clone()],
                    result.b[result.rec(boss).unwrap().range.clone()],
                    "native Blossomon boss generator changed"
                );
                let gen = source.named("beGeneraterData", "GENE-01").unwrap();
                for (&field, bytes) in &source.rec(gen).unwrap().fields {
                    if field != graft::GENERATOR_ROW && field != graft::GENERATOR_SEQ {
                        assert_eq!(
                            source.b[bytes.clone()],
                            result.b[result.rec(gen).unwrap().field(field).unwrap()]
                        );
                    }
                }
            }
            let source = Pack::parse(data(187)).unwrap();
            let result = Pack::parse(&out).unwrap();
            assert!(graft::find_named(&result, "beModelCtrlInfo", replacement)
                .unwrap()
                .is_some());
            for id in 0..source.ids.len() as u32 {
                if source.class(id, "beLuaData").is_err() {
                    continue;
                }
                let name = graft::name_of(&source, source.rec(id).unwrap()).unwrap();
                let a = source.val(id, 3).unwrap();
                let b = result.val(id, 3).unwrap();
                let am = source.memory[source.val(a, 3).unwrap() as usize]
                    .clone()
                    .unwrap();
                let bm = result.memory[result.val(b, 3).unwrap() as usize]
                    .clone()
                    .unwrap();
                if name == b"first.lua" {
                    let names = BTreeMap::from([("e_scum".into(), replacement.into())]);
                    let expected = lua_names::rename(&source.b[am], &names).unwrap().unwrap();
                    assert_eq!(result.b[bm], expected);
                } else {
                    assert_eq!(source.b[am], result.b[bm], "unrelated script changed");
                }
            }
            eprintln!("SUKAMON {variable}, seed {seed}, donors {DONORS}: intro -> {replacement}; all difficulties and boss scripts preserved");
        }
    }
}

#[test]
fn seed_67_abyss_inherits_encounter_stats_with_and_without_crowns() {
    for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
        let Some(path) = env::var_os(variable) else {
            continue;
        };
        let iso = std::fs::read(path).unwrap();
        let (base, size) = if variable == "DW4_RETAIL_USA_ISO" {
            let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
            (file.lba as usize * 2048, file.size as usize)
        } else {
            let file = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
            (file.offset as usize, file.size as usize)
        };
        let afs = &iso[base..base + size];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let (offset, len) = entries[621];
        let data = &afs[offset..offset + len];
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let table = EnemyTable::read(&iso, &layout).unwrap();
        let mut random = Randomiser::scan(
            afs,
            &entries,
            table.clone(),
            Options {
                seed: 67,
                donors: DONORS,
                warp: None,
                only: Some(vec![621]),
            },
        )
        .unwrap();
        let expanded = materialize(data, &expand(data).unwrap()).unwrap();
        random.stage(621, expanded, afs, &entries).unwrap();
        let variants = &random.rewards[&621].rows;
        if variable == "DW4_RETAIL_USA_ISO" {
            let mecha: BTreeSet<_> = variants
                .values()
                .filter(|v| family(v.combat as u32) == Some("e_mecha4"))
                .map(|v| v.combat)
                .collect();
            assert_eq!(mecha, BTreeSet::from([57, 58, 62, 65]));
        }
        let plan = random.reward_plan(&iso).unwrap();
        let unscaled = plan.unscaled(&iso).unwrap();
        assert!(!unscaled.is_empty());
        for crown_mod in [false, true] {
            let settings = crate::plan::PatchPlan {
                crown_mod,
                ..Default::default()
            };
            let (expected, _) = crate::plan::transform(
                &table,
                &settings,
                &crate::ratio::RatioVector::derive(&table),
            );
            let blocks = expected.blocks();
            let mut regions: Vec<_> = unscaled
                .iter()
                .map(|region| {
                    let index = blocks
                        .iter()
                        .position(|b| b.len() == region.bytes.len())
                        .unwrap();
                    crate::region::WriteRegion {
                        offset: region.offset,
                        bytes: blocks[index].clone(),
                    }
                })
                .collect();
            plan.apply(&mut regions, &expected).unwrap();
            if !crown_mod {
                assert_eq!(
                    regions, unscaled,
                    "archive and application writers must agree"
                );
            }
            for region in &regions {
                let index = blocks
                    .iter()
                    .position(|b| b.len() == region.bytes.len())
                    .unwrap();
                let stride = [4, 24, 3][index];
                for (&row, variant) in variants {
                    if index == 1 {
                        let stats: Vec<_> = region.bytes[row * 24..(row + 1) * 24]
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| i16::from_le_bytes(*b))
                            .collect();
                        assert_eq!(&stats[..5], &expected.stat[variant.reward][..5]);
                        assert_eq!(&stats[9..], &expected.stat[variant.reward][9..]);
                        for (column, value) in stats.iter().enumerate().take(9).skip(5) {
                            assert!(
                                (i32::from(*value)
                                    - i32::from(expected.stat[variant.reward][column]))
                                .abs()
                                    <= 100
                            );
                        }
                    } else {
                        assert_eq!(
                            &region.bytes[row * stride..(row + 1) * stride],
                            &blocks[index][variant.reward * stride..(variant.reward + 1) * stride]
                        );
                    }
                }
            }
            eprintln!(
                "PASS {variable} seed 67: {} balanced variants, crown stats {crown_mod}",
                variants.len()
            );
        }
    }
}
