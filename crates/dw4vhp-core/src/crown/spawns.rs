//! Copy only the four-player enemy counts, preserving each party-size drop profile.
use super::*;
use crate::plan::DESTRUCTIBLE_ROWS;

pub(super) fn expand(b: &[u8]) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let mut builder = Builder::new(&p);
    let mut profiles = BTreeMap::new();
    let mut lists = BTreeMap::new();
    let mut changed = Vec::new();
    let mut memory_template = None;

    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        if p.classes[p.objects[*index].class].name != "beGeneraterData" {
            continue;
        }
        let id = id as u32;
        let row = p.val(id, 7)? as usize;
        // The row range is validated against the same 649-row retail table as the
        // stat patch. Gimmicks and destructible generators retain their own counts.
        if row >= 649 || DESTRUCTIBLE_ROWS.contains(&row) {
            continue;
        }
        let list = p.val(id, 14)?;
        p.class(list, "beGeneraterPlayerDataList")?;
        let original = p.array(list)?;
        need(original.len() == 4, "expected four spawn-count profiles")?;
        for &profile in &original {
            p.class(profile, "beGeneraterPlayerData")?;
        }
        let count = p.val(original[3], 2)?;
        let max = p.val(original[3], 3)?;
        let mut replacement = original.clone();
        for slot in 0..3 {
            let profile = original[slot];
            if p.val(profile, 2)? == count && p.val(profile, 3)? == max {
                continue;
            }
            let key = (profile, count, max);
            replacement[slot] = if let Some(&existing) = profiles.get(&key) {
                existing
            } else {
                // Clone the source party-size profile, keeping both drop-list
                // references. Selecting the entire four-player profile would
                // also change drops, contrary to this option's scope.
                let new = builder.obj(profile, &[(2, count), (3, max)])?;
                profiles.insert(key, new);
                new
            };
        }
        if replacement == original {
            continue;
        }
        let key = (list, replacement.clone());
        let new_list = if let Some(&existing) = lists.get(&key) {
            existing
        } else {
            let template = p.val(list, 4)?;
            memory_template = Some(template);
            let memory = builder.mem(words(&replacement), template)?;
            let new = builder.obj(list, &[(4, memory)])?;
            lists.insert(key, new);
            new
        };
        builder.modify(id, &[(14, new_list)])?;
        changed.push((id, list, new_list));
    }
    let Some(template) = memory_template else {
        return Ok(Vec::new());
    };
    let edits = builder.finish(template)?;
    let out = materialize(b, &edits)?;
    verify(&p, &out, &changed)?;
    Ok(edits)
}

