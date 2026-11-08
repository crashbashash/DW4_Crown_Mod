//! Retire unused native graphs without renumbering any surviving resource.
//! The loader allocates directory entries even when no registry reaches them.
//! Retired IDs therefore become inert four-byte raw buffers, rather than
//! orphaned model objects or reference-bearing memory blocks.
use super::*;
use std::collections::BTreeSet;

fn graph(p: &Pack) -> Result<Vec<Vec<u32>>> {
    let mut edges = vec![Vec::new(); p.ids.len()];
    for (id, targets) in edges.iter_mut().enumerate() {
        if let Some(index) = p.ids[id] {
            for (slot, _) in graft::ref_slots(p, &p.objects[index]) {
                let target = p.val(id as u32, slot)?;
                if target != NULL {
                    need(
                        (target as usize) < p.ids.len(),
                        "native reference outside pack",
                    )?;
                    targets.push(target);
                }
            }
        } else if let Some(range) = &p.memory[id] {
            // The loader resolves the entire reference block, including spare
            // list capacity. Following just the owner's count is insufficient.
            if p.dirs[p.mapping[id] as usize].val(p.b, 10)? == 0 {
                for word in p.b[range.clone()].as_chunks::<4>().0 {
                    let target = u32::from_le_bytes(*word);
                    if (target as usize) < p.ids.len() {
                        targets.push(target);
                    }
                }
            }
        }
    }
    Ok(edges)
}

fn reachable(edges: &[Vec<u32>], roots: impl IntoIterator<Item = u32>) -> BTreeSet<u32> {
    let mut found = BTreeSet::new();
    let mut pending: Vec<_> = roots.into_iter().collect();
    while let Some(id) = pending.pop() {
        if found.insert(id) {
            pending.extend(&edges[id as usize]);
        }
    }
    found
}

/// A retained object or even an orphan reference block can keep part of a model
/// alive. Preserve its whole transitive closure, including shared geometry.
fn unreferenced(p: &Pack, candidates: &BTreeSet<u32>) -> Result<BTreeSet<u32>> {
    let edges = graph(p)?;
    let protected = reachable(
        &edges,
        (0..p.ids.len() as u32).filter(|id| !candidates.contains(id)),
    );
    Ok(candidates.difference(&protected).copied().collect())
}

fn retire(b: &[u8], dead: &BTreeSet<u32>) -> Result<Vec<u8>> {
    if dead.is_empty() {
        return Ok(b.to_vec());
    }
    let p = Pack::parse(b)?;
    let root_offset = p.table + 8 + 2 * p.ids.len();
    need(
        !dead.contains(&u32at(b, root_offset)?),
        "cannot retire pack root",
    )?;
    let raw = p
        .dirs
        .iter()
        .position(|d| {
            p.classes[d.class].name == "igMemoryDirEntry"
                && d.val(b, 7).is_ok_and(|size| size == 4)
                && d.val(b, 10).is_ok_and(|kind| kind != 0)
        })
        .ok_or_else(|| bad("no inert four-byte memory descriptor"))? as u16;
    let edges = graph(&p)?;
    need(
        edges.iter().enumerate().all(|(id, targets)| {
            dead.contains(&(id as u32)) || targets.iter().all(|target| !dead.contains(target))
        }),
        "retained resource references retired native data",
    )?;
    let mut out = b[..root_offset + 4].to_vec();
    for &id in dead {
        let at = p.table + 8 + 2 * id as usize;
        out[at..at + 2].copy_from_slice(&raw.to_le_bytes());
    }
    let start = out.len();
    let mut objects = 0;
    for (id, object) in p.ids.iter().enumerate() {
        if !dead.contains(&(id as u32)) {
            if let Some(index) = object {
                out.extend(&b[p.objects[*index].range.clone()]);
                objects += 1;
            }
        }
    }
    let object_bytes = out.len() - start;
    let pool = out.len();
    for (id, memory) in p.memory.iter().enumerate() {
        if dead.contains(&(id as u32)) {
            out.extend(words(&[NULL]));
        } else if let Some(range) = memory {
            out.extend(&b[range.start..range.start + a4(range.len())]);
        }
    }
    let pool_bytes = out.len() - pool;
    put(&mut out, 16, object_bytes as u32);
    put(&mut out, 20, objects);
    put(&mut out, 24, pool_bytes as u32);
    let after = Pack::parse(&out)?;
    need(
        after.ids.len() == p.ids.len(),
        "native reclamation changed IDs",
    )?;
    for id in 0..p.ids.len() {
        if dead.contains(&(id as u32)) {
            need(after.ids[id].is_none(), "retired object still allocates")?;
            continue;
        }
        need(
            after.mapping[id] == p.mapping[id],
            "retained directory changed",
        )?;
        if let Some(index) = p.ids[id] {
            need(
                b[p.objects[index].range.clone()] == out[after.rec(id as u32)?.range.clone()],
                "retained object changed during reclamation",
            )?;
        } else if let Some(range) = &p.memory[id] {
            need(
                Some(&b[range.clone()]) == after.memory[id].as_ref().map(|r| &out[r.clone()]),
                "retained memory changed during reclamation",
            )?;
        }
    }
    Ok(out)
}

