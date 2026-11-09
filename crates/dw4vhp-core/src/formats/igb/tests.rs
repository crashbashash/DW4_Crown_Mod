use super::*;

fn legacy_fields(b: &[u8], p: usize, classes: &[Class]) -> Result<BTreeMap<u16, Range<usize>>> {
    let class = u32at(b, p)? as usize;
    let size = u32at(b, p + 4)? as usize;
    let c = classes
        .get(class)
        .ok_or_else(|| bad("invalid record class"))?;
    need(
        size >= 8 && p.checked_add(size).is_some_and(|end| end <= b.len()),
        "invalid record extent",
    )?;
    let mut q = p + 8;
    let mut fields = BTreeMap::new();
    for &(kind, id, bytes) in &c.fields {
        let width = if kind == 4 {
            let len = u32at(b, q)? as usize;
            need(len <= 8192 && len.is_multiple_of(4), "invalid string size")?;
            4 + len
        } else {
            a4(bytes as usize)
        };
        need(q + width <= p + size, "field outside record")?;
        need(fields.insert(id, q..q + width).is_none(), "duplicate field")?;
        q += width;
    }
    need(q == p + size, "record size disagrees with descriptors")?;
    Ok(fields)
}

#[test]
fn shared_field_layouts_match_legacy_ranges_and_malformed_record_rejection() {
    for descriptors in [
        vec![(1, 3, 4), (1, 2, 2), (1, 11, 0)],
        vec![(4, 2, 0), (1, 9, 4), (1, 3, 0)],
        vec![(1, 7, 0), (1, 5, 12), (4, 2, 0), (1, 3, 4)],
        vec![(4, 7, 0), (1, 3, 4), (4, 2, 0), (1, 5, 0)],
        vec![(1, 8, 0), (4, 2, 0), (1, 10, 0)],
        vec![(1, 2, 4), (1, 2, 4)],
        vec![(4, 2, 0), (1, 2, 4)],
        vec![(4, 2, 0), (4, 2, 0)],
    ] {
        let classes = [
            Class {
                name: "igFixture".into(),
                fields: descriptors,
            },
            // An unused duplicate-field class must not invalidate good records.
            Class {
                name: "igUnused".into(),
                fields: vec![(1, 2, 4), (1, 2, 4)],
            },
        ];
        let layouts = classes.iter().map(FieldLayout::compile).collect::<Vec<_>>();
        for offset in [0, 12, 28] {
            for length in [0, 4, 12, 32] {
                let mut bytes = vec![0xb4; offset];
                bytes.extend(words(&[0, 0]));
                let mut lengths = Vec::new();
                for &(kind, _, width) in &classes[0].fields {
                    if kind == 4 {
                        lengths.push(bytes.len());
                        bytes.extend(words(&[length]));
                        bytes.resize(bytes.len() + length as usize, 0x61);
                    } else {
                        bytes.resize(bytes.len() + a4(width as usize), 0xa5);
                    }
                }
                let size = bytes.len() - offset;
                put(&mut bytes, offset + 4, size as u32);
                let compare = |b: &[u8]| {
                    let old = legacy_fields(b, offset, &classes);
                    let new = record(b, offset, &classes, &layouts);
                    assert_eq!(
                        old.is_ok(),
                        new.is_ok(),
                        "{:?} at {offset}, string length {length}",
                        classes[0].fields
                    );
                    if let (Ok(old), Ok(new)) = (old, new) {
                        assert_eq!(
                            old,
                            new.fields.iter().map(|(&id, range)| (id, range)).collect()
                        );
                        for (id, range) in old {
                            assert_eq!(new.field(id).unwrap(), range);
                        }
                    }
                };
                compare(&bytes);
                for declared in [0, 7, (size - 4) as u32, (size + 4) as u32] {
                    let mut malformed = bytes.clone();
                    put(&mut malformed, offset + 4, declared);
                    compare(&malformed);
                }
                for at in lengths {
                    for invalid in [1, 8196, 0xffff_fffc, 0, 4] {
                        let mut malformed = bytes.clone();
                        put(&mut malformed, at, invalid);
                        compare(&malformed);
                    }
                }
            }
        }
    }
}