fn verify(p: &Pack<'_>, out: &[u8], changed: &[(u32, u32, u32)]) -> Result<()> {
    let v = Pack::parse(out)?;
    need(
        p.mapping == v.mapping[..p.mapping.len()]
            && p.b[p.pool..] == out[v.pool..v.pool + p.b.len() - p.pool],
        "spawn patch changed original IDs or memory",
    )?;
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let old = &p.objects[*index];
        let expected = if let Some((_, _, list)) = changed.iter().find(|x| x.0 == id as u32) {
            old.patch(p.b, &[(14, *list)])?
        } else {
            p.b[old.range.clone()].to_vec()
        };
        need(
            out[v.rec(id as u32)?.range.clone()] == expected,
            "spawn patch changed unrelated original object data",
        )?;
    }
    for &(generator, old_list, new_list) in changed {
        need(
            v.val(generator, 14)? == new_list,
            "spawn list was not linked",
        )?;
        let old = p.array(old_list)?;
        let new = v.array(new_list)?;
        need(new.len() == 4, "spawn patch changed profile count")?;
        let counts = [(2, p.val(old[3], 2)?), (3, p.val(old[3], 3)?)];
        for (&before, &after) in old.iter().zip(&new) {
            need(
                p.rec(before)?.patch(p.b, &counts)? == out[v.rec(after)?.range.clone()],
                "spawn profile changed more than enemy counts",
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Construct a small format fixture from metadata and invented values, with
    // shared profiles that require different counts for two enemy generators.
    fn fixture() -> Vec<u8> {
        fn rec(class: u32, values: &[u32]) -> Vec<u8> {
            words(&[class, 8 + 4 * values.len() as u32])
                .into_iter()
                .chain(words(values))
                .collect()
        }
        let classes: [(&str, &[(u16, u16)]); 6] = [
            ("igObject", &[(0, 1)]),
            ("igObjectDirEntry", &[(1, 11)]),
            ("igMemoryDirEntry", &[(1, 7)]),
            ("beGeneraterData", &[(1, 7), (0, 14)]),
            ("beGeneraterPlayerData", &[(1, 2), (1, 3), (0, 4), (0, 5)]),
            ("beGeneraterPlayerDataList", &[(1, 2), (1, 3), (7, 4)]),
        ];
        let mut b = vec![0; 64];
        for (name, fields) in classes {
            b.extend(name.as_bytes());
            b.push(0);
            b.resize((b.len() + 1) & !1, 0);
            for &(ty, ordinal) in fields {
                b.extend(ty.to_le_bytes());
                b.extend(ordinal.to_le_bytes());
                b.extend(4u16.to_le_bytes());
            }
        }
        b.resize(0x800, 0);
        let mut objects = vec![
            (4, vec![1, 1, 12, 13]),
            (4, vec![2, 2, 14, 15]),
            (4, vec![3, 3, 16, 17]),
            (4, vec![5, 5, 18, 19]),
            (4, vec![9, 2, 18, 19]),
            (5, vec![4, 4, 20]),
            (5, vec![4, 4, 21]),
            (3, vec![0, 5]),
            (3, vec![33, 6]),
            (3, vec![370, 5]),
            (3, vec![638, 11]),
            (5, vec![4, 4, 22]),
        ];
        objects.extend((0..8).map(|_| (0, vec![NULL])));
        let pool = words(&[0, 1, 2, 3, 0, 1, 2, 4, 2, 2, 2, 2]);
        let mut directories = Vec::new();
        let mut payload = Vec::new();
        for (class, fields) in &objects {
            directories.extend(rec(1, &[*class]));
            payload.extend(rec(*class, fields));
        }
        for _ in 0..3 {
            directories.extend(rec(2, &[16]));
        }
        put(&mut b, 0, directories.len() as u32);
        put(&mut b, 4, 23);
        put(&mut b, 12, 6);
        put(&mut b, 16, payload.len() as u32);
        put(&mut b, 20, objects.len() as u32);
        put(&mut b, 24, pool.len() as u32);
        b.extend(directories);
        b.extend(words(&[8 + 2 * 23, 23]));
        for id in 0..23u16 {
            b.extend(id.to_le_bytes());
        }
        b.extend(words(&[7]));
        b.extend(payload);
        b.extend(pool);
        b
    }

    #[test]
    fn shared_profiles_get_private_counts_without_changing_drops_or_crates() {
        let b = fixture();
        let p = Pack::parse(&b).unwrap();
        let out = materialize(&b, &expand(&b).unwrap()).unwrap();
        let v = Pack::parse(&out).unwrap();
        for (generator, expected) in [(7, (5, 5)), (8, (9, 2))] {
            let before = p.array(p.val(generator, 14).unwrap()).unwrap();
            let after = v.array(v.val(generator, 14).unwrap()).unwrap();
            for (old, new) in before.into_iter().zip(after) {
                assert_eq!((v.val(new, 2).unwrap(), v.val(new, 3).unwrap()), expected);
                for field in [4, 5] {
                    assert_eq!(p.val(old, field).unwrap(), v.val(new, field).unwrap());
                }
            }
        }
        assert_eq!(v.val(9, 14).unwrap(), 5); // Crate shares the original enemy list.
        assert_eq!(v.array(5).unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(v.val(10, 14).unwrap(), 11); // Already-equal counts do not need clones.
        assert!(expand(&out).unwrap().is_empty());
    }

    #[test]
    fn incomplete_player_profiles_are_refused() {
        let mut b = fixture();
        let r = Pack::parse(&b).unwrap().rec(5).unwrap().field(2).unwrap();
        put(&mut b, r.start, 3);
        assert!(expand(&b).is_err());
    }

    #[test]
    fn destructible_only_packs_need_no_changes() {
        let mut b = fixture();
        for id in [7, 8] {
            let r = Pack::parse(&b).unwrap().rec(id).unwrap().field(7).unwrap();
            put(&mut b, r.start, 370);
        }
        assert!(expand(&b).unwrap().is_empty());
    }
}
