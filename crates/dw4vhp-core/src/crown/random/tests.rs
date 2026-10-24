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
fn mangrove_seed_1278675365_changes_regular_spawns_and_keeps_scripted_groups() {
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
        let mut r = Randomiser::scan(
            afs,
            &entries,
            EnemyTable::read(&iso, &layout).unwrap(),
            Options {
                seed: 1278675365,
                donors: 3,
                warp: None,
                only: Some(vec![686]),
            },
        )
        .unwrap();
        let stage = r.stages[&686].clone();
        let initial = materialize(data(686), &expand(data(686)).unwrap()).unwrap();
        let out = r.stage(686, initial, afs, &entries).unwrap();
        let mut changed = 0;
        let mut protected = BTreeSet::new();
        for pack in &stage.gens {
            let source = Pack::parse(data(*pack)).unwrap();
            let patched = r
                .retarget(*pack, data(*pack))
                .unwrap()
                .unwrap_or_else(|| data(*pack).to_vec());
            let result = Pack::parse(&patched).unwrap();
            for g in generators(&source).unwrap() {
                let id = source.named("beGeneraterData", &g.name).unwrap();
                let new = result.val(id, graft::GENERATOR_ROW).unwrap();
                if stage.pinned.contains(&g.name) || g.seq {
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
        for name in [
            "GENE-131", "GENE-139", "GENE-165", "GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04",
            "GENE-s05",
        ] {
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
                assert_eq!(
                    native.b[native.memory[mem as usize].clone().unwrap()],
                    result.b[result.memory[mem as usize].clone().unwrap()]
                );
            }
        }
        eprintln!("MANGROVE {variable}: {changed} ordinary rows changed; {} scripted groups and all original scripts preserved", protected.len());
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
            donors: 3,
            warp: None,
            only: Some(vec![179, 187]),
        },
    )
    .unwrap();
    let original = materialize(data(187), &expand(data(187)).unwrap()).unwrap();
    let out = r.stage(187, original.clone(), afs, &entries).unwrap();
    let p = Pack::parse(&out).unwrap();
    assert!(graft::find_named(&p, "beModelCtrlInfo", "e_nume")
        .unwrap()
        .is_some());
    assert!(graft::find_named(&p, "beModelCtrlInfo", "e_mecha3")
        .unwrap()
        .is_none());
    assert_eq!(super::super::capacity::slots(&p, "e_mecha4").unwrap(), 6);
    assert_eq!(super::super::capacity::slots(&p, "e_infermo").unwrap(), 12);
    assert_eq!(r.stats.copied, 2);
    for &pack in &r.stages[&187].gens {
        let bytes = r.retarget(pack, data(pack)).unwrap().unwrap();
        let target = Pack::parse(&bytes).unwrap();
        let mut counts = BTreeMap::new();
        for g in generators(&target).unwrap() {
            if r.plans[&pack].rows.contains_key(&g.name) {
                *counts
                    .entry(family(r.plans[&pack].rows[&g.name]).unwrap())
                    .or_insert(0) += 1;
            }
        }
        assert_eq!(counts, BTreeMap::from([("e_mecha4", 2), ("e_infermo", 1)]));
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
        "Humid Cave B2: {} -> {} model bytes; MR04/Infermon assignments and pools retained",
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
    let mut r = Randomiser::scan(
        afs,
        &entries,
        EnemyTable::read(&iso, &layout).unwrap(),
        Options {
            seed: 1520,
            donors: 3,
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
            if let Some(m) = family(g.row) {
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
                if original.classes[original.objects[*ix].class].name == "beGeneraterPlayerData" {
                    assert_eq!(
                        original.val(id as u32, 3).unwrap(),
                        patched.val(id as u32, 3).unwrap()
                    );
                }
            }
        }
    }
    assert!(checked >= 9);
    assert!(
        super::super::capacity::slots(&p, "e_tyrano").unwrap() > 4,
        "the imported four-slot Tyrannomon pool must grow"
    );
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
        "seed 1520: Tyrannomon slots {}, crown slots {}",
        super::super::capacity::slots(&p, "e_tyrano").unwrap(),
        super::super::capacity::slots(&p, "i_rareicon").unwrap()
    );
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
