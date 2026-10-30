use super::*;

#[test]
fn randomised_minotaurmon_bypass_healing_for_native_and_imported_models() {
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
                only: None,
            },
        )
        .unwrap();
        // Exercise a resident family and a real import without process-wide RNG overrides.
        r.donors.retain(|(model, _), _| *model == "e_minotaru");
        for entry in [249, 621] {
            let stage = r.stages[&entry].clone();
            assert_eq!(stage.residents.contains_key("e_minotaru"), entry == 249);
            let out = r.stage(entry, data(entry).to_vec(), afs, &entries).unwrap();
            let model = Pack::parse(&out).unwrap();
            // Check the actual native/imported alternate entrance bypasses ADITHER.
            let info = model.named("beModelCtrlInfo", "e_minotaru").unwrap();
            let seq = model
                .array(model.val(info, 5).unwrap())
                .unwrap()
                .into_iter()
                .find(|&id| model.class(id, "beModelCtrlInfoDataSeq").is_ok())
                .unwrap();
            let entrance = model
                .array(model.val(seq, 4).unwrap())
                .unwrap()
                .into_iter()
                .find(|&id| model.rec(id).unwrap().named(model.b, "SQ_NENTR"))
                .unwrap();
            assert!(model
                .array(model.val(entrance, 3).unwrap())
                .unwrap()
                .into_iter()
                .any(|id| {
                    model.class(id, "beModelCtrlLABELCTRL").is_ok()
                        && model.rec(id).unwrap().named(model.b, "NOMR")
                        && {
                            let f = model.rec(id).unwrap().field(3).unwrap();
                            model.b[f.start + 4..f.end].starts_with(b"SQ_SWON\0")
                        }
                }));
            let mut counts = Vec::new();
            for pack in &stage.gens {
                let original = Pack::parse(data(*pack)).unwrap();
                let out = r.retarget(*pack, data(*pack)).unwrap().unwrap();
                assert_eq!(r.retarget(*pack, &out).unwrap().unwrap(), out);
                let after = Pack::parse(&out).unwrap();
                let mut minotaurmon = 0;
                for g in generators(&original).unwrap() {
                    let id = original.named("beGeneraterData", &g.name).unwrap();
                    let new = after.rec(id).unwrap();
                    let selected = r.plans[pack].rows.get(&g.name).copied();
                    let sequence = new.field(graft::GENERATOR_SEQ).unwrap();
                    if selected.and_then(family) == Some("e_minotaru") {
                        assert!(
                            after.b[sequence.start + 4..sequence.end].starts_with(b"SQ_NENTR\0")
                        );
                        // The EXP variant has a private row, so its family cannot
                        // be inferred from the final row number alone.
                        let variant = after.val(id, graft::GENERATOR_ROW).unwrap() as usize;
                        let reward = &r.rewards[&entry].rows[&variant];
                        assert_eq!(family(reward.combat as u32), Some("e_minotaru"));
                        assert_eq!(reward.reward, g.row as usize);
                        minotaurmon += 1;
                    } else if selected.is_none()
                        || !portable_entrance(&g)
                        || selected.and_then(family) == family(g.row)
                    {
                        let before = original
                            .rec(id)
                            .unwrap()
                            .field(graft::GENERATOR_SEQ)
                            .unwrap();
                        assert_eq!(original.b[before], after.b[sequence]);
                    }
                    if protected_entrance(&g)
                        || stage.pinned.contains(&g.name)
                        || family(g.row).is_some_and(|m| stage.pinned_families.contains(m))
                    {
                        assert_eq!(
                            original.b[original.rec(id).unwrap().range.clone()],
                            after.b[new.range.clone()]
                        );
                    }
                }
                assert!(minotaurmon > 0, "{variable} stage {entry} pack {pack}");
                counts.push(minotaurmon);
            }
            eprintln!("MINOTAURMON {variable} stage {entry}: {counts:?} alternate entrances across difficulties");
        }
    }
}