#[test]
fn repeated_fixed_and_single_string_records_share_offsets_without_sharing_positions() {
    let bytes = fixtures::object_graph();
    let source = Pack::parse(&bytes).unwrap();
    let mut builder = Builder::new(&source);
    let list = builder.obj(1, &[(2, 0), (3, 0)]).unwrap();
    let name = "a-longer-test-node";
    let width = a4(name.len() + 1);
    let mut raw = words(&[4, (12 + width) as u32, width as u32]);
    raw.extend(name.as_bytes());
    raw.resize(12 + width, 0);
    let named = builder.obj_data(2, raw).unwrap();
    let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
    let p = Pack::parse(&out).unwrap();
    assert!(Arc::ptr_eq(
        &p.rec(1).unwrap().fields.layout,
        &p.rec(list).unwrap().fields.layout
    ));
    assert!(Arc::ptr_eq(
        &p.rec(2).unwrap().fields.layout,
        &p.rec(named).unwrap().fields.layout
    ));
    assert_ne!(
        p.rec(1).unwrap().field(2).unwrap(),
        p.rec(list).unwrap().field(2).unwrap()
    );
    assert!(p.array(list).unwrap().is_empty());
    assert!(p.rec(2).unwrap().named(&out, "test-node"));
    assert!(p.rec(named).unwrap().named(&out, name));
    assert_eq!(p.rec(named).unwrap().field(2).unwrap().len(), 4 + width);
}

