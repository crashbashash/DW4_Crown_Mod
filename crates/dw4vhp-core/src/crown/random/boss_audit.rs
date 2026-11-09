//! Read-only investigation of boss resources and arena dependencies. This does
//! not change the shipped randomizer's eligibility or seed rules.
//! The prototype checks serialized resources, callback routing, audio and local
//! stat-row planning. Entrance sequences, summon dependencies, arena geometry,
//! memory headroom and playable completion still require separate validation.
//! Overworld boss variants and quest encounters are outside the swap matrix.
use super::*;
use serde_json::json;

const CANDIDATES: &[(usize, &str)] = &[
    (187, "e_blosso"),
    (210, "e_mammos"),
    (241, "e_skulgrey"),
    (265, "e_anomalo"),
    (296, "e_tonogeko"),
    (319, "e_mecha4"),
    (349, "e_diabolo"),
    (394, "e_mecha5"),
    (425, "e_ldknight"),
];

/// Run explicitly with a retail PS2/GameCube ISO and an evidence destination:
/// `DW4_BOSS_AUDIT_ISO=... DW4_BOSS_AUDIT_OUT=... cargo test --features devtools
/// audit_boss_contracts -- --ignored --nocapture`.
#[test]
#[ignore]
fn audit_boss_contracts() {
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO")).unwrap();
    let release = crate::release::detect(&iso).unwrap();
    let (offset, size) = if release.is_gamecube() {
        let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
        (f.offset as usize, f.size as usize)
    } else {
        let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        (f.lba as usize * 2048, f.size as usize)
    };
    let afs = &iso[offset..offset + size];
    let entries = (0..u32at(afs, 4).unwrap() as usize)
        .map(|i| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        })
        .collect::<Vec<_>>();
    let data = |entry: usize| {
        let (offset, size) = entries[entry];
        &afs[offset..offset + size]
    };
    let mut stages = Vec::new();
    for (entry, &(_, size)) in entries.iter().enumerate() {
        if size < 1_500_000 || memchr::memmem::find(data(entry), b"KIND_ENEMY\0").is_none() {
            continue;
        }
        let pack = Pack::parse(data(entry)).unwrap();
        let mut bosses = ROW_GROUPS
            .iter()
            .filter(|g| {
                !replaceable(g.model)
                    && !g.model.starts_with("g_")
                    && graft::find_named(&pack, "beModelCtrlInfo", g.model)
                        .unwrap()
                        .is_some()
            })
            .map(|g| g.model)
            .chain(
                BOSS_REGULAR
                    .iter()
                    .filter(|(e, _)| *e == entry)
                    .map(|(_, model)| *model),
            )
            .collect::<BTreeSet<_>>();
        let mut scripts = Vec::new();
        for id in 0..pack.ids.len() as u32 {
            if pack.class(id, "beLuaData").is_err() {
                continue;
            }
            let name = graft::name_of(&pack, pack.rec(id).unwrap()).unwrap();
            let name = String::from_utf8_lossy(name).into_owned();
            let object = pack.val(id, 3).unwrap();
            let size = pack.val(object, 2).unwrap() as usize;
            let range = pack.memory[pack.val(object, 3).unwrap() as usize]
                .as_ref()
                .unwrap();
            let constants = lua_names::constants(&pack.b[range.start..range.start + size])
                .unwrap()
                .into_iter()
                .map(|c| String::from_utf8_lossy(&c.text).into_owned())
                .collect::<Vec<_>>();
            if constants.iter().any(|c| c == "SETMODELHP") {
                for constant in &constants {
                    if let Some((model, _)) = constant.split_once("/I<") {
                        if let Some(group) = ROW_GROUPS
                            .iter()
                            .filter(|g| model.starts_with(g.model))
                            .max_by_key(|g| g.model.len())
                        {
                            bosses.insert(group.model);
                        }
                    }
                }
            }
            scripts.push(json!({"name": name, "constants": constants}));
        }
        if bosses.is_empty() {
            continue;
        }
        scripts.retain(|script| {
            bosses.iter().any(|model| {
                script["name"].as_str().unwrap().contains(model)
                    || script["constants"].as_array().unwrap().iter().any(|c| {
                        let c = c.as_str().unwrap();
                        c.starts_with(model)
                    })
            })
        });
        let mut gen_packs = Vec::new();
        for candidate in entry + 1..=(entry + 7).min(entries.len() - 1) {
            if data(candidate).len() >= 1_500_000
                || memchr::memmem::find(data(candidate), b"beGeneraterData\0").is_none()
            {
                continue;
            }
            let p = Pack::parse(data(candidate)).unwrap();
            let gens = generators(&p)
                .unwrap()
                .into_iter()
                .filter(|g| family(g.row).is_some_and(|m| bosses.contains(m)))
                .map(|g| {
                    let id = p.named("beGeneraterData", &g.name).unwrap();
                    let strings = p
                        .rec(id)
                        .unwrap()
                        .fields
                        .iter()
                        .filter_map(|(n, r)| {
                            let class = &p.classes[p.rec(id).unwrap().class];
                            class
                                .fields
                                .iter()
                                .any(|(t, field, _)| *t == 4 && field == n)
                                .then(|| {
                                    (
                                        n.to_string(),
                                        String::from_utf8_lossy(&p.b[r.start + 4..r.end])
                                            .trim_end_matches('\0')
                                            .to_string(),
                                    )
                                })
                        })
                        .collect::<BTreeMap<_, _>>();
                    json!({"name": g.name, "row": g.row, "model": family(g.row),
                        "entrance": g.entrance, "start": g.start, "most": g.most,
                        "strings": strings})
                })
                .collect::<Vec<_>>();
            gen_packs.push(json!({"entry": candidate, "generators": gens}));
        }
        let models = bosses
            .iter()
            .map(|model| {
                let profile = graft::profile(&pack, model);
                let (ram, issue) = match profile {
                    Ok(profile) => (Some(profile.ram), None),
                    Err(error) => (None, Some(error.to_string())),
                };
                json!({"model": model, "ram": ram, "profileIssue": issue,
                    "sequences": model_sequences(&pack, model).ok(),
                    "audioComplete": super::super::sound::complete(data(entry - 1), model).unwrap()})
            })
            .collect::<Vec<_>>();
        eprintln!("BOSS AUDIT pack {entry}: {bosses:?}");
        stages.push(json!({"entry": entry, "skipped": SKIPPED.contains(&entry),
            "models": models, "generatorPacks": gen_packs, "scripts": scripts}));
    }
    assert!(!stages.is_empty());
    let report = json!({"rulesVersion": RULES_VERSION, "stages": stages});
    let output = env::var_os("DW4_BOSS_AUDIT_OUT").expect("evidence JSON destination");
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