#[test]
fn every_scripted_spawn_and_quest_dependency_survives() {
    for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
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
                donors: 0,
                warp: None,
                only: None,
            },
        )
        .unwrap();
        for (e, models) in [
            (265, vec!["e_mummy", "e_pharaoh", "e_mecha1", "e_mecha2"]),
            (319, vec!["e_mecha4"]),
            (394, vec!["e_mecha5"]),
        ] {
            for model in models {
                assert!(
                    r.stages[&e].pinned_families.contains(model),
                    "{e} {model} is controlled by stage scripts"
                );
            }
        }
        let (mut protected, mut sequences, mut changed) = (0, 0, 0);
        let mut kinds = BTreeSet::new();
        for (e, stage) in r.stages.clone() {
            let initial = if memchr::memmem::find(data(e), b"i_rareicon").is_some() {
                materialize(data(e), &expand(data(e)).unwrap()).unwrap()
            } else {
                data(e).to_vec()
            };
            let out = r
                .stage(e, initial, afs, &entries)
                .unwrap_or_else(|err| panic!("{variable} {e}: {err}"));
            for pack in &stage.gens {
                let original = Pack::parse(data(*pack)).unwrap();
                let after = r
                    .retarget(*pack, data(*pack))
                    .unwrap()
                    .unwrap_or_else(|| data(*pack).to_vec());
                let after = Pack::parse(&after).unwrap();
                let target = generators(&after).unwrap();
                for g in generators(&original).unwrap() {
                    let target = target.iter().find(|n| n.name == g.name).unwrap();
                    let must_keep = protected_entrance(&g)
                        || stage.pinned.contains(&g.name)
                        || family(g.row).is_some_and(|m| stage.pinned_families.contains(m));
                    if must_keep {
                        assert_eq!(target.row, g.row, "{variable} {pack} {}", g.name);
                        let id = original.named("beGeneraterData", &g.name).unwrap();
                        let f = original
                            .rec(id)
                            .unwrap()
                            .field(graft::GENERATOR_SEQ)
                            .unwrap();
                        let t = after.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                        assert_eq!(original.b[f.clone()], after.b[t]);
                        protected += 1;
                        if g.seq && family(g.row).is_some_and(|m| m.starts_with("e_")) {
                            sequences += 1;
                            kinds.insert(
                                String::from_utf8_lossy(&original.b[f.start + 4..f.end])
                                    .trim_end_matches('\0')
                                    .to_string(),
                            );
                        }
                    } else if target.row != g.row {
                        changed += 1;
                    }
                    if e == 449 && target.row != g.row {
                        assert!(["e_mummy", "e_mecha1", "e_minotaru", "e_pharaoh"].contains(
                            &family(
                                r.rewards
                                    .get(&e)
                                    .and_then(|s| s.rows.get(&(target.row as usize)))
                                    .map(|v| v.combat as u32)
                                    .unwrap_or(target.row)
                            )
                            .unwrap()
                        ));
                    }
                }
            }
            if e == 449 {
                assert!(
                    !r.stats.trace.iter().any(|s| s.starts_with("copy 449 ")),
                    "counter quest must use native hit scripts"
                );
            }
            // All native model-owned scripts remain byte-identical, including
            // kill-counter scripts and summons; capacity edits only add slots.
            let p = Pack::parse(data(e)).unwrap();
            let q = Pack::parse(&out).unwrap();
            for id in 0..p.ids.len() as u32 {
                if p.class(id, "beLuaData").is_ok()
                    && graft::name_of(&p, p.rec(id).unwrap())
                        .is_some_and(|n| n.starts_with(b"hit_") || n.starts_with(b"cal_"))
                {
                    let obj = p.val(id, 3).unwrap();
                    let mem = p.val(obj, 3).unwrap();
                    assert_eq!(
                        p.b[p.memory[mem as usize].clone().unwrap()],
                        q.b[q.memory[mem as usize].clone().unwrap()]
                    );
                }
            }
        }
        assert!(
            protected > 500 && sequences > 300 && changed > 100,
            "{variable}: {protected} protected, {sequences} entrances, {changed} changed"
        );
        assert!(kinds.is_superset(&BTreeSet::from([
            "SQ_WAIT".into(),
            "SQ_FALL".into(),
            "SQ_JUMP".into(),
            "SQ_JUMP1".into(),
            "SQ_ENTRHS".into(),
            "SQ_NENTR".into(),
            "SQ_ATCK1".into(),
            "SQ_SWON".into()
        ])));
        eprintln!("SCRIPT AUDIT {variable}: {protected} protected rows, {sequences} enemy entrances, {changed} ordinary rows changed; {kinds:?}");
    }
}

