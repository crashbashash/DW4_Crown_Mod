use super::*;

#[test]
fn retail_train_sides_are_ranged_on_every_difficulty_and_boarders_stay_native() {
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
        let file = std::fs::File::open(path).unwrap();
        let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
        let release = crate::release::detect(&iso).unwrap();
        let (offset, length) = if release.is_gamecube() {
            let file = gcm::find_file(&iso, "NDMW.AFS").unwrap();
            (file.offset as usize, file.size as usize)
        } else {
            let file = iso9660::find_file(&iso, "NDMW.AFS").unwrap();
            (file.lba as usize * 2048, file.size as usize)
        };
        let afs = &iso[offset..offset + length];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|index| {
                (
                    u32at(afs, 8 + 8 * index).unwrap() as usize,
                    u32at(afs, 12 + 8 * index).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let data = |entry: usize| {
            let (offset, length) = entries[entry];
            &afs[offset..offset + length]
        };
        let original = Pack::parse(data(658)).unwrap();
        let boarder = original.named("beModelCtrlInfo", "e_mecha1").unwrap();
        let roots = original
            .array(original.val(boarder, 5).unwrap())
            .unwrap()
            .into_iter()
            .filter(|&id| {
                original.class(id, "beModelCtrlInfoDataSeq").is_ok()
                    || original.class(id, "beModelCtrlInfoDataAIM").is_ok()
            })
            .collect::<Vec<_>>();
        let boarder_actions = graft::closure(&original, &roots).unwrap().0;
        for seed in [7, 67, 1520] {
            let table = EnemyTable::read(&iso, &release.layout_in(&iso).unwrap()).unwrap();
            let mut random = Randomiser::scan(
                afs,
                &entries,
                table,
                Options {
                    picks_only: false,
                    seed,
                    donors: DONORS,
                    warp: None,
                    only: Some(vec![658]),
                    random_bosses: false,
                },
            )
            .unwrap();
            let normal = random
                .stage(658, data(658).to_vec(), afs, &entries)
                .unwrap();
            let mut rosters = Vec::new();
            for (difficulty, pack) in [(0, 660), (1, 659), (2, 661)] {
                let resource_bytes = random.model_for_generator(658, pack, &normal);
                let resources = Pack::parse(resource_bytes).unwrap();
                assert_eq!(
                    super::super::ranged_ai::apply(resource_bytes, 658).unwrap(),
                    resource_bytes,
                    "completed train policy is idempotent"
                );
                for &id in &boarder_actions {
                    let Some(index) = original.ids[id as usize] else {
                        continue;
                    };
                    let record = &original.objects[index];
                    if matches!(
                        original.classes[record.class].name.as_str(),
                        "beModelCtrlAIData" | "beModelCtrlAIMap" | "beModelCtrlData"
                    ) {
                        let after = resources.rec(id).unwrap();
                        assert_eq!(
                            &original.b[record.range.clone()],
                            &resources.b[after.range.clone()],
                            "{variable} seed {seed} mode {difficulty}: native MR01 action {id}"
                        );
                    }
                }
                let before = Pack::parse(data(pack)).unwrap();
                let after = random.retarget(pack, data(pack)).unwrap().unwrap();
                let after = Pack::parse(&after).unwrap();
                let changed = generators(&after).unwrap();
                let (mut boarders, mut sides) = (0, 0);
                let mut families = BTreeSet::new();
                let mut exported = Vec::new();
                for generator in generators(&before).unwrap() {
                    let new = changed
                        .iter()
                        .find(|new| new.name == generator.name)
                        .unwrap();
                    let protected = protected_entrance(&generator);
                    let model_row = random.rewards[&658]
                        .rows
                        .get(&(new.row as usize))
                        .map_or(new.row, |variant| variant.combat as u32);
                    if protected {
                        assert_eq!(family(generator.row), Some("e_mecha1"));
                        assert_eq!(generator.entrance, "SQ_FALL");
                        assert_eq!(new.row, generator.row);
                        assert_eq!(new.entrance, generator.entrance);
                        assert_eq!(new.start, generator.start);
                        boarders += 1;
                    } else if family(generator.row).is_some() {
                        let model = family(model_row).unwrap();
                        assert!(super::super::encounters::RIVER_RANGED.contains(&model));
                        resources.named("beModelCtrlInfo", model).unwrap();
                        families.insert(model.to_string());
                        sides += 1;
                    }
                    assert_eq!(generator.pos, new.pos);
                    assert_eq!(generator.most, new.most);
                    assert_eq!(generator.population, new.population);
                    exported.push(serde_json::json!({"name":new.name,"family":family(model_row),"position":new.pos,"most":new.most,"protected":protected}));
                }
                assert_eq!((boarders, sides), (38, 26));
                assert_eq!(families.len(), DONORS);
                eprintln!("TRAIN {variable} seed {seed} difficulty {difficulty}: {families:?}; {boarders} native falling generators, {sides} ranged side generators");
                if let Some(path) = env::var_os("DW4_TRAIN_EXPORT") {
                    let path = std::path::PathBuf::from(path);
                    std::fs::create_dir_all(&path).unwrap();
                    std::fs::write(
                        path.join(format!("{variable}-{seed}-{difficulty}.json")),
                        serde_json::to_vec_pretty(&exported).unwrap(),
                    )
                    .unwrap();
                }
                rosters.push(families);
            }
            assert_ne!(rosters[0], rosters[1]);
            assert_ne!(rosters[0], rosters[2]);
            assert_ne!(rosters[1], rosters[2]);
        }
    }
}
