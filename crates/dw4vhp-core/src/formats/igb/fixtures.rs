//! Small test-owned IGB object graph, assembled from format metadata only.
//! ID 0 and ID 3 deliberately share one memory descriptor: growing ID 0 must
//! not alter ID 3, and a generated reference block must never retain ID 0.
use super::*;

pub(crate) fn object_graph() -> Vec<u8> {
    let classes = [
        Class {
            name: "igObject".into(),
            fields: vec![(1, 1, 4)],
        },
        Class {
            name: "igObjectDirEntry".into(),
            fields: vec![(1, 11, 4)],
        },
        Class {
            name: "igMemoryDirEntry".into(),
            fields: vec![(1, 7, 4)],
        },
        Class {
            name: "igObjectList".into(),
            fields: vec![(1, 2, 4), (1, 3, 4), (7, 4, 4)],
        },
        Class {
            name: "igNamedObject".into(),
            fields: vec![(4, 2, 0)],
        },
    ];
    let names_start = 48 + 24 * classes.len();
    let mut names = Vec::new();
    let mut definitions = Vec::new();
    for (index, class) in classes.iter().enumerate() {
        let start = names_start + names.len();
        let padded = (start + class.name.len() + 2) & !1;
        definitions.extend(words(&[
            (padded - start) as u32,
            1,
            0,
            class.fields.len() as u32,
            if index >= 3 { 0 } else { NULL },
            12,
        ]));
        names.extend(class.name.as_bytes());
        names.resize(names.len() + padded - start - class.name.len(), 0);
        for &(kind, ordinal, size) in &class.fields {
            names.extend(kind.to_le_bytes());
            names.extend(ordinal.to_le_bytes());
            names.extend(size.to_le_bytes());
        }
    }
    let mut out = vec![0; 48];
    out.extend(&definitions);
    out.extend(&names);
    out.resize(0x800, 0);
    // Three descriptors and four IDs; memory IDs 0/3 share descriptor 2.
    out.extend(words(&[1, 12, 3, 1, 12, 4, 2, 12, 8]));
    out.extend(words(&[16, 4]));
    for index in [2u16, 0, 1, 2] {
        out.extend(index.to_le_bytes());
    }
    out.extend(1u32.to_le_bytes()); // Root object ID.
    let object_start = out.len();
    out.extend(words(&[3, 20, 2, 2, 0]));
    let name = b"test-node\0";
    let width = a4(name.len());
    out.extend(words(&[4, (12 + width) as u32, width as u32]));
    out.extend(name);
    out.resize(out.len() + width - name.len(), 0);
    let object_bytes = out.len() - object_start;
    out.extend(words(&[2, NULL]));
    out.extend(words(&[0xfeedface, 0xaabbccdd]));
    for (word, value) in [
        (0, 36),
        (1, 3),
        (2, definitions.len() + names.len()),
        (3, classes.len()),
        (4, object_bytes),
        (5, 2),
        (6, 16),
    ] {
        put(&mut out, word * 4, value as u32);
    }
    out
}