#[test]
fn reviewed_scripts_and_normal_entrances_follow_replacements() {
    for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
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
        let data = |e: usize| {
            let (o, l) = entries[e];
            &afs[o..o + l]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let table = EnemyTable::read(&iso, &layout).unwrap();
        let mut r = Randomiser::scan(
            afs,
            &entries,
            table.clone(),
            Options {
                seed: 67,
                donors: DONORS,
                warp: None,
                only: None,
            },
        )
        .unwrap();
        let mut entrances = 0;
        for entry in [241, 394, 410, 628, 649] {
            let stage = r.stages[&entry].clone();
            let out = r.stage(entry, data(entry).to_vec(), afs, &entries).unwrap();
            let original = Pack::parse(data(entry)).unwrap();
            let result = Pack::parse(&out).unwrap();
            if entry == 410 {
                assert_ne!(
                    r.plans[&stage.gens[0]]
                        .rows
                        .get("GENE-08")
                        .copied()
                        .and_then(family),
                    Some("e_mecha2"),
                    "MR02 lacks SQ_NENTR: using its default entrance hung the live activation test"
                );
            }
            let mut edits = Vec::new();
            if let Some(intro) = INTRO_STAGES.iter().find(|s| s.model_entry == entry) {
                let names: BTreeMap<_, _> = intro
                    .sites
                    .iter()
                    .map(|&(generator, model)| {
                        let target = r.plans[&stage.gens[0]].rows[generator];
                        let selected = family(target).unwrap();
                        assert_ne!(selected, model);
                        (model.to_string(), selected.to_string())
                    })
                    .collect();
                edits.push((vec![intro.script], names));
            }
            if let Some(rule) = FAMILY_REMAPS.iter().find(|rule| rule.entry == entry) {
                let mut names = BTreeMap::new();
                for &model in rule.models {
                    let mut selected = BTreeSet::new();
                    let mut count = 0;
                    for &pack in &stage.gens {
                        for g in generators(&Pack::parse(data(pack)).unwrap()).unwrap() {
                            if family(g.row) == Some(model) {
                                let target = r.plans[&pack].rows[&g.name];
                                selected.insert(family(target).unwrap());
                                count += 1;
                            }
                        }
                    }
                    assert_eq!(
                        selected.len(),
                        1,
                        "one replacement must cover the whole family"
                    );
                    let target = *selected.first().unwrap();
                    assert_ne!(target, model);
                    names.insert(model.to_string(), target.to_string());
                    eprintln!(
                        "FAMILY {variable} {entry}: {model} -> {target}, {count} difficulty rows"
                    );
                }
                assert_eq!(names.values().collect::<BTreeSet<_>>().len(), names.len());
                edits.push((rule.scripts.to_vec(), names));
            }
            for (scripts, names) in edits {
                for script in scripts {
                    let raw = |p: &Pack| {
                        let id = p.named("beLuaData", script).unwrap();
                        let object = p.val(id, 3).unwrap();
                        let length = p.val(object, 2).unwrap() as usize;
                        let memory = p.memory[p.val(object, 3).unwrap() as usize]
                            .as_ref()
                            .unwrap();
                        p.b[memory.start..memory.start + length].to_vec()
                    };
                    assert_eq!(
                        raw(&result),
                        lua_names::rename(&raw(&original), &names).unwrap().unwrap(),
                        "{variable} {entry} {script}: only reviewed constants may change"
                    );
                }
            }
            for &pack in &stage.gens {
                let before = Pack::parse(data(pack)).unwrap();
                let updated = r.retarget(pack, data(pack)).unwrap().unwrap();
                let after = Pack::parse(&updated).unwrap();
                for g in generators(&before).unwrap() {
                    let id = before.named("beGeneraterData", &g.name).unwrap();
                    let target = r.plans[&pack].rows.get(&g.name).copied();
                    let changed_family = target.is_some_and(|row| family(row) != family(g.row));
                    if portable_entrance(&g) && changed_family {
                        let expected = if target.and_then(family) == Some("e_pharaoh") {
                            "SQ_ENTR"
                        } else {
                            &g.entrance
                        };
                        assert!(model_sequences(&result, target.and_then(family).unwrap())
                            .unwrap()
                            .contains(expected));
                        let seq = after.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                        let text = &after.b[seq.start + 4..seq.end];
                        if target.and_then(family) == Some("e_minotaru") {
                            assert!(text.starts_with(b"SQ_NENTR\0"));
                        } else {
                            assert_eq!(
                                text.split(|b| *b == 0).next().unwrap(),
                                expected.as_bytes()
                            );
                        }
                        entrances += 1;
                    }
                    if protected_entrance(&g)
                        || stage.pinned.contains(&g.name)
                        || family(g.row).is_some_and(|m| stage.pinned_families.contains(m))
                    {
                        assert_eq!(
                            before.b[before.rec(id).unwrap().range.clone()],
                            after.b[after.rec(id).unwrap().range.clone()],
                            "protected {} in {pack}",
                            g.name
                        );
                    }
                    for (&field, range) in &before.rec(id).unwrap().fields {
                        if field != graft::GENERATOR_ROW && field != graft::GENERATOR_SEQ {
                            assert_eq!(
                                before.b[range.clone()],
                                after.b[after.rec(id).unwrap().field(field).unwrap()]
                            );
                        }
                    }
                }
            }
        }
        assert!(entrances > 0);
        // Exhaustion must retain every member and the native cleanup/freeze targets.
        let mut exhausted = Randomiser::scan(
            afs,
            &entries,
            table,
            Options {
                seed: 67,
                donors: DONORS,
                warp: None,
                only: None,
            },
        )
        .unwrap();
        let mut available = 2;
        for hp in &mut exhausted.table.hp {
            if *hp == 0 {
                if available > 0 {
                    available -= 1;
                } else {
                    *hp = 1;
                }
            }
        }
        exhausted
            .stage(628, data(628).to_vec(), afs, &entries)
            .unwrap();
        for &pack in &exhausted.stages[&628].gens {
            assert!(exhausted.plans[&pack].rows.is_empty());
        }
        assert!(!exhausted.rewards.contains_key(&628));
        eprintln!("RELAXED {variable}: {entrances} normal entrance rows replaced; family exhaustion stays atomic");
    }
}

