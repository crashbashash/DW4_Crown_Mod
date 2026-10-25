//! Expands the authored crown resources together: model and animation RAM,
//! private mutable scene roots, and a private two-word visibility mask.
//! No executable offsets or external game metadata are used. Original geometry
//! and model IDs remain intact. Randomized rewards use otherwise unfilled rows
//! in each area's enemy database. Archive order is preserved.
//! The river model pack also receives missile damage and turret crown scripts.
use crate::{error::Result, gcm, iso9660, region::WriteRegion};
use std::{collections::BTreeMap, io::Write, ops::Range};

mod archive;
mod capacity;
mod drops;
pub mod encounters;
mod exp;
pub mod graft;
mod height;
mod lua_names;
mod model;
mod practice;
mod random;
mod rewards;
mod sound;
pub use random::Options as RandomOptions;
pub use random::{DONORS as RANDOM_DONORS, RULES_VERSION as RANDOM_RULES_VERSION};
#[cfg(test)]
mod ownership;
mod river;
mod spawns;

#[cfg(all(test, feature = "devtools"))]
use crate::formats::igb::class_table;
use crate::formats::igb::{
    a2048, a4, add_classes, bad, class_defs, materialize, need, put, u32at, words, Builder,
    ClassDef, Edit, Pack, Record, NULL,
};
pub use archive::CrownPlan;
use model::expand;

pub const CAPACITY: u32 = 64;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_uses_null_references_when_id_zero_is_memory() {
        let Some(path) = std::env::var_os("DW4_RETAIL_GC_USA_ISO") else {
            return;
        };
        let iso = std::fs::read(path).unwrap();
        let file = gcm::find_file(&iso, "NDMW.AFS").unwrap();
        let afs = &iso[file.offset as usize..(file.offset + file.size) as usize];
        // Cliff Dungeon B1's sound pack starts with the root list's memory at
        // ID 0. Resolving a dummy zero as an object corrupts that list's second
        // pointer when the loader increments the supposed object's refcount.
        let offset = u32at(afs, 8 + 8 * 194).unwrap() as usize;
        let size = u32at(afs, 12 + 8 * 194).unwrap() as usize;
        let bytes = &afs[offset..offset + size];
        let p = Pack::parse(bytes).unwrap();
        assert!(p.memory[0].is_some());
        let root = u32at(bytes, p.table + 8 + 2 * p.ids.len()).unwrap();
        assert_eq!(p.val(root, 4).unwrap(), 0);
        let mut builder = Builder::new(&p);
        builder.mem(words(&[NULL]), 0).unwrap();
        let edits = builder.finish(0).unwrap();
        let out = materialize(bytes, &edits).unwrap();
        let q = Pack::parse(&out).unwrap();
        assert_eq!(q.ids.len(), p.ids.len() + 2);
        let padding = q.memory[p.ids.len() + 1].clone().unwrap();
        assert_eq!(&out[padding], &words(&[NULL]));
        assert_eq!(q.array(root).unwrap(), p.array(root).unwrap());
    }

    #[test]
    fn malformed_packs_are_refused_without_panicking() {
        for n in [0, 4, 47, 48, 2048, 32768] {
            assert!(expand(&vec![0; n]).is_err());
        }
        let mut b = vec![0; 64];
        put(&mut b, 12, u32::MAX);
        assert!(expand(&b).is_err());
    }
}

#[cfg(test)]
mod synthetic_graph_tests {
    use super::*;

    #[test]
    fn imported_graph_remaps_references_without_aliasing_original_memory() {
        let bytes = crate::formats::igb::fixtures::object_graph();
        let source = Pack::parse(&bytes).unwrap();
        let destination = Pack::parse(&bytes).unwrap();
        let (ids, owners) = graft::closure(&source, &[1]).unwrap();
        assert_eq!(ids.iter().copied().collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(owners[&0], 2);
        graft::check_blocks(&source, &ids, &owners).unwrap();
        let mut builder = Builder::new(&destination);
        let map = graft::copy_graph(
            &destination,
            &source,
            &mut builder,
            &ids,
            &owners,
            &mut graft::Report::default(),
        )
        .unwrap();
        let out = materialize(&bytes, &builder.finish(0).unwrap()).unwrap();
        graft::verify(
            &destination,
            &source,
            &out,
            &map,
            &owners,
            &[],
            &BTreeMap::new(),
        )
        .unwrap();
        let result = Pack::parse(&out).unwrap();
        assert_eq!(result.array(1).unwrap(), [2, NULL]);
        assert_eq!(result.array(map[&1]).unwrap(), [map[&2], NULL]);
        assert_ne!(result.val(1, 4).unwrap(), result.val(map[&1], 4).unwrap());
    }

    #[test]
    fn dangling_graph_references_fail_before_copying() {
        let mut bytes = crate::formats::igb::fixtures::object_graph();
        let memory = Pack::parse(&bytes).unwrap().memory[0].clone().unwrap();
        put(&mut bytes, memory.start, 99);
        let source = Pack::parse(&bytes).unwrap();
        assert!(graft::closure(&source, &[1]).is_err());
    }
}