use super::boss::graft_model as prototype;

/// Structural feasibility check only. No shipped seed or eligibility changes.
#[test]
#[ignore]
fn prototype_boss_swaps_keep_destination_victory_and_donor_combat() {
    assert_eq!(
        CANDIDATES
            .iter()
            .map(|(_, model)| *model)
            .collect::<BTreeSet<_>>(),
        super::boss_plan::MODELS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
    );
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO")).unwrap();
    let (offset, size) = if crate::release::detect(&iso).unwrap().is_gamecube() {
        let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
        (f.offset as usize, f.size as usize)
    } else {
        let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        (f.lba as usize * 2048, f.size as usize)
    };
    let afs = &iso[offset..offset + size];
    let release = crate::release::detect(&iso).unwrap();
    let table = EnemyTable::read(&iso, &release.layout_in(&iso).unwrap()).unwrap();
    let data = |e: usize| {
        let offset = u32at(afs, 8 + 8 * e).unwrap() as usize;
        let size = u32at(afs, 12 + 8 * e).unwrap() as usize;
        &afs[offset..offset + size]
    };
    let output = std::path::PathBuf::from(env::var_os("DW4_BOSS_AUDIT_OUT").unwrap());
    let mut swaps = Vec::new();
    for entry in [730, 737, 744, 751, 758, 765] {
        assert!(SKIPPED.contains(&entry), "world boss phase must stay fixed");
        assert!(!CANDIDATES.iter().any(|(candidate, _)| *candidate == entry));
    }
    for &(destination, from) in CANDIDATES {
        for &(source, to) in CANDIDATES {
            if from == to {
                continue;
            }
            if graft::find_named(
                &Pack::parse(data(destination)).unwrap(),
                "beModelCtrlInfo",
                to,
            )
            .unwrap()
            .is_some()
            {
                // A shared body would also give ordinary residents the boss's
                // victory callback. A future planner must reject this pairing
                // or supply a genuinely separate model and script namespace.
                assert_eq!((destination, to), (425, "e_mecha5"));
                swaps.push(json!({"destination": destination, "from": from, "source": source,
                    "to": to, "result": {"eligible": false, "verified": false,
                        "reason": "resident family already present; requires an isolated boss model"}}));
                continue;
            }
            let result = (|| -> Result<_> {
                let bytes = prototype(data(destination), data(source), from, to)?;
                let companions =
                    graft::companion_models(&Pack::parse(data(source))?, None, &[to.to_string()])?;
                let sounds = super::super::sound::graft_enemy(
                    data(destination - 1),
                    data(source - 1),
                    data(source - 1),
                    to,
                    &companions,
                )?;
                need(
                    super::super::sound::complete(&sounds, to)?,
                    "incomplete boss audio",
                )?;
                // Plan all difficulties with private local stat/reward variants.
                // The generator's completion callback and every other byte stay native.
                let packs = (destination + 1..=destination + 7)
                    .filter(|&entry| {
                        data(entry).len() < 1_500_000
                            && memchr::memmem::find(data(entry), b"beGeneraterData\0").is_some()
                    })
                    .collect::<Vec<_>>();
                need(packs.len() == 3, "missing boss difficulty packs")?;
                let used = packs
                    .iter()
                    .flat_map(|&entry| {
                        generators(&Pack::parse(data(entry)).unwrap())
                            .unwrap()
                            .into_iter()
                            .map(|g| g.row as usize)
                    })
                    .collect::<BTreeSet<_>>();
                let free = (0..table.len())
                    .filter(|row| table.hp[*row] == 0 && !used.contains(row))
                    .collect();
                let mut rewards = super::super::rewards::StageRewards::new(packs.clone(), free);
                let mut count = 0;
                for pack in packs {
                    let before = Pack::parse(data(pack))?;
                    let mut target = Retarget::default();
                    let mut expected = data(pack).to_vec();
                    for g in generators(&before)? {
                        if family(g.row) != Some(from) {
                            continue;
                        }
                        let combat = nearest_row(&table, g.row, to)
                            .ok_or_else(|| bad("no boss rarity match"))?;
                        let row = rewards
                            .assign(combat as usize, g.row as usize)
                            .ok_or_else(|| bad("no local boss stat row"))?;
                        assert_eq!(
                            rewards.rows[&row],
                            super::super::rewards::Variant {
                                combat: combat as usize,
                                reward: g.row as usize
                            }
                        );
                        target.rows.insert(g.name.clone(), row as u32);
                        let id = before.named("beGeneraterData", &g.name)?;
                        let field = before.rec(id)?.field(graft::GENERATOR_ROW)?;
                        expected[field].copy_from_slice(&(row as u32).to_le_bytes());
                    }
                    let (after, changed) = graft::retarget_rows(data(pack), &target)?;
                    assert_eq!(after, expected, "only the boss row may change");
                    need(changed > 0, "missing boss generator")?;
                    count += changed;
                }
                Ok((bytes.len(), sounds.len(), count))
            })();
            let status = match result {
                Ok((bytes, sound_bytes, count)) => {
                    eprintln!("BOSS PROTOTYPE {destination} {from} -> {to}: verified resources and callbacks");
                    json!({"bytes": bytes, "soundBytes": sound_bytes,
                        "generators": count, "verified": true})
                }
                Err(error) => {
                    eprintln!("BOSS PROTOTYPE {destination} {from} -> {to}: {error}");
                    json!({"verified": false, "issue": error.to_string()})
                }
            };
            swaps.push(
                json!({"destination": destination, "from": from, "source": source,
                "to": to, "result": status}),
            );
        }
    }
    std::fs::write(output, serde_json::to_vec_pretty(&swaps).unwrap()).unwrap();
    assert_eq!(swaps.len(), CANDIDATES.len() * (CANDIDATES.len() - 1));
    assert_eq!(
        swaps
            .iter()
            .filter(|s| s["result"]["eligible"] == false)
            .count(),
        1
    );
    assert_eq!(
        swaps
            .iter()
            .filter(|s| s["result"]["verified"] == true)
            .count(),
        71,
        "see evidence JSON for incompatible pairs"
    );
}

