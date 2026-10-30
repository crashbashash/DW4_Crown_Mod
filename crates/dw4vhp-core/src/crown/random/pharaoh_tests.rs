use super::super::river::lua::Chunk;
use super::*;
use crate::table::EnemyTable;

#[test]
fn retail_pharaoh_summons_follow_selected_families() {
    for variable in [
        "DW4_RETAIL_USA_ISO",
        "DW4_RETAIL_EUROPE_ISO",
        "DW4_RETAIL_JAPAN_ISO",
        "DW4_RETAIL_GC_USA_ISO",
        "DW4_RETAIL_GC_JAPAN_ISO",
    ] {
        let Some(path) = std::env::var_os(variable) else {
            continue;
        };
        let iso = std::fs::read(path).unwrap();
        let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
            let file = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
            (file.offset as usize, file.size as usize)
        } else {
            let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
            (file.lba as usize * 2048, file.size as usize)
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
        r.donors
            .retain(|(model, _), _| ["e_pharaoh", "e_mummy", "e_otama"].contains(model));
        for entry in [621, 249] {
            let out = r.stage(entry, data(entry).to_vec(), afs, &entries).unwrap();

            for pack in &r.stages[&entry].gens {
                for (name, row) in &r.plans[pack].rows {
                    if family(*row) == Some("e_pharaoh") {
                        assert_eq!(
                            r.plans[pack].sequences[name], "SQ_ENTR",
                            "Pharaohmon must initialize its summon labels"
                        );
                    }
                }
            }
            let p = Pack::parse(&out).unwrap();
            let id = p.named("beLuaData", "cal_e_pharaoh.lua").unwrap();
            let obj = p.val(id, 3).unwrap();
            let raw = &out[p.memory[p.val(obj, 3).unwrap() as usize].clone().unwrap()];
            let c = Chunk::read(raw).unwrap();
            assert!(c.strings.iter().any(|s| s == b"SYSgetRandom\0"));
            for (pc, &ins) in c.code.iter().enumerate() {
                if c.is(ins, 16, "MDLisLive") {
                    assert_eq!(
                        c.code[pc + 1] & 63,
                        12,
                        "live checks must use the tracked model table"
                    );
                }
            }
            assert!(r
                .stats
                .trace
                .iter()
                .any(|s| s.starts_with("pick ") && s.ends_with(" e_pharaoh")));
            if let Some(root) = std::env::var_os("DW4_PHARAOH_AUDIT_DIR") {
                let dir = std::path::PathBuf::from(root)
                    .join(variable)
                    .join(entry.to_string());
                std::fs::create_dir_all(&dir).unwrap();
                let mut metadata = String::from("allowed = {} rows = {}\n");
                if c.code.iter().any(|&ins| c.is(ins, 7, "e_mummy")) {
                    let base = super::super::capacity::slots(&p, "e_mummy").unwrap()
                        - super::super::pharaoh::LIMIT;
                    metadata.push_str(&format!("mummy_base = {base}\n"));
                } else {
                    metadata.push_str("mummy_base = nil\n");
                }

                for &model in REGULAR {
                    if let Ok(capacity) = super::super::capacity::slots(&p, model) {
                        metadata.push_str(&format!("allowed[{model:?}] = {capacity}\n"));
                    }
                }
                for (difficulty, original) in [116, 120, 124].into_iter().enumerate() {
                    metadata.push_str(&format!("rows[{original}] = {difficulty}\n"));
                    for (row, variant) in &r.rewards[&entry].rows {
                        if variant.reward == original {
                            metadata.push_str(&format!("rows[{row}] = {difficulty}\n"));
                        }
                    }
                }
                std::fs::write(dir.join("metadata.lua"), metadata).unwrap();

                for name in ["cal_e_pharaoh.lua", "hit_e_mummy.lua", "mag_e_mummy.lua"] {
                    if let Ok(id) = p.named("beLuaData", name) {
                        let obj = p.val(id, 3).unwrap();
                        let raw = &out[p.memory[p.val(obj, 3).unwrap() as usize].clone().unwrap()];
                        std::fs::write(dir.join(name), raw).unwrap();
                    }
                }
                std::fs::write(dir.join("plan.txt"), r.log.join("\n")).unwrap();
            }
            println!(
                "{variable} stage {entry}: {}",
                r.log
                    .iter()
                    .rev()
                    .find(|s| s.contains("rolls each summon"))
                    .unwrap()
            );
        }
        // Repeat a crowded stage with exactly the rows its generators used.
        // Summons must reuse those encounters when no free variant rows remain.
        let entry = 709;
        r.stage(entry, data(entry).to_vec(), afs, &entries).unwrap();
        let used: BTreeSet<_> = r.rewards[&entry]
            .targets
            .values()
            .flat_map(|targets| targets.values())
            .map(|&row| row as usize)
            .filter(|row| r.rewards[&entry].rows.contains_key(row))
            .collect();
        assert!(!used.is_empty());
        for (row, hp) in r.table.hp.iter_mut().enumerate() {
            if *hp == 0 && !used.contains(&row) {
                *hp = 1;
            }
        }
        r.sounds.remove(&(entry - 1));
        r.rewards.remove(&entry);
        r.log.clear();
        let out = r.stage(entry, data(entry).to_vec(), afs, &entries).unwrap();
        assert!(
            r.log
                .iter()
                .any(|line| line.contains("summons reuse selected encounter rows")),
            "{variable}: {:?}",
            r.log
        );
        let p = Pack::parse(&out).unwrap();
        assert!(p.named("beLuaData", "cal_e_pharaoh.lua").is_ok());
        println!("{variable}: exhausted summon variants reuse selected stage encounters");
    }
}

/// Optional focused image for a fresh-disc Dolphin gameplay check. Keep other
/// stages native so encounter setup does not depend on unrelated seed choices.
#[test]
fn build_pharaoh_gamecube_gameplay_fixture() {
    let Some(output) = std::env::var_os("DW4_PHARAOH_TEST_IMAGE") else {
        return;
    };
    let input = std::env::var_os("DW4_RETAIL_GC_USA_ISO").expect("GameCube USA input");
    let iso = std::fs::read(input).unwrap();
    let mut plan = super::super::CrownPlan::build_gcm_with_balance(
        &iso,
        false,
        false,
        false,
        0,
        Some(Options {
            seed: 1520,
            donors: 1,
            warp: None,
            only: Some(vec![249]),
        }),
        false,
    )
    .unwrap();
    let report = crate::disc::inspect_retail_bytes(&iso).unwrap();
    plan.regions
        .extend(crate::gc_memory::regions(&iso, &report).unwrap());
    let mut file = std::fs::File::create_new(&output).unwrap();
    plan.write_image(&iso, &mut file).unwrap();
    drop(file);
    plan.verify(&iso, &std::fs::read(&output).unwrap()).unwrap();
    println!("Verified focused Pharaohmon GameCube gameplay fixture");
}