#[test]
fn practice_randomisation_preserves_lessons_and_party_branches() {
    for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
        let Some(path) = env::var_os(variable) else {
            continue;
        };
        let file = std::fs::File::open(path).unwrap();
        let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
        let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
            let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
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
        let data = |e: usize| {
            let (o, l) = entries[e];
            &afs[o..o + l]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let table = EnemyTable::read(&iso, &layout).unwrap();
        for four_players in [false, true] {
            let mut r = Randomiser::scan(
                afs,
                &entries,
                table.clone(),
                Options {
                    seed: 67,
                    donors: DONORS,
                    warp: None,
                    only: Some(vec![607]),
                },
            )
            .unwrap();
            assert!(r.donors.values().all(|d| d.source != 607));
            let stage = r.stages[&607].clone();
            assert_eq!(stage.gens, [608, 609, 610]);
            assert!(stage.pinned.is_empty() && stage.pinned_families.is_empty());
            let mut initial = materialize(data(607), &expand(data(607)).unwrap()).unwrap();
            if four_players {
                initial = materialize(&initial, &super::super::practice::expand(&initial).unwrap())
                    .unwrap();
            }
            let out = r.stage(607, initial.clone(), afs, &entries).unwrap();
            let p = Pack::parse(&initial).unwrap();
            let q = Pack::parse(&out).unwrap();
            // Includes rock/box teaching rules, lesson callbacks, custom player
            // hit scripts, flags and the already-patched graduation thresholds.
            for id in 0..p.ids.len() as u32 {
                if p.class(id, "beLuaData").is_err() {
                    continue;
                }
                let name =
                    std::str::from_utf8(graft::name_of(&p, p.rec(id).unwrap()).unwrap()).unwrap();
                let new = q.named("beLuaData", name).unwrap();
                let bytes = |pack: &Pack, id| {
                    let obj = pack.val(id, 3).unwrap();
                    let range = pack.memory[pack.val(obj, 3).unwrap() as usize]
                        .clone()
                        .unwrap();
                    pack.b[range.start..range.start + pack.val(obj, 2).unwrap() as usize].to_vec()
                };
                assert_eq!(bytes(&p, id), bytes(&q, new), "{variable} {name}");
            }
            let mut changed = 0;
            for pack in stage.gens {
                let initial = if four_players {
                    materialize(
                        data(pack),
                        &super::super::spawns::expand(data(pack)).unwrap(),
                    )
                    .unwrap()
                } else {
                    data(pack).to_vec()
                };
                let before = Pack::parse(&initial).unwrap();
                let out = r.retarget(pack, &initial).unwrap().unwrap();
                let after = Pack::parse(&out).unwrap();
                assert_eq!(generators(&before).unwrap().len(), 18);
                for g in generators(&before).unwrap() {
                    let id = before.named("beGeneraterData", &g.name).unwrap();
                    let b = before.rec(id).unwrap();
                    let a = after.rec(id).unwrap();
                    // Every byte except the row and compatible entrance stays
                    // intact: count, position, start/finish callbacks and drops.
                    let mut expected = before.b[b.range.clone()].to_vec();
                    for field in [graft::GENERATOR_ROW, graft::GENERATOR_SEQ] {
                        let old = b.field(field).unwrap();
                        expected[old.start - b.range.start..old.end - b.range.start]
                            .copy_from_slice(&after.b[a.field(field).unwrap().clone()]);
                    }
                    assert_eq!(expected, after.b[a.range.clone()], "{variable} {}", g.name);
                    let target = after.val(id, graft::GENERATOR_ROW).unwrap();
                    if g.name.starts_with("GBOX-") {
                        assert_eq!(target, g.row);
                    }
                    if target != g.row {
                        changed += 1;
                        let v = r.rewards[&607].rows[&(target as usize)];
                        assert_eq!(v.reward, g.row as usize);
                        assert!((638..=645).contains(&v.reward));
                    }
                }
            }
            assert!(changed > 0, "{variable}: practice must actually randomize");
            // Resolve the actual practice-local databases, not a nearby stage.
            let rewards = super::super::rewards::Plan::build(&iso, &table, &r.rewards).unwrap();
            assert!(!rewards.models.is_empty());
            for (crown_mod, exclude_practice) in [(false, true), (true, true), (true, false)] {
                let plan = crate::plan::PatchPlan {
                    crown_mod,
                    exclude_practice,
                    practice_buff: true,
                    practice_speed_mult: 0.5,
                    ..Default::default()
                };
                let (balanced, _) = crate::plan::transform(
                    &table,
                    &plan,
                    &crate::ratio::RatioVector::derive(&table),
                );
                let blocks = balanced.blocks();
                let mut regions = rewards.unscaled(&iso).unwrap();
                for region in &mut regions {
                    let index = blocks
                        .iter()
                        .position(|b| b.len() == region.bytes.len())
                        .unwrap();
                    region.bytes = blocks[index].clone();
                }
                rewards.apply(&mut regions, &balanced).unwrap();
                for region in &regions {
                    let index = blocks
                        .iter()
                        .position(|b| b.len() == region.bytes.len())
                        .unwrap();
                    let stride = [4, 24, 3][index];
                    for (&row, variant) in &r.rewards[&607].rows {
                        for byte in 0..stride {
                            if index == 1 && (10..18).contains(&byte) {
                                continue;
                            }
                            assert_eq!(region.bytes[row*stride+byte], blocks[index][variant.reward*stride+byte],
                                "{variable}: practice balance row {row}, crown={crown_mod}, excluded={exclude_practice}");
                        }
                    }
                }
            }
            eprintln!("PRACTICE {variable}: {changed} changed generators, four-player={four_players}; scripts and callbacks preserved");
        }
    }
}
