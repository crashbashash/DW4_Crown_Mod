//! Instance resources shared by all generators of a model. Geometry, textures,
//! skeleton definitions and animation clips stay shared; each added slot owns
//! its model state, actor, animation state and mutable scene/list objects.
use super::*;
use std::collections::{BTreeSet, HashMap};

const PRIVATE: &[&str] = &[
    "beBaseInfoRam",
    "beBaseInfoDataList",
    "beModelCtrlInfoRamData",
    "beModelCtrlNode2",
    "beAction2InfoRam",
    "beHitLandDelivInfoRam",
    "beShadow01InfoRam",
    "igAnimationCombiner",
    "igAnimationCombinerBoneInfoListList",
    "igAnimationCombinerBoneInfoList",
    "igAnimationStateList",
    "igIntList",
    "igActor",
    "igActorList",
    "igAnimationModifierList",
    "igAppearanceList",
    "igAttrList",
    "igNodeList",
];

fn private_graph(p: &Pack, roots: &[u32]) -> Result<(BTreeSet<u32>, BTreeMap<u32, usize>)> {
    let mut ids = BTreeSet::new();
    let mut tables = BTreeMap::new();
    let mut queue = roots.to_vec();
    while let Some(id) = queue.pop() {
        if !ids.insert(id) {
            continue;
        }
        let r = p.rec(id)?;
        let class = &p.classes[r.class].name;
        need(
            PRIVATE.contains(&class.as_str()),
            "unknown instance resource",
        )?;
        for (slot, memory) in graft::ref_slots(p, r) {
            let v = p.val(id, slot)?;
            if v == NULL {
                continue;
            }
            if memory {
                need(
                    p.memory.get(v as usize).is_some_and(Option::is_some),
                    "missing private memory",
                )?;
                ids.insert(v);
                if class == "igAnimationCombiner" {
                    need([7, 8, 12, 13].contains(&slot), "unknown combiner memory")?;
                } else if class == "igIntList" {
                    need(slot == 4, "unknown private integer list")?;
                } else {
                    need(
                        class.ends_with("List") && slot == 4,
                        "unknown instance memory",
                    )?;
                    let elements = p.array(id)?;
                    tables.insert(v, elements.len());
                    for e in elements {
                        if e != NULL && PRIVATE.contains(&p.classes[p.rec(e)?.class].name.as_str())
                        {
                            queue.push(e);
                        }
                    }
                }
            } else if PRIVATE.contains(&p.classes[p.rec(v)?.class].name.as_str()) {
                queue.push(v);
            }
        }
    }
    Ok((ids, tables))
}

fn node(p: &Pack, ram: u32) -> Result<u32> {
    p.class(ram, "beBaseInfoRam")?;
    let data = p.array(p.val(ram, 10)?)?;
    need(data.len() == 1, "expected one enemy model datum")?;
    p.class(data[0], "beModelCtrlInfoRamData")?;
    let node = p.val(data[0], 4)?;
    p.class(node, "beModelCtrlNode2")?;
    Ok(node)
}

pub(super) fn slots(p: &Pack, model: &str) -> Result<usize> {
    p.array(p.val(p.named("beModelCtrlInfo", model)?, 6)?)
        .map(|x| x.len())
}

