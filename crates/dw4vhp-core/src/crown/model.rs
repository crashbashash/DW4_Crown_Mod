//! Expand crown model, animation and visibility capacity.
use super::*;

pub(super) fn expand(b: &[u8]) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let info = p.named("beModelCtrlInfo", "i_rareicon")?;
    let ramlist = p.val(info, 6)?;
    let mut rams = p.array(ramlist)?;
    need(rams.len() == 4, "original crown capacity must be four")?;
    let mut nodes = Vec::new();
    for &ram in &rams {
        p.class(ram, "beBaseInfoRam")?;
        let one = p.array(p.val(ram, 10)?)?;
        need(one.len() == 1, "expected one crown model datum")?;
        p.class(one[0], "beModelCtrlInfoRamData")?;
        nodes.push(p.val(one[0], 4)?);
    }
    let parents = p
        .ids
        .iter()
        .enumerate()
        .filter_map(|(id, x)| x.map(|i| (id, &p.objects[i])))
        .filter(|(id, r)| {
            p.classes[r.class].name == "igNodeList"
                && p.array(*id as u32).ok().as_ref() == Some(&nodes)
        })
        .map(|(id, _)| id as u32)
        .collect::<Vec<_>>();
    need(parents.len() == 1, "expected one crown scene parent")?;
    let parent = parents[0];
    let action = p.named("beAction2Info", "i_rareicon")?;
    let actionlist = p.val(action, 6)?;
    let mut actions = p.array(actionlist)?;
    need(actions.len() == 4, "expected four crown animation slots")?;
    for (a, n) in actions.iter().zip(&nodes) {
        p.class(*a, "beAction2InfoRam")?;
        need(p.val(*a, 11)? == *n, "crown model/animation mismatch")?;
    }
    let switch = p.val(action, 7)?;
    p.class(switch, "igSwitch")?;
    need(p.val(switch, 7)? == parent, "crown switch parent mismatch")?;
    let mask = p.val(switch, 8)?;
    p.class(mask, "igBitMask")?;
    need(
        p.val(mask, 5)? == 4 && p.val(mask, 2)? == 1,
        "unexpected crown visibility mask",
    )?;
    let mut enemy_slots = 0;
    for (id, x) in p.ids.iter().enumerate() {
        if let Some(idx) = x {
            let r = &p.objects[*idx];
            if p.classes[r.class].name == "beModelCtrlInfo"
                && r.field(7).ok().is_some_and(|range| {
                    b.get(range.start + 4..range.end)
                        .is_some_and(|s| s.starts_with(b"KIND_ENEMY\0"))
                })
            {
                enemy_slots += p.val(p.val(id as u32, 6)?, 2)?;
            }
        }
    }
    need(
        enemy_slots + 4 <= CAPACITY,
        "enemy pools exceed crown capacity",
    )?;
    let mut builder = Builder::new(&p);
    for n in 4..CAPACITY as usize {
        let ram = rams[n % 4];
        let dl = p.val(ram, 10)?;
        let one = p.array(dl)?[0];
        let node = p.val(one, 4)?;
        p.class(node, "beModelCtrlNode2")?;
        let child = p.val(node, 7)?;
        let cm = builder.mem(words(&p.array(child)?), p.val(child, 4)?)?;
        let childlist = builder.obj(child, &[(4, cm)])?;
        let attrs = p.val(node, 15)?;
        need(p.array(attrs)?.is_empty(), "nonempty crown overrides")?;
        let attrlist = builder.obj(attrs, &[])?;
        let newnode = builder.obj(node, &[(7, childlist), (15, attrlist)])?;
        let newone = builder.obj(one, &[(4, newnode), (5, NULL)])?;
        let dm = builder.mem(words(&[newone]), p.val(dl, 4)?)?;
        let datalist = builder.obj(dl, &[(4, dm)])?;
        let newram = builder.obj(ram, &[(10, datalist)])?;
        let ar = actions[n % 4];
        let ap = p.val(ar, 12)?;
        let actor = p.val(ar, 13)?;
        need(
            p.array(ap)?.is_empty() && p.array(actor)?.is_empty(),
            "nonempty crown animation template",
        )?;
        let newap = builder.obj(ap, &[])?;
        let newactor = builder.obj(actor, &[])?;
        let newaction = builder.obj(
            ar,
            &[(11, newnode), (12, newap), (13, newactor), (14, NULL)],
        )?;
        rams.push(newram);
        nodes.push(newnode);
        actions.push(newaction);
    }
    for (list, ids) in [(ramlist, &rams), (parent, &nodes), (actionlist, &actions)] {
        // The loader resolves old backing blocks even after their owner moves.
        // Grow the owned block so its original four references are released on unload.
        builder.grow_mem(p.val(list, 4)?, words(&ids[4..]))?;
        builder.modify(list, &[(2, CAPACITY), (3, CAPACITY)])?;
    }
    builder.grow_mem(p.val(mask, 4)?, vec![0; 4])?;
    builder.modify(mask, &[(2, 2), (3, 2), (5, CAPACITY)])?;
    let grows = builder.grown();
    let edits = builder.finish(p.val(parent, 4)?)?;
    let out = materialize(b, &edits)?;
    let v = Pack::parse(&out)?;
    need(
        v.array(ramlist)? == rams
            && v.array(parent)? == nodes
            && v.array(actionlist)? == actions
            && v.val(v.val(switch, 8)?, 5)? == CAPACITY,
        "expanded crown graph failed verification",
    )?;
    for (a, n) in actions.iter().zip(nodes.iter()) {
        need(v.val(*a, 11)? == *n, "expanded animation/node mismatch")?;
    }
    graft::verify_pool(&p, &v, &grows)?;
    need(
        p.ids == v.ids[..p.ids.len()]
            && p.mapping.iter().enumerate().all(|(id, &entry)| {
                v.mapping[id] == entry || (p.memory[id].is_some() && v.memory[id].is_some())
            }),
        "original IDs changed",
    )?;
    for (id, old) in p.ids.iter().enumerate() {
        if let Some(idx) = old {
            let r = &p.objects[*idx];
            let new = v.rec(id as u32)?;
            if ![ramlist, parent, actionlist, mask].contains(&(id as u32)) {
                need(
                    b[r.range.clone()] == out[new.range.clone()],
                    "unrelated original object changed",
                )?;
            }
        }
    }
    let unique = nodes
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    need(unique.len() == CAPACITY as usize, "crown roots are shared")?;
    for (id, idx) in v.ids.iter().enumerate().skip(p.ids.len()) {
        if let Some(idx) = idx {
            let r = &v.objects[*idx];
            for &(ty, n, _) in &v.classes[r.class].fields {
                if ty == 0 || ty == 7 {
                    let value = r.val(&out, n)?;
                    if value != NULL {
                        need(
                            (value as usize) < v.ids.len(),
                            "new reference outside ID table",
                        )?;
                        need(
                            if ty == 0 {
                                v.ids[value as usize].is_some()
                            } else {
                                v.memory[value as usize].is_some()
                            },
                            "new reference has incorrect kind",
                        )?;
                    }
                }
            }
        } else {
            need(v.memory[id].is_some(), "unknown generated resource")?;
        }
    }
    // Retain original pool segments around the growth insertions as source ranges,
    // so all 85 patch plans use only a few MB of RAM.
    let mut compact = Vec::new();
    for e in edits {
        if e.range.start == p.pool && e.range.end == b.len() {
            let objadd = (u32at(&out, 16)? - u32at(b, 16)?) as usize;
            compact.push(Edit {
                range: p.pool..p.pool,
                bytes: e.bytes[..objadd].to_vec(),
            });
            let mut grown = 0;
            for &(at, len) in &grows {
                let start = objadd + at + grown;
                compact.push(Edit {
                    range: p.pool + at..p.pool + at,
                    bytes: e.bytes[start..start + len].to_vec(),
                });
                grown += len;
            }
            compact.push(Edit {
                range: b.len()..b.len(),
                bytes: e.bytes[objadd + b.len() - p.pool + grown..].to_vec(),
            });
        } else {
            compact.push(e);
        }
    }
    Ok(compact)
}