fn rewrite_list(p: &Pack, builder: &mut Builder, list: u32, entries: &[u32]) -> Result<()> {
    let block = p.val(list, 4)?;
    let range = p
        .memory
        .get(block as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing native list block"))?;
    need(
        entries.len() * 4 <= range.len(),
        "native list cannot grow during reclamation",
    )?;
    let mut values = vec![NULL; range.len() / 4];
    values[..entries.len()].copy_from_slice(entries);
    builder.rewrite_mem(block, words(&values))?;
    builder.modify(
        list,
        &[(2, entries.len() as u32), (3, entries.len() as u32)],
    )
}

fn owned_script(name: &[u8], model: &str) -> bool {
    name.ends_with(format!("_{model}.lua").as_bytes())
}

/// Treat all Lua name lookups as dependencies, not only reviewed ENEMY commands.
/// Own callbacks disappear with an unused model, but still protect partial pools.
pub(super) fn script_dependency(b: &[u8], model: &str, unused: bool) -> Result<bool> {
    pool_script_dependency(b, model, unused, None)
}

pub(super) fn pool_script_dependency(
    b: &[u8],
    model: &str,
    unused: bool,
    reviewed_intro: Option<&str>,
) -> Result<bool> {
    let p = Pack::parse(b)?;
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaData").is_err() {
            continue;
        }
        let name = graft::name_of(&p, p.rec(id)?).unwrap_or_default();
        if reviewed_intro.is_some_and(|script| name == script.as_bytes()) {
            continue;
        }
        if unused && owned_script(name, model) {
            continue;
        }
        let object = p.val(id, 3)?;
        let size = p.val(object, 2)? as usize;
        let range = p
            .memory
            .get(p.val(object, 3)? as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing native script"))?;
        need(size <= range.len(), "native script exceeds memory")?;
        for c in lua_names::constants(&b[range.start..range.start + size])? {
            if c.text.windows(model.len()).any(|w| w == model.as_bytes()) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Remove an unused enemy body's registries, scene branch and exclusively
/// owned resources. Keep companion projectiles/effects registered independently:
/// other attacks can resolve those by name without requiring the enemy body.
pub(super) fn unused(b: &[u8], model: &str) -> Result<Option<Vec<u8>>> {
    if script_dependency(b, model, true)? {
        return Ok(None);
    }
    let p = Pack::parse(b)?;
    let related = |name: &[u8]| name == model.as_bytes();
    let root = u32at(b, p.table + 8 + 2 * p.ids.len())?;
    let mut roots = BTreeSet::new();
    let mut models = BTreeSet::new();
    for id in p.array(root)? {
        let name = graft::name_of(&p, p.rec(id)?).unwrap_or_default();
        if related(name) {
            roots.insert(id);
            if p.class(id, "beAction2Info").is_ok() {
                models.insert(String::from_utf8_lossy(name).into_owned());
            }
        }
    }
    let scripts = (0..p.ids.len() as u32)
        .filter(|&id| p.class(id, "beLuaData").is_ok())
        .filter(|&id| {
            let name = graft::name_of(&p, p.rec(id).unwrap()).unwrap_or_default();
            owned_script(name, model) || models.iter().any(|m| owned_script(name, m))
        })
        .collect::<BTreeSet<_>>();
    roots.extend(&scripts);
    let candidates = reachable(&graph(&p)?, roots.iter().copied());
    let mut builder = Builder::new(&p);
    let entries = p
        .array(root)?
        .into_iter()
        .filter(|id| !roots.contains(id))
        .collect::<Vec<_>>();
    rewrite_list(&p, &mut builder, root, &entries)?;
    let mut children: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for m in &models {
        let switch = p.val(p.named("beAction2Info", m)?, 7)?;
        if let Ok((_, list)) = graft::parent_group(&p, switch) {
            children.entry(list).or_default().insert(switch);
        }
    }
    for (list, remove) in children {
        let entries = p
            .array(list)?
            .into_iter()
            .filter(|id| !remove.contains(id))
            .collect::<Vec<_>>();
        rewrite_list(&p, &mut builder, list, &entries)?;
    }
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaDataList").is_ok() {
            let entries = p.array(id)?;
            let kept = entries
                .iter()
                .copied()
                .filter(|id| !scripts.contains(id))
                .collect::<Vec<_>>();
            if kept.len() != entries.len() {
                rewrite_list(&p, &mut builder, id, &kept)?;
            }
        }
    }
    let detached = materialize(b, &builder.finish(p.val(root, 4)?)?)?;
    let after = Pack::parse(&detached)?;
    let dead = unreferenced(&after, &candidates)?;
    // Action/particle commands also resolve resources by string, without an
    // object-reference edge. A shared command may survive inside the candidate
    // closure, so inspect every surviving object's serialized string fields.
    for (id, index) in after.ids.iter().enumerate() {
        if dead.contains(&(id as u32)) {
            continue;
        }
        let Some(index) = index else { continue };
        let record = &after.objects[*index];
        for &(kind, slot, _) in &after.classes[record.class].fields {
            // Attack hit-region filters describe which model names a hit may
            // match. An absent model is simply not a target; the selector does
            // not instantiate or require its actor/model allocation.
            // A shared material's igAttrSet name can retain an old projectile
            // label; that label is not a model lookup command.
            if slot == 2
                && (after.classes[record.class].name == "beAsTargetModelData"
                    || matches!(
                        after.classes[record.class].name.as_str(),
                        "igAttrSet"
                            | "igSwitch"
                            | "igGroup"
                            | "igActor"
                            | "igTransform"
                            | "igGeometry"
                            | "igAppearance"
                            | "igAnimation"
                            | "igSkeleton"
                    ))
            {
                continue;
            }
            if kind == 4 {
                let range = record.field(slot)?;
                let text = after.b[range.start + 4..range.end]
                    .split(|c| *c == 0)
                    .next()
                    .unwrap_or_default();
                if related(text) || text.windows(model.len()).any(|w| w == model.as_bytes()) {
                    if std::env::var_os("DW4_RECLAIM_TRACE").is_some() {
                        eprintln!(
                            "retain {model}: {} {id} field {slot} names {}",
                            after.classes[record.class].name,
                            String::from_utf8_lossy(text)
                        );
                    }
                    return Ok(None);
                }
            }
        }
    }
    // Unknown external ownership keeps the complete original family intact.
    if !roots.is_subset(&dead) {
        return Ok(None);
    }
    let out = retire(&detached, &dead)?;
    need(
        graft::find_named(&Pack::parse(&out)?, "beModelCtrlInfo", model)?.is_none(),
        "native model remains after retirement",
    )?;
    Ok(Some(out))
}

/// Shrink only audited native pools. The shared mesh/animation
/// definitions survive; redundant private actors and their buffers are retired.
pub(super) fn shrink(b: &[u8], model: &str, count: usize) -> Result<Vec<u8>> {
    let p = Pack::parse(b)?;
    let old = capacity::slots(&p, model)?;
    if count >= old || count == 0 {
        return Ok(b.to_vec());
    }
    let lists = capacity::instance_lists(&p, model)?;
    let mut candidates = BTreeSet::new();
    for index in count..old {
        let roots = lists
            .iter()
            .map(|(_, entries)| entries[index])
            .collect::<Vec<_>>();
        candidates.extend(capacity::private_graph(&p, &roots)?.0);
    }
    let mut builder = Builder::new(&p);
    for (list, entries) in lists {
        rewrite_list(&p, &mut builder, list, &entries[..count])?;
    }
    let action = p.named("beAction2Info", model)?;
    let mask = p.val(p.val(action, 7)?, 8)?;
    let words_count = count.div_ceil(32);
    let block = p.val(mask, 4)?;
    let range = p
        .memory
        .get(block as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing native mask"))?;
    let mut bits = b[range.clone()].to_vec();
    bits[words_count * 4..].fill(0);
    if !count.is_multiple_of(32) {
        let at = (words_count - 1) * 4;
        let word = u32at(&bits, at)? & ((1u32 << (count % 32)) - 1);
        put(&mut bits, at, word);
    }
    builder.rewrite_mem(block, bits)?;
    builder.modify(
        mask,
        &[
            (2, words_count as u32),
            (3, words_count as u32),
            (5, count as u32),
        ],
    )?;
    let detached = materialize(b, &builder.finish(block)?)?;
    let dead = unreferenced(&Pack::parse(&detached)?, &candidates)?;
    retire(&detached, &dead)
}

#[cfg(test)]
pub(super) fn retired_callback(before: &Pack, after: &Pack, id: u32) -> bool {
    if after.class(id, "beLuaData").is_ok() {
        return false;
    }
    assert!(
        after.ids[id as usize].is_none(),
        "retired callback ID reused as an object"
    );
    let name = graft::name_of(before, before.rec(id).unwrap()).unwrap();
    assert!(
        (0..before.ids.len() as u32).any(|model_id| {
            if before.class(model_id, "beModelCtrlInfo").is_err() {
                return false;
            }
            let model =
                std::str::from_utf8(graft::name_of(before, before.rec(model_id).unwrap()).unwrap())
                    .unwrap();
            owned_script(name, model)
                && graft::find_named(after, "beModelCtrlInfo", model)
                    .unwrap()
                    .is_none()
        }),
        "a surviving model or stage lost its callback: {}",
        String::from_utf8_lossy(name)
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let classes = [
            ("igObject", vec![(1u16, 1u16, 4u16)]),
            ("igObjectDirEntry", vec![(1, 11, 4)]),
            ("igMemoryDirEntry", vec![(1, 7, 4), (1, 10, 4)]),
            ("igObjectList", vec![(1, 2, 4), (1, 3, 4), (7, 4, 4)]),
            ("igFixtureResource", vec![(7, 3, 4)]),
        ];
        let mut b = vec![0; 48];
        for (name, fields) in &classes {
            b.extend(name.as_bytes());
            b.push(0);
            if !b.len().is_multiple_of(2) {
                b.push(0);
            }
            for &(kind, slot, size) in fields {
                b.extend(kind.to_le_bytes());
                b.extend(slot.to_le_bytes());
                b.extend(size.to_le_bytes());
            }
        }
        b.resize(0x800, 0);
        b.extend(words(&[1, 12, 3, 1, 12, 4, 2, 16, 8, 0, 2, 16, 4, 1]));
        b.extend(words(&[20, 6]));
        for index in [2u16, 0, 1, 3, 1, 3] {
            b.extend(index.to_le_bytes());
        }
        b.extend(1u32.to_le_bytes());
        b.extend(words(&[3, 20, 2, 2, 0, 4, 12, 3, 4, 12, 3]));
        b.extend(words(&[2, 4, 2, 0xabcdef01]));
        for (at, value) in [(0, 56), (4, 4), (12, 5), (16, 44), (20, 3), (24, 16)] {
            put(&mut b, at, value);
        }
        b
    }

    #[test]
    fn reclamation_preserves_shared_assets_ids_and_raw_data() {
        let b = fixture();
        let p = Pack::parse(&b).unwrap();
        let mut builder = Builder::new(&p);
        rewrite_list(&p, &mut builder, 1, &[4]).unwrap();
        let detached = materialize(&b, &builder.finish(0).unwrap()).unwrap();
        let q = Pack::parse(&detached).unwrap();
        let candidates = BTreeSet::from([2, 3]);
        let dead = unreferenced(&q, &candidates).unwrap();
        assert_eq!(
            dead,
            BTreeSet::from([2]),
            "the other resource still owns buffer 3"
        );
        let result = retire(&detached, &dead).unwrap();
        let r = Pack::parse(&result).unwrap();
        assert_eq!(r.ids.len(), p.ids.len());
        assert_eq!(r.array(1).unwrap(), [4]);
        assert_eq!(r.val(4, 3).unwrap(), 3);
        assert_eq!(&result[r.memory[3].clone().unwrap()], words(&[2]));
        assert_eq!(&result[r.memory[5].clone().unwrap()], words(&[0xabcdef01]));
        assert!(r.ids[2].is_none());
        assert!(result.len() < detached.len());
    }

    #[test]
    fn loader_references_in_spare_capacity_protect_native_objects() {
        let mut b = fixture();
        let p = Pack::parse(&b).unwrap();
        let count = p.rec(1).unwrap().field(2).unwrap().start;
        let block = p.memory[0].clone().unwrap();
        put(&mut b, count, 1);
        put(&mut b, block.start, 4);
        put(&mut b, block.start + 4, 2);
        let p = Pack::parse(&b).unwrap();
        assert_eq!(p.array(1).unwrap(), [4]);
        assert!(unreferenced(&p, &BTreeSet::from([2])).unwrap().is_empty());
        assert!(retire(&b, &BTreeSet::from([2])).is_err());
    }
}