/// Expand the existing pool without changing its original slots or IDs. Returns
/// unchanged bytes when the authored capacity already covers the demand.
pub(super) fn expand(b: &[u8], model: &str, count: usize) -> Result<Vec<u8>> {
    let p = Pack::parse(b)?;
    let info = p.named("beModelCtrlInfo", model)?;
    let ramlist = p.val(info, 6)?;
    let mut rams = p.array(ramlist)?;
    let old = rams.len();
    if count <= old {
        return Ok(b.to_vec());
    }
    need(
        old > 0 && count <= u16::MAX as usize,
        "invalid instance capacity",
    )?;
    let action = p.named("beAction2Info", model)?;
    let actionlist = p.val(action, 6)?;
    let mut actions = p.array(actionlist)?;
    let switch = p.val(action, 7)?;
    p.class(switch, "igSwitch")?;
    let parent = p.val(switch, 7)?;
    p.class(parent, "igNodeList")?;
    let mut nodes = p.array(parent)?;
    need(
        actions.len() == old && nodes.len() == old,
        "model instance counts differ",
    )?;
    for ((&ram, &ar), &n) in rams.iter().zip(&actions).zip(&nodes) {
        need(
            node(&p, ram)? == n && p.val(ar, 11)? == n,
            "model/animation scene mismatch",
        )?;
    }
    let mask = p.val(switch, 8)?;
    p.class(mask, "igBitMask")?;
    need(
        p.val(mask, 5)? as usize == old,
        "model visibility count differs",
    )?;
    // The animation database owns a separate combiner pool. Physics and
    // shadow infos can also have their own slot lists; all use the same index.
    let mut auxiliary = Vec::new();
    if let Some(database) = graft::find_named(&p, "igAnimationDatabase", model)? {
        let list = p.val(database, 9)?;
        auxiliary.push((list, p.array(list)?));
    }
    for class in ["beHitLandDelivInfo", "beShadow01Info"] {
        if let Some(info) = graft::find_named(&p, class, model)? {
            let list = p.val(info, 6)?;
            let entries = p.array(list)?;
            if !entries.is_empty() {
                auxiliary.push((list, entries));
            }
        }
    }
    need(
        auxiliary.iter().all(|(_, entries)| entries.len() == old),
        "linked instance counts differ",
    )?;
    let roots = |n: usize| {
        std::iter::once(rams[n])
            .chain(std::iter::once(actions[n]))
            .chain(auxiliary.iter().map(|(_, entries)| entries[n]))
            .collect::<Vec<_>>()
    };
    let (ids, tables) = private_graph(&p, &roots(0))?;
    // The authored instances provide an independent boundary check: anything
    // reachable only from the first slot must be copied, never silently shared.
    if old > 1 {
        let (first, _) = graft::closure(&p, &roots(0))?;
        let (second, _) = graft::closure(&p, &roots(1))?;
        need(
            first.difference(&second).all(|id| ids.contains(id)),
            "unhandled private instance resource",
        )?;
    }
    let mut builder = Builder::new(&p).reuse_directories();
    let mut copies = Vec::new();
    for _ in old..count {
        let base = p.ids.len() + builder.mapping.len();
        need(
            base + ids.len() < 1_000_000,
            "expanded instances exceed ID table limit",
        )?;
        let map: HashMap<u32, u32> = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, (base + i) as u32))
            .collect();
        let translate = |id| map.get(&id).copied().unwrap_or(id);
        for &id in &ids {
            let issued = if let Some(ix) = p.ids[id as usize] {
                let r = &p.objects[ix];
                let changes = graft::ref_slots(&p, r)
                    .map(|(s, _)| Ok((s, translate(p.val(id, s)?))))
                    .collect::<Result<Vec<_>>>()?;
                builder.obj(id, &changes)?
            } else {
                let range = p.memory[id as usize]
                    .clone()
                    .ok_or_else(|| bad("missing instance memory"))?;
                let mut data = p.b[range].to_vec();
                for i in 0..tables.get(&id).copied().unwrap_or(0) {
                    let v = u32at(&data, i * 4)?;
                    put(&mut data, i * 4, translate(v));
                }
                builder.mem(data, id)?
            };
            need(issued == map[&id], "instance ID drift")?;
        }
        rams.push(map[&rams[0]]);
        actions.push(map[&actions[0]]);
        nodes.push(map[&nodes[0]]);
        for (_, entries) in &mut auxiliary {
            entries.push(map[&entries[0]]);
        }
        copies.push(map);
    }
    for (list, elements) in [(ramlist, &rams), (actionlist, &actions), (parent, &nodes)]
        .into_iter()
        .chain(auxiliary.iter().map(|(list, entries)| (*list, entries)))
    {
        // Keep the original backing block: the IGB loader resolves its object
        // references even if the list is redirected to a replacement block.
        // Leaving that block orphaned keeps its original instances alive.
        builder.grow_mem(p.val(list, 4)?, words(&elements[old..]))?;
        builder.modify(list, &[(2, count as u32), (3, count as u32)])?;
    }
    let word_count = count.div_ceil(32);
    let mut bits = vec![0; word_count * 4];
    let old_bits = p.array(mask)?;
    need(
        old_bits.len() <= word_count,
        "oversized original visibility mask",
    )?;
    for (i, word) in old_bits.iter().enumerate() {
        put(&mut bits, i * 4, *word);
    }
    builder.grow_mem(p.val(mask, 4)?, bits[old_bits.len() * 4..].to_vec())?;
    builder.modify(
        mask,
        &[
            (2, word_count as u32),
            (3, word_count as u32),
            (5, count as u32),
        ],
    )?;
    let grows = builder.grown();
    let edits = builder.finish(p.val(ramlist, 4)?)?;
    let out = materialize(b, &edits)?;
    let v = Pack::parse(&out)?;
    need(
        v.array(ramlist)? == rams
            && v.array(actionlist)? == actions
            && v.array(parent)? == nodes
            && v.val(mask, 5)? as usize == count,
        "expanded instance graph differs",
    )?;
    need(
        nodes.iter().collect::<BTreeSet<_>>().len() == count,
        "instances share a scene root",
    )?;
    for (list, entries) in &auxiliary {
        need(v.array(*list)? == *entries, "linked pool expansion differs")?;
    }
    // Verify every copied payload, including references that intentionally stay
    // shared. This also proves actors/lists are remapped into their own slot.
    for map in copies {
        for (&src, &dst) in &map {
            let translate = |id| map.get(&id).copied().unwrap_or(id);
            if p.ids[src as usize].is_some() {
                let r = p.rec(src)?;
                let changes = graft::ref_slots(&p, r)
                    .map(|(s, _)| Ok((s, translate(p.val(src, s)?))))
                    .collect::<Result<Vec<_>>>()?;
                need(
                    r.patch(b, &changes)? == out[v.rec(dst)?.range.clone()],
                    "instance clone differs",
                )?;
            } else {
                let mut expected = b[p.memory[src as usize].clone().unwrap()].to_vec();
                for i in 0..tables.get(&src).copied().unwrap_or(0) {
                    let id = u32at(&expected, i * 4)?;
                    put(&mut expected, i * 4, translate(id));
                }
                need(
                    expected == out[v.memory[dst as usize].clone().unwrap()],
                    "instance memory differs",
                )?;
            }
        }
    }
    graft::verify_pool(&p, &v, &grows)?;
    need(
        p.mapping.iter().enumerate().all(|(id, &entry)| {
            v.mapping[id] == entry || (p.memory[id].is_some() && v.memory[id].is_some())
        }),
        "original instance IDs changed",
    )?;
    for (id, ix) in p.ids.iter().enumerate() {
        if let Some(ix) = ix {
            if ![ramlist, actionlist, parent, mask].contains(&(id as u32))
                && !auxiliary.iter().any(|(list, _)| *list == id as u32)
            {
                need(
                    b[p.objects[*ix].range.clone()] == out[v.rec(id as u32)?.range.clone()],
                    "unrelated instance object changed",
                )?;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_instance_pools_have_private_actors_and_wide_masks() {
        let mut ran = 0;
        for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
            let Some(path) = std::env::var_os(variable) else {
                continue;
            };
            let iso = std::fs::read(path).unwrap();
            let afs = if variable.contains("GC") {
                let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
                &iso[f.offset as usize..(f.offset + f.size) as usize]
            } else {
                let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                &iso[f.lba as usize * 2048..f.lba as usize * 2048 + f.size as usize]
            };
            let mut seen = BTreeSet::new();
            for e in 0..u32at(afs, 4).unwrap() as usize {
                let (o, l) = (
                    u32at(afs, 8 + 8 * e).unwrap() as usize,
                    u32at(afs, 12 + 8 * e).unwrap() as usize,
                );
                let b = &afs[o..o + l];
                if memchr::memmem::find(b, b"KIND_ENEMY\0").is_none() {
                    continue;
                }
                let p = Pack::parse(b).unwrap();
                for id in 0..p.ids.len() as u32 {
                    if p.class(id, "beModelCtrlInfo").is_err() {
                        continue;
                    }
                    let Some(name) = graft::name_of(&p, p.rec(id).unwrap())
                        .and_then(|n| std::str::from_utf8(n).ok())
                    else {
                        continue;
                    };
                    if !name.starts_with("e_")
                        || seen.contains(name)
                        || slots(&p, name).unwrap() < 2
                    {
                        continue;
                    }
                    seen.insert(name.to_string());
                    let out = expand(b, name, 70)
                        .unwrap_or_else(|err| panic!("{variable} {e} {name}: {err}"));
                    let v = Pack::parse(&out).unwrap();
                    let info = v.named("beModelCtrlInfo", name).unwrap();
                    let action = v.named("beAction2Info", name).unwrap();
                    let rams = v.array(v.val(info, 6).unwrap()).unwrap();
                    let actions = v.array(v.val(action, 6).unwrap()).unwrap();
                    assert_eq!(rams.len(), 70);
                    for (class, slot) in [
                        ("beModelCtrlInfo", 6),
                        ("beAction2Info", 6),
                        ("igAnimationDatabase", 9),
                        ("beHitLandDelivInfo", 6),
                        ("beShadow01Info", 6),
                    ] {
                        let Some(info) = graft::find_named(&p, class, name).unwrap() else {
                            continue;
                        };
                        let list = p.val(info, slot).unwrap();
                        if p.array(list).unwrap().is_empty() {
                            continue;
                        }
                        assert_eq!(
                            v.val(list, 4).unwrap(),
                            p.val(list, 4).unwrap(),
                            "{name} {class}: expansion must keep the original backing block"
                        );
                    }
                    let mask = v.val(v.val(action, 7).unwrap(), 8).unwrap();
                    assert_eq!(v.val(mask, 5).unwrap(), 70);
                    assert_eq!(v.array(mask).unwrap().len(), 3);
                    if name == "e_tyrano" {
                        // Object references are 32-bit even though remap entries
                        // select 16-bit directory descriptors. Reusing descriptors
                        // must allow a large pool to cross the 65535-ID boundary.
                        let large = expand(b, name, 4000).unwrap();
                        let large = Pack::parse(&large).unwrap();
                        assert!(large.ids.len() > u16::MAX as usize);
                        assert!(large.dirs.len() < u16::MAX as usize);
                        assert_eq!(slots(&large, name).unwrap(), 4000);
                    }
                    let old = slots(&p, name).unwrap();
                    let mut linked = Vec::new();
                    let database = v.named("igAnimationDatabase", name).unwrap();
                    let combiners = v.array(v.val(database, 9).unwrap()).unwrap();
                    assert_eq!(
                        combiners.len(),
                        70,
                        "{name} must have an animation combiner for every slot"
                    );
                    linked.push(combiners);
                    for class in ["beHitLandDelivInfo", "beShadow01Info"] {
                        if let Some(info) = graft::find_named(&v, class, name).unwrap() {
                            let entries = v.array(v.val(info, 6).unwrap()).unwrap();
                            if !entries.is_empty() {
                                assert_eq!(entries.len(), 70);
                                linked.push(entries);
                            }
                        }
                    }
                    let mut private = BTreeSet::new();
                    for n in old..70 {
                        let roots = std::iter::once(rams[n])
                            .chain(std::iter::once(actions[n]))
                            .chain(linked.iter().map(|entries| entries[n]))
                            .collect::<Vec<_>>();
                        let (ids, _) = private_graph(&v, &roots).unwrap();
                        for id in ids {
                            assert!(private.insert(id), "{name} new slots share mutable data");
                        }
                        assert_eq!(node(&v, rams[n]).unwrap(), v.val(actions[n], 11).unwrap());
                    }
                    assert_eq!(
                        expand(&out, name, 70).unwrap(),
                        out,
                        "already large enough is a no-op"
                    );
                }
                if e == 614 {
                    let expanded = materialize(b, &super::super::expand(b).unwrap()).unwrap();
                    let out = expand(&expanded, "i_rareicon", 130).unwrap();
                    assert_eq!(
                        slots(&Pack::parse(&out).unwrap(), "i_rareicon").unwrap(),
                        130
                    );
                }
            }
            assert!(
                seen.len() >= 20,
                "exercise all ordinary enemy families: {seen:?}"
            );
            ran += 1;
        }
        if ran == 0 {
            eprintln!("SKIP: no retail PS2/GameCube instance fixtures configured");
        }
    }
}
