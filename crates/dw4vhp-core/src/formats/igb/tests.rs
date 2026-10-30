use super::*;

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
