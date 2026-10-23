use super::*;
use memmap2::Mmap;
use std::{fs::File, path::Path};

// Compare resource graphs without their pack-local IDs or class indexes. Texture
// pixels, sampler settings and blend parameters all contribute to the result.
fn fingerprint(p: &Pack<'_>, root: u32) -> Vec<u8> {
    fn visit(
        p: &Pack<'_>,
        id: u32,
        tables: &HashMap<u32, usize>,
        seen: &mut BTreeMap<u32, usize>,
        out: &mut Vec<u8>,
    ) {
        if id == NULL {
            out.extend(b"null;");
            return;
        }
        if let Some(index) = seen.get(&id) {
            out.extend(format!("ref{index};").bytes());
            return;
        }
        seen.insert(id, seen.len());
        if let Ok(r) = p.rec(id) {
            out.extend(p.classes[r.class].name.bytes());
            let refs: BTreeSet<_> = ref_slots(p, r).map(|(slot, _)| slot).collect();
            for (&slot, range) in &r.fields {
                out.extend(slot.to_le_bytes());
                if refs.contains(&slot) {
                    visit(p, p.val(id, slot).unwrap(), tables, seen, out);
                } else {
                    out.extend(&p.b[range.clone()]);
                }
            }
        } else {
            let range = p.memory[id as usize].clone().unwrap();
            if let Some(&count) = tables.get(&id) {
                out.extend(count.to_le_bytes());
                for i in 0..count {
                    visit(
                        p,
                        u32at(p.b, range.start + 4 * i).unwrap(),
                        tables,
                        seen,
                        out,
                    );
                }
                out.extend(&p.b[range.start + 4 * count..range.end]);
            } else {
                out.extend(&p.b[range]);
            }
        }
        out.push(b';');
    }
    let (_, tables) = closure(p, &[root]).unwrap();
    let mut out = Vec::new();
    visit(p, root, &tables, &mut BTreeMap::new(), &mut out);
    out
}

fn render_states(p: &Pack<'_>, name: &str) -> Vec<Vec<u8>> {
    let class = if name.starts_with("x_") {
        "beAction2Info"
    } else {
        "beParticleCtrl2Info"
    };
    let info = p.named(class, name).unwrap();
    let mut node = p.val(info, 7).unwrap();
    let mut states = Vec::new();
    loop {
        let holders = node_holders(p, node);
        if holders.is_empty() {
            break;
        }
        assert_eq!(holders.len(), 1, "ambiguous render parent for {name}");
        node = holders[0].0;
        let r = p.rec(node).unwrap();
        if p.classes[r.class].name == "igAttrSet" {
            states.push(fingerprint(p, p.val(node, 8).unwrap()));
        }
    }
    states
}

#[test]
fn imported_particles_keep_native_textures_and_render_states() {
    for key in [
        "DW4_RETAIL_USA_ISO",
        "DW4_RETAIL_EUROPE_ISO",
        "DW4_RETAIL_JAPAN_ISO",
        "DW4_RETAIL_GC_USA_ISO",
        "DW4_RETAIL_GC_JAPAN_ISO",
    ] {
        let Some(path) = std::env::var_os(key) else {
            continue;
        };
        let file = File::open(Path::new(&path)).unwrap();
        let image = unsafe { Mmap::map(&file).unwrap() };
        let (base, size) = if crate::release::detect(&image).unwrap().is_gamecube() {
            let f = gcm::find_file(&image, "ndmw.afs").unwrap();
            (f.offset as usize, f.size as usize)
        } else {
            let f = iso9660::find_file(&image, "NDMW.AFS").unwrap();
            (f.lba as usize * 2048, f.size as usize)
        };
        let afs = &image[base..base + size];
        let entry = |i: usize| {
            let off = u32at(afs, 8 + 8 * i).unwrap() as usize;
            let size = u32at(afs, 12 + 8 * i).unwrap() as usize;
            &afs[off..off + size]
        };
        for target in [614, 621] {
            let mut dst = entry(target).to_vec();
            let original = Pack::parse(&dst).unwrap();
            let mut preserved: BTreeMap<_, _> = effect_infos(&original)
                .unwrap()
                .into_iter()
                .filter(|(class, _, _)| class == "beParticleCtrl2Info")
                .take(3)
                .map(|(_, name, _)| {
                    let states = render_states(&original, &name);
                    (name, states)
                })
                .collect();
            for (source, model, effects) in [
                (187, "e_scum", &["y_005eblst01a"][..]),
                (312, "e_mecha3", &["y_021eblst01b"][..]),
                (
                    273,
                    "e_mecha2",
                    &["y_014eblst01a", "y_014eblst01b", "x_014eblst01b"][..],
                ),
            ] {
                let sp = Pack::parse(entry(source)).unwrap();
                let profile = profile(&sp, model).unwrap();
                let missing = profile.missing(&Pack::parse(&dst).unwrap());
                let base = if missing.is_empty() {
                    dst
                } else {
                    add_classes(&dst, &missing).unwrap()
                };
                let (edits, report) = graft(&base, entry(source), &[model.into()]).unwrap();
                dst = materialize(&base, &edits).unwrap();
                let result = Pack::parse(&dst).unwrap();
                for (name, states) in &preserved {
                    assert!(
                        states == &render_states(&result, name),
                        "{key} {target}: existing effect {name} changed"
                    );
                }
                for &effect in effects {
                    let expected = render_states(&sp, effect);
                    assert!(
                        expected.len() >= 3,
                        "fixture needs texture, blend and pass state"
                    );
                    let actual = render_states(&result, effect);
                    assert!(actual.len() >= expected.len());
                    for (level, state) in expected.iter().enumerate() {
                        assert!(
                            state == &actual[level],
                            "{key} {model} -> {target}: render state differs at ancestor {level}"
                        );
                    }
                    preserved.insert(effect.to_string(), actual);
                    eprintln!("PASS {key}: {effect} -> {target}: native texture and render ancestry; prior effects preserved");
                }
                assert!(report.unattached.is_empty(), "{:?}", report.unattached);
                assert!(report.approximate.is_empty(), "{:?}", report.approximate);
            }
        }
    }
}
