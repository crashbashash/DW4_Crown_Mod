//! Opt-in retail checks for references the loader resolves outside list owners.
use super::*;
use std::collections::BTreeSet;

fn no_orphan_references(p: &Pack) {
    let mut owned = BTreeSet::new();
    for r in &p.objects {
        for (slot, _) in graft::ref_slots(p, r) {
            let id = r.val(p.b, slot).unwrap();
            if p.memory.get(id as usize).is_some_and(Option::is_some) {
                owned.insert(id as usize);
            }
        }
    }
    for (id, range) in p.memory.iter().enumerate() {
        let Some(range) = range else { continue };
        let directory = &p.dirs[p.mapping[id] as usize];
        if directory.val(p.b, 10).unwrap() != 0 {
            continue;
        }
        for word in p.b[range.clone()].as_chunks::<4>().0 {
            let target = u32::from_le_bytes(*word) as usize;
            if p.memory.get(target).is_some_and(Option::is_some) {
                owned.insert(target);
            }
        }
    }
    for (id, range) in p.memory.iter().enumerate() {
        let Some(range) = range else { continue };
        let directory = &p.dirs[p.mapping[id] as usize];
        if owned.contains(&id) || directory.val(p.b, 10).unwrap() != 0 {
            continue;
        }
        assert!(
            p.b[range.clone()].as_chunks::<4>().0.iter().all(|word| {
                let target = u32::from_le_bytes(*word) as usize;
                !p.ids.get(target).is_some_and(Option::is_some)
            }),
            "unowned reference block {id} retains objects after their owner is replaced"
        );
    }
}

fn crown_lists(p: &Pack) -> Vec<u32> {
    let model = p.named("beModelCtrlInfo", "i_rareicon").unwrap();
    let action = p.named("beAction2Info", "i_rareicon").unwrap();
    let switch = p.val(action, 7).unwrap();
    vec![
        p.val(model, 6).unwrap(),
        p.val(action, 6).unwrap(),
        p.val(switch, 7).unwrap(),
        p.val(switch, 8).unwrap(),
    ]
}

#[test]
fn expanded_crowns_and_imported_scripts_keep_backing_blocks() {
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
        // Release tests supply plain images; compressed-container coverage lives
        // in the integration matrix rather than this resource-ownership check.
        let bytes = std::fs::read(path).unwrap();
        let (base, size) = if crate::gcm::game_id(&bytes).is_ok() {
            let f = crate::gcm::find_file(&bytes, "ndmw.afs").unwrap();
            (f.offset as usize, f.size as usize)
        } else {
            let f = iso9660::find_file(&bytes, "NDMW.AFS").unwrap();
            (f.lba as usize * 2048, f.size as usize)
        };
        let afs = &bytes[base..base + size];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let data = |i: usize| {
            let (offset, size) = entries[i];
            &afs[offset..offset + size]
        };
        let layout = crate::release::detect(&bytes)
            .unwrap()
            .layout_in(&bytes)
            .unwrap();
        let mut random = random::Randomiser::scan(
            afs,
            &entries,
            crate::table::EnemyTable::read(&bytes, &layout).unwrap(),
            RandomOptions {
                seed: 2162128323,
                donors: crate::crown::RANDOM_DONORS,
                warp: None,
                only: Some(vec![679, 686]),
            },
        )
        .unwrap();
        for entry in [679, 686] {
            let source = Pack::parse(data(entry)).unwrap();
            no_orphan_references(&source);
            let initial = materialize(source.b, &expand(source.b).unwrap()).unwrap();
            let expanded = Pack::parse(&initial).unwrap();
            assert_eq!(crown_lists(&source), crown_lists(&expanded));
            for list in crown_lists(&source) {
                assert_eq!(
                    source.val(list, 4).unwrap(),
                    expanded.val(list, 4).unwrap(),
                    "{variable}: crown list {list} backing block"
                );
            }
            no_orphan_references(&expanded);
            let lua = (0..source.ids.len() as u32)
                .find(|&id| source.class(id, "beLuaDataList").is_ok())
                .unwrap();
            let backing = source.val(lua, 4).unwrap();
            let output = random.stage(entry, initial, afs, &entries).unwrap();
            let result = Pack::parse(&output).unwrap();
            assert_eq!(
                result.val(lua, 4).unwrap(),
                backing,
                "{variable}: script registry backing block"
            );
            let scripts = result.array(lua).unwrap();
            assert!(
                scripts.len() > source.array(lua).unwrap().len(),
                "exercise imported callbacks"
            );
            assert_eq!(
                scripts
                    .iter()
                    .copied()
                    .filter(|id| (*id as usize) < source.ids.len())
                    .collect::<Vec<_>>(),
                source.array(lua).unwrap(),
                "authored script lookup order changed"
            );
            for id in source.array(lua).unwrap() {
                assert!(scripts.contains(&id), "original script missing");
            }
            no_orphan_references(&result);
            let sound = &random.sounds[&(entry - 1)];
            no_orphan_references(&Pack::parse(sound).unwrap());
            eprintln!("OWNERSHIP {variable} entry {entry}: crown and script backing IDs preserved; no orphan object references");
        }
    }
}
