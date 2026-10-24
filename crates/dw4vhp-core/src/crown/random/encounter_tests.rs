use super::*;

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
            (628, vec!["e_nume"]),
            (649, vec!["e_mummy", "e_tyrano"]),
            (686, vec!["e_torta"]),
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
                    let must_keep = g.seq
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
            protected > 500 && sequences > 500 && changed > 100,
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