/// A warm-cache parser benchmark over complete real model and generator packs.
/// DW4_PARSER_ISO / DW4_PARSER_REPORT name external evidence files; no game
/// data is stored in the repository. Compile before timing the executable.
#[test]
#[ignore]
fn profile_retail_parser() {
    let path = std::env::var_os("DW4_PARSER_ISO").expect("parser input ISO");
    let file = std::fs::File::open(path).unwrap();
    // Safety: the benchmark reads its input without modifying it.
    let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
    let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
    let afs = &iso[f.lba as usize * 2048..f.lba as usize * 2048 + f.size as usize];
    let packs = [
        187, 191, 210, 214, 241, 245, 265, 269, 296, 300, 319, 323, 349, 353, 394, 398, 425, 429,
    ]
    .map(|entry| {
        let offset = u32at(afs, 8 + 8 * entry).unwrap() as usize;
        let size = u32at(afs, 12 + 8 * entry).unwrap() as usize;
        &afs[offset..offset + size]
    });
    let started = std::time::Instant::now();
    for _ in 0..50 {
        for bytes in packs {
            std::hint::black_box(Pack::parse(std::hint::black_box(bytes)).unwrap());
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    let output = std::env::var_os("DW4_PARSER_REPORT").expect("parser timing report");
    crate::profiling::write(std::path::Path::new(&output)).unwrap();
    let mut report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    report["wallSeconds"] = seconds.into();
    report["parses"] = (packs.len() * 50).into();
    std::fs::write(output, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    eprintln!(
        "PARSER: {} real pack parses in {seconds:.3}s",
        packs.len() * 50
    );
}

#[test]
fn synthetic_graph_parses_classes_names_and_memory_id_zero() {
    let bytes = fixtures::object_graph();
    let p = Pack::parse(&bytes).unwrap();
    assert!(p.memory[0].is_some());
    assert_eq!(p.array(1).unwrap(), [2, NULL]);
    assert_eq!(p.named("igNamedObject", "test-node").unwrap(), 2);
    assert_eq!(
        class_defs(&bytes).unwrap()[4].parent.as_deref(),
        Some("igObject")
    );
    assert_eq!(materialize(&bytes, &[]).unwrap(), bytes);
}

#[test]
fn growing_one_block_keeps_other_owners_and_ids_intact() {
    let bytes = fixtures::object_graph();
    let p = Pack::parse(&bytes).unwrap();
    assert_eq!(p.mapping[0], p.mapping[3]);
    let original_other = &bytes[p.memory[3].clone().unwrap()];
    let mut builder = Builder::new(&p);
    builder.grow_mem(0, words(&[2])).unwrap();
    builder.modify(1, &[(2, 3), (3, 3)]).unwrap();
    let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
    let q = Pack::parse(&out).unwrap();
    assert_ne!(q.mapping[0], q.mapping[3]);
    assert_eq!(q.ids.len(), p.ids.len());
    assert_eq!(q.array(1).unwrap(), [2, NULL, 2]);
    assert_eq!(&out[q.memory[3].clone().unwrap()], original_other);
    assert_eq!(q.named("igNamedObject", "test-node").unwrap(), 2);
}

#[test]
fn growing_a_block_does_not_resize_copies_reusing_its_descriptor() {
    let bytes = fixtures::object_graph();
    let p = Pack::parse(&bytes).unwrap();
    let mut prep = Builder::new(&p);
    prep.grow_mem(0, words(&[2])).unwrap();
    let bytes = materialize(&bytes, &prep.finish(0).unwrap()).unwrap();
    let p = Pack::parse(&bytes).unwrap();
    assert_eq!(p.mapping.iter().filter(|&&d| d == p.mapping[0]).count(), 1);
    let payload = bytes[p.memory[0].clone().unwrap()].to_vec();
    for copy_first in [true, false] {
        let mut builder = Builder::new(&p).reuse_directories();
        let copy = if copy_first {
            Some(builder.mem(payload.clone(), 0).unwrap())
        } else {
            None
        };
        builder.grow_mem(0, words(&[NULL])).unwrap();
        let copy = copy.unwrap_or_else(|| builder.mem(payload.clone(), 0).unwrap());
        let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
        let q = Pack::parse(&out).unwrap();
        assert_eq!(q.memory[0].as_ref().unwrap().len(), payload.len() + 4);
        assert_eq!(&out[q.memory[copy as usize].clone().unwrap()], &payload);
        assert_eq!(
            out[q.memory[3].clone().unwrap()],
            bytes[p.memory[3].clone().unwrap()]
        );
    }
}

#[test]
fn rewriting_a_registry_keeps_its_backing_id_and_shared_descriptor_peer() {
    let bytes = fixtures::object_graph();
    let p = Pack::parse(&bytes).unwrap();
    let mut builder = Builder::new(&p);
    // A sorted insertion can move old entries as well as extend the list.
    builder.rewrite_mem(0, words(&[NULL, 2, 2])).unwrap();
    builder.modify(1, &[(2, 3), (3, 3)]).unwrap();
    let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
    let q = Pack::parse(&out).unwrap();
    assert_eq!(q.ids.len(), p.ids.len());
    assert_eq!(q.val(1, 4).unwrap(), 0);
    assert_eq!(q.array(1).unwrap(), [NULL, 2, 2]);
    assert_eq!(
        out[q.memory[3].clone().unwrap()],
        bytes[p.memory[3].clone().unwrap()]
    );
    assert!(Builder::new(&p).rewrite_mem(0, words(&[2])).is_err());
    assert!(Builder::new(&p).rewrite_mem(0, vec![0; 9]).is_err());
}

#[test]
fn appending_an_object_uses_null_reference_padding_and_preserves_originals() {
    let bytes = fixtures::object_graph();
    let p = Pack::parse(&bytes).unwrap();
    let mut builder = Builder::new(&p).reuse_directories();
    assert_eq!(builder.obj(2, &[]).unwrap(), 4);
    let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
    let q = Pack::parse(&out).unwrap();
    assert_eq!(&q.mapping[..4], &p.mapping);
    assert_eq!(q.array(1).unwrap(), p.array(1).unwrap());
    assert_eq!(&out[q.memory[5].clone().unwrap()], words(&[NULL]));
    assert!(q.rec(4).unwrap().named(&out, "test-node"));
}

#[test]
fn class_addition_preserves_graph_and_rejects_unknown_parents() {
    let bytes = fixtures::object_graph();
    let definition = ClassDef {
        class: Class {
            name: "igFixtureExtra".into(),
            fields: vec![(1, 2, 4)],
        },
        parent: Some("igObject".into()),
        slots: 3,
    };
    let out = add_classes(&bytes, std::slice::from_ref(&definition)).unwrap();
    let p = Pack::parse(&out).unwrap();
    assert_eq!(p.array(1).unwrap(), [2, NULL]);
    assert_eq!(
        class_defs(&out).unwrap().last().unwrap().parent.as_deref(),
        Some("igObject")
    );
    assert!(add_classes(
        &bytes,
        &[ClassDef {
            parent: Some("igMissing".into()),
            ..definition
        }]
    )
    .is_err());
}

#[test]
fn malformed_directories_and_overlapping_edits_are_refused() {
    let bytes = fixtures::object_graph();
    let p = Pack::parse(&bytes).unwrap();
    let mut bad_root = bytes.clone();
    put(&mut bad_root, p.table + 8 + 2 * p.ids.len(), 99);
    assert!(Pack::parse(&bad_root).is_err());
    let mut bad_extent = bytes.clone();
    put(&mut bad_extent, 24, 0);
    assert!(Pack::parse(&bad_extent).is_err());
    let mut bad_object_count = bytes.clone();
    put(&mut bad_object_count, 20, p.ids.len() as u32 + 1);
    assert!(Pack::parse(&bad_object_count).is_err());
    // Resolving a shared descriptor once must not skip checks on later IDs.
    let mut wrong_class = bytes.clone();
    let mapping = p.table + 8 + 2 * 2;
    wrong_class[mapping..mapping + 2].copy_from_slice(&p.mapping[1].to_le_bytes());
    assert!(Pack::parse(&wrong_class).is_err());
    let mut shared_memory_overrun = bytes.clone();
    let size = p.dirs[p.mapping[0] as usize].field(7).unwrap().start;
    put(&mut shared_memory_overrun, size, 12);
    assert!(Pack::parse(&shared_memory_overrun).is_err());
    assert!(materialize(
        &bytes,
        &[
            Edit {
                range: 5..10,
                bytes: vec![]
            },
            Edit {
                range: 9..11,
                bytes: vec![]
            }
        ]
    )
    .is_err());
}