/// Exercise the roster planner against actual final patrol assignments. Force
/// MR05 into Humid Cave's ordinary groups, then remove optional imports for the
/// other arenas so a second floor remains available for MR05 as a boss.
#[test]
#[ignore]
fn prototype_boss_rosters_follow_final_patrols_and_preserve_counts() {
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO")).unwrap();
    let release = crate::release::detect(&iso).unwrap();
    let (offset, size) = if release.is_gamecube() {
        let file = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
        (file.offset as usize, file.size as usize)
    } else {
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        (file.lba as usize * 2048, file.size as usize)
    };
    let afs = &iso[offset..offset + size];
    let entries = (0..u32at(afs, 4).unwrap() as usize)
        .map(|entry| {
            (
                u32at(afs, 8 + 8 * entry).unwrap() as usize,
                u32at(afs, 12 + 8 * entry).unwrap() as usize,
            )
        })
        .collect::<Vec<_>>();
    let data = |entry: usize| {
        let (offset, size) = entries[entry];
        &afs[offset..offset + size]
    };
    let table = EnemyTable::read(&iso, &release.layout_in(&iso).unwrap()).unwrap();
    let mut random = Randomiser::scan(
        afs,
        &entries,
        table,
        Options {
            picks_only: false,
            seed: 1520,
            donors: DONORS,
            warp: None,
            only: None,
            random_bosses: false,
        },
    )
    .unwrap();
    random.donors.retain(|(model, _), _| *model == "e_mecha5");
    let mut tiers: [Vec<super::boss_plan::Slot>; 3] = std::array::from_fn(|_| Vec::new());
    for (entry, original, boss) in [
        (187, "e_blosso", "GENE-08"),
        (210, "e_mammos", "GENE-12"),
        (394, "e_mecha5", "GENE-23"),
    ] {
        if entry != 187 {
            random.donors.clear();
        }
        let stage = random.stages[&entry].clone();
        random
            .stage(entry, data(entry).to_vec(), afs, &entries)
            .unwrap();
        for (difficulty, pack) in [stage.gens[1], stage.gens[0], stage.gens[2]]
            .into_iter()
            .enumerate()
        {
            let patrols = generators(&Pack::parse(data(pack)).unwrap())
                .unwrap()
                .into_iter()
                .filter(|g| g.name != boss)
                .filter_map(|g| {
                    let row = random
                        .plans
                        .get(&pack)
                        .and_then(|p| p.rows.get(&g.name))
                        .copied()
                        .unwrap_or(g.row);
                    family(row).map(str::to_string)
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(patrols.contains("e_mecha5"), entry == 187);
            tiers[difficulty].push(super::boss_plan::Slot {
                id: entry.to_string(),
                original: original.to_string(),
                patrols,
                compatible: super::boss_plan::MODELS
                    .iter()
                    .map(|m| m.to_string())
                    .collect(),
            });
        }
    }
    let mut evidence = Vec::new();
    for (difficulty, slots) in tiers.iter().enumerate() {
        let expected = slots
            .iter()
            .map(|s| s.original.clone())
            .collect::<BTreeSet<_>>();
        for seed in 0..50 {
            let chosen =
                super::boss_plan::assign(slots, &mut Rng::new(seed, difficulty as u64)).unwrap();
            assert_eq!(chosen.values().cloned().collect::<BTreeSet<_>>(), expected);
            assert_eq!(chosen["187"], "e_mammos");
            assert_eq!(chosen["210"], "e_mecha5");
            assert_eq!(chosen["394"], "e_blosso");
            for slot in slots {
                assert!(!slot.patrols.contains(&chosen[&slot.id]));
            }
            if seed == 0 {
                evidence.push(json!({"difficulty": difficulty,
                    "bosses": slots.iter().map(|s| json!({"entry": s.id,
                        "original": s.original, "patrols": s.patrols,
                        "selectedBoss": chosen[&s.id]})).collect::<Vec<_>>()}));
            }
        }
    }
    std::fs::write(
        env::var_os("DW4_BOSS_AUDIT_OUT").expect("evidence JSON destination"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}
