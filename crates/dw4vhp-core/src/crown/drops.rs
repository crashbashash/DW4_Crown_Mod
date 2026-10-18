//! Optional category rebalance. Original tables and shared prop references survive.
use super::*;
use crate::plan::DESTRUCTIBLE_ROWS;

type ItemKey = (Vec<u8>, u32, u32);
#[derive(Clone, Debug, PartialEq, Eq)]
struct Item {
    name: Vec<u8>,
    id: u32,
    rank: u32,
    rate: u32,
}
impl Item {
    fn group(&self) -> Result<usize> {
        match self.name.as_slice() {
            b"weapon" | b"armor1" | b"armor2" | b"abChip" => Ok(0),
            b"insItem" => match self.id {
                0 => Ok(3),
                1..=3 => Ok(1),
                4..=25 => Ok(2),
                _ => Err(bad("unknown install-item ID in loot table")),
            },
            b"conItem" => Ok(3),
            _ => Err(bad("unsupported loot category")),
        }
    }
    fn key(&self) -> ItemKey {
        (self.name.clone(), self.id, self.rank)
    }
}
fn name(p: &Pack<'_>, id: u32) -> Result<Vec<u8>> {
    let r = p.rec(id)?.field(2)?;
    let bytes =
        p.b.get(r.start + 4..r.end)
            .ok_or_else(|| bad("invalid loot name"))?;
    Ok(bytes.split(|b| *b == 0).next().unwrap_or_default().to_vec())
}
fn items(p: &Pack<'_>, id: u32) -> Result<Vec<Item>> {
    p.class(id, "beGeneraterItemData")?;
    let list = p.val(id, 3)?;
    p.class(list, "beGeneraterItemDataOneList")?;
    p.array(list)?
        .into_iter()
        .map(|id| {
            p.class(id, "beGeneraterItemDataOne")?;
            Ok(Item {
                name: name(p, id)?,
                id: p.val(id, 3)?,
                rank: p.val(id, 4)?,
                rate: p.val(id, 5)?,
            })
        })
        .collect()
}
fn eligible(n: &[u8], entries: &[Item]) -> bool {
    matches!(n.first(), Some(b'n' | b'h' | b'v'))
        && !entries.is_empty()
        && entries.iter().all(|i| i.group().is_ok())
        // Preserve authored guaranteed rewards, including tutorial lessons and
        // boss/final rewards. Common quest/epic/no-drop tables fail the name test.
        && entries.iter().map(|i| i.rate as u64).sum::<u64>() < 10000
        && entries.iter().any(|i| i.rate > 0)
}
#[derive(Default)]
pub(super) struct Library {
    pools: BTreeMap<Vec<u8>, Vec<Item>>,
}
impl Library {
    pub(super) fn collect(&mut self, bytes: &[u8]) -> Result<()> {
        let p = Pack::parse(bytes)?;
        for (id, idx) in p.ids.iter().enumerate() {
            let Some(idx) = idx else { continue };
            if p.classes[p.objects[*idx].class].name != "beGeneraterItemData" {
                continue;
            }
            let id = id as u32;
            let n = name(&p, id)?;
            let entries = items(&p, id)?;
            if eligible(&n, &entries) {
                if let Some(old) = self.pools.get(&n) {
                    need(
                        *old == entries,
                        "same named loot pool has conflicting contents",
                    )?;
                } else {
                    self.pools.insert(n, entries);
                }
            }
        }
        Ok(())
    }
    fn donor(&self, prefix: &[u8], group: usize) -> Option<Vec<Item>> {
        // The tutorial's missing equipment category starts with the earliest
        // Goburimon pool, rather than importing later-area weapons.
        let early = [prefix, b"gobr1"].concat();
        self.pools
            .get(&early)
            .into_iter()
            .chain(
                self.pools
                    .iter()
                    .filter(|(n, _)| n.starts_with(prefix))
                    .map(|(_, v)| v),
            )
            .map(|entries| {
                entries
                    .iter()
                    .filter(|i| i.rate > 0 && i.group().ok() == Some(group))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .find(|entries| !entries.is_empty())
    }
}
const TARGETS: [[u32; 4]; 3] = [
    [5000, 1500, 200, 2300],
    [5000, 2000, 400, 1600],
    [5000, 2500, 600, 1400],
];

/// Largest-remainder allocation preserves authored relative weights and produces
/// exact category totals, including small entries that ordinary rounding loses.
fn allocate(entries: &[Item], target: u32) -> Result<Vec<Item>> {
    let sum: u64 = entries.iter().map(|i| i.rate as u64).sum();
    need(sum > 0, "missing reward category")?;
    let mut out = entries.to_vec();
    let mut fractions = Vec::new();
    let mut used = 0;
    for (index, item) in out.iter_mut().enumerate() {
        let product = item.rate as u64 * target as u64;
        item.rate = (product / sum) as u32;
        used += item.rate;
        fractions.push((index, product % sum));
    }
    fractions.sort_by_key(|(i, remainder)| (std::cmp::Reverse(*remainder), *i));
    for &(index, _) in fractions.iter().take((target - used) as usize) {
        out[index].rate += 1;
    }
    Ok(out.into_iter().filter(|i| i.rate > 0).collect())
}
fn rebalance(
    entries: &[Item],
    n: &[u8],
    difficulty: usize,
    local: &Library,
    global: &Library,
) -> Result<Vec<Item>> {
    need(difficulty < 3 && n.len() >= 3, "invalid loot difficulty")?;
    let mut prefix = n[..3].to_vec();
    prefix[0] = b"nhv"[difficulty];
    let mut out = Vec::new();
    for (group, &target) in TARGETS[difficulty].iter().enumerate() {
        let own: Vec<_> = entries
            .iter()
            .filter(|i| i.rate > 0 && i.group().ok() == Some(group))
            .cloned()
            .collect();
        let selected = if own.is_empty() {
            local
                .donor(&prefix, group)
                .or_else(|| global.donor(&prefix, group))
                // Install items/consumables absent from quest-dungeon libraries
                // use basic World 1 pickups. Equipment never crosses areas.
                .or_else(|| {
                    (group != 0)
                        .then(|| global.donor(&[b"nhv"[difficulty], b'Z', b'1'], group))
                        .flatten()
                })
                .or_else(|| {
                    // Some areas have no plug pool on higher difficulties.
                    // Keep that area's lower-difficulty plugs when possible;
                    // otherwise use the basic Normal World 1 plugs.
                    (group == 2)
                        .then(|| {
                            (0..difficulty)
                                .rev()
                                .find_map(|d| {
                                    global.donor(&[b"nhv"[d], prefix[1], prefix[2]], group)
                                })
                                .or_else(|| global.donor(b"nZ1", group))
                        })
                        .flatten()
                })
                .ok_or_else(|| {
                    bad(&format!(
                        "no area-appropriate donor for {} category {group}",
                        String::from_utf8_lossy(n)
                    ))
                })?
        } else {
            own
        };
        out.extend(allocate(&selected, target)?);
    }
    // Retail tests `r -= weight; r < 1`, with r initially 0..9999. The first
    // entry owns the extra zero outcome; compensate so displayed rates are exact.
    out[0].rate -= 1;
    Ok(out)
}

struct Rewrite<'p, 'a> {
    p: &'p Pack<'a>,
    builder: Builder<'p, 'a>,
    local: Library,
    tables: BTreeMap<(u32, usize), u32>,
    lists: BTreeMap<(u32, Vec<u32>), u32>,
    profiles: BTreeMap<(u32, u32, u32), u32>,
    entries: BTreeMap<(ItemKey, u32), u32>,
    template: Option<u32>,
    memory: Option<u32>,
}
impl Rewrite<'_, '_> {
    fn list(&mut self, old: u32, ids: Vec<u32>) -> Result<u32> {
        if self.p.array(old)? == ids {
            return Ok(old);
        }
        let key = (old, ids.clone());
        if let Some(id) = self.lists.get(&key) {
            return Ok(*id);
        }
        let template = self.p.val(old, 4)?;
        self.memory = Some(template);
        let memory = self.builder.mem(words(&ids), template)?;
        let count = ids.len() as u32;
        let id = self
            .builder
            .obj(old, &[(2, count), (3, count), (4, memory)])?;
        self.lists.insert(key, id);
        Ok(id)
    }
    fn table(&mut self, old: u32, difficulty: usize, global: &Library) -> Result<u32> {
        if let Some(id) = self.tables.get(&(old, difficulty)) {
            return Ok(*id);
        }
        let n = name(self.p, old)?;
        let before = items(self.p, old)?;
        if !eligible(&n, &before) {
            return Ok(old);
        }
        let after = rebalance(&before, &n, difficulty, &self.local, global)?;
        let template = self
            .template
            .ok_or_else(|| bad("missing loot-entry template"))?;
        let mut ids = Vec::new();
        for item in after {
            let key = (item.key(), item.rate);
            let id = if let Some(id) = self.entries.get(&key) {
                *id
            } else {
                let r = self.p.rec(template)?;
                let mut raw = r.patch(self.p.b, &[(3, item.id), (4, item.rank), (5, item.rate)])?;
                let field = r.field(2)?;
                let mut string = item.name;
                string.push(0);
                string.resize(a4(string.len()), 0);
                let mut encoded = words(&[string.len() as u32]);
                encoded.extend(string);
                raw.splice(
                    field.start - r.range.start..field.end - r.range.start,
                    encoded,
                );
                let len = raw.len() as u32;
                put(&mut raw, 4, len);
                let id = self.builder.obj_data(template, raw)?;
                self.entries.insert(key, id);
                id
            };
            ids.push(id);
        }
        let list = self.list(self.p.val(old, 3)?, ids)?;
        let id = self.builder.obj(old, &[(3, list)])?;
        self.tables.insert((old, difficulty), id);
        Ok(id)
    }
    fn drop_list(&mut self, old: u32, global: &Library) -> Result<u32> {
        self.p.class(old, "beGeneraterItemDataList")?;
        let mut ids = self.p.array(old)?;
        need(ids.len() == 3, "expected three loot difficulties")?;
        for (difficulty, id) in ids.iter_mut().enumerate() {
            *id = self.table(*id, difficulty, global)?;
        }
        self.list(old, ids)
    }
}

pub(super) fn expand(bytes: &[u8], global: &Library) -> Result<Vec<Edit>> {
    let p = Pack::parse(bytes)?;
    let mut local = Library::default();
    local.collect(bytes)?;
    let template = p.ids.iter().enumerate().find_map(|(id, idx)| {
        idx.filter(|idx| p.classes[p.objects[*idx].class].name == "beGeneraterItemDataOne")
            .map(|_| id as u32)
    });
    let mut rw = Rewrite {
        p: &p,
        builder: Builder::new(&p),
        local,
        tables: BTreeMap::new(),
        lists: BTreeMap::new(),
        profiles: BTreeMap::new(),
        entries: BTreeMap::new(),
        template,
        memory: None,
    };
    let mut changes = BTreeMap::new();
    for (id, idx) in p.ids.iter().enumerate() {
        let Some(idx) = idx else { continue };
        if p.classes[p.objects[*idx].class].name != "beGeneraterData" {
            continue;
        }
        let id = id as u32;
        let row = p.val(id, 7)? as usize;
        if row >= 649 || DESTRUCTIBLE_ROWS.contains(&row) {
            continue;
        }
        let old = p.val(id, 14)?;
        p.class(old, "beGeneraterPlayerDataList")?;
        let mut profiles = p.array(old)?;
        need(profiles.len() == 4, "expected four loot party profiles")?;
        for profile in &mut profiles {
            p.class(*profile, "beGeneraterPlayerData")?;
            let ordinary = rw.drop_list(p.val(*profile, 4)?, global)?;
            let last = rw.drop_list(p.val(*profile, 5)?, global)?;
            if ordinary == p.val(*profile, 4)? && last == p.val(*profile, 5)? {
                continue;
            }
            let key = (*profile, ordinary, last);
            *profile = if let Some(id) = rw.profiles.get(&key) {
                *id
            } else {
                let id = rw.builder.obj(*profile, &[(4, ordinary), (5, last)])?;
                rw.profiles.insert(key, id);
                id
            };
        }
        let new = rw.list(old, profiles)?;
        if old != new {
            rw.builder.modify(id, &[(14, new)])?;
            changes.insert(id, new);
        }
    }
    let Some(template) = rw.memory else {
        return Ok(Vec::new());
    };
    let edits = rw.builder.finish(template)?;
    let out = materialize(bytes, &edits)?;
    let v = Pack::parse(&out)?;
    need(
        p.mapping == v.mapping[..p.mapping.len()]
            && p.b[p.pool..] == out[v.pool..v.pool + p.b.len() - p.pool],
        "loot patch changed original IDs or memory",
    )?;
    for (id, idx) in p.ids.iter().enumerate() {
        let Some(idx) = idx else { continue };
        let expected = if let Some(list) = changes.get(&(id as u32)) {
            p.objects[*idx].patch(bytes, &[(14, *list)])?
        } else {
            bytes[p.objects[*idx].range.clone()].to_vec()
        };
        need(
            out[v.rec(id as u32)?.range.clone()] == expected,
            "loot patch changed unrelated original object",
        )?;
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(name: &[u8], id: u32, rate: u32) -> Item {
        Item {
            name: name.to_vec(),
            id,
            rank: 0,
            rate,
        }
    }
    #[test]
    fn exhaustive_retail_roll_matches_all_three_distributions() {
        let entries = vec![
            item(b"weapon", 0, 1),
            item(b"weapon", 25, 2),
            item(b"armor1", 3, 3),
            item(b"insItem", 1, 7),
            item(b"insItem", 4, 11),
            item(b"conItem", 0, 13),
        ];
        for (difficulty, target) in TARGETS.iter().enumerate() {
            let out = rebalance(
                &entries,
                b"nZ1gobr1",
                difficulty,
                &Library::default(),
                &Library::default(),
            )
            .unwrap();
            let mut actual = [0; 5];
            for roll in 0..10000i32 {
                let mut r = roll;
                let mut group = 4;
                for i in &out {
                    r -= i.rate as i32;
                    if r < 1 {
                        group = i.group().unwrap();
                        break;
                    }
                }
                actual[group] += 1;
            }
            assert_eq!(&actual[..4], target);
            assert_eq!(actual[4], if difficulty == 2 { 500 } else { 1000 });
            assert_eq!(
                out.iter()
                    .filter(|i| i.group().unwrap() == 0)
                    .map(|i| (i.id, i.rank))
                    .collect::<Vec<_>>(),
                vec![(0, 0), (25, 0), (3, 0)]
            );
        }
    }
    #[test]
    fn missing_categories_use_area_donors_but_never_import_equipment_across_areas() {
        let entries = vec![item(b"weapon", 50, 50)];
        let mut global = Library::default();
        global.pools.insert(
            b"nZ1gobr1".to_vec(),
            vec![
                item(b"weapon", 0, 100),
                item(b"insItem", 1, 1),
                item(b"conItem", 0, 10),
            ],
        );
        global
            .pools
            .insert(b"nZ1rare1".to_vec(), vec![item(b"insItem", 4, 1)]);
        let out = rebalance(&entries, b"nQDrank01", 0, &Library::default(), &global).unwrap();
        assert!(out.contains(&item(b"weapon", 50, 4999)));
        assert!(rebalance(
            &[item(b"insItem", 0, 10)],
            b"nZ9bad",
            0,
            &Library::default(),
            &global
        )
        .is_err());
        let tutorial = rebalance(
            &[item(b"insItem", 0, 10)],
            b"nZ1sp04a",
            0,
            &Library::default(),
            &global,
        )
        .unwrap();
        assert_eq!(tutorial[0].id, 0);
    }
    #[test]
    fn missing_very_hard_plugs_use_lower_difficulty_without_changing_xdata_ids() {
        let own = vec![
            item(b"weapon", 50, 10),
            item(b"insItem", 3, 20),
            item(b"conItem", 0, 30),
        ];
        let mut global = Library::default();
        global
            .pools
            .insert(b"hZ1rare1".to_vec(), vec![item(b"insItem", 9, 100)]);
        global
            .pools
            .insert(b"nZ1rare1".to_vec(), vec![item(b"insItem", 4, 100)]);
        let out = rebalance(&own, b"vZ1gobr1", 2, &Library::default(), &global).unwrap();
        assert!(out
            .iter()
            .any(|i| i.name == b"insItem" && i.id == 9 && i.rate == 600));
        assert!(out
            .iter()
            .any(|i| i.name == b"insItem" && i.id == 3 && i.rate == 2500));
        assert!(!out.iter().any(|i| i.name == b"insItem" && i.id == 4));
    }

    // Invented format fixture: an enemy and crate share the entire loot graph,
    // and all three difficulties share one authored table. Rewriting must split
    // those references while leaving the crate and original records untouched.
    fn shared_fixture() -> Vec<u8> {
        fn rec(class: u32, payload: Vec<u8>) -> Vec<u8> {
            let mut out = words(&[class, 8 + payload.len() as u32]);
            out.extend(payload);
            out
        }
        fn string(s: &[u8]) -> Vec<u8> {
            let mut data = s.to_vec();
            data.push(0);
            data.resize(a4(data.len()), 0);
            let mut out = words(&[data.len() as u32]);
            out.extend(data);
            out
        }
        let classes: [(&str, &[(u16, u16)]); 10] = [
            ("igObject", &[(0, 1)]),
            ("igObjectDirEntry", &[(1, 11)]),
            ("igMemoryDirEntry", &[(1, 7)]),
            ("beGeneraterData", &[(1, 7), (0, 14)]),
            ("beGeneraterPlayerData", &[(1, 2), (1, 3), (0, 4), (0, 5)]),
            ("beGeneraterPlayerDataList", &[(1, 2), (1, 3), (7, 4)]),
            ("beGeneraterItemDataList", &[(1, 2), (1, 3), (7, 4)]),
            ("beGeneraterItemData", &[(4, 2), (0, 3)]),
            ("beGeneraterItemDataOneList", &[(1, 2), (1, 3), (7, 4)]),
            ("beGeneraterItemDataOne", &[(4, 2), (1, 3), (1, 4), (1, 5)]),
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
        let mut objects = Vec::new();
        for (name, id) in [
            (b"weapon".as_slice(), 0),
            (b"insItem", 1),
            (b"insItem", 4),
            (b"conItem", 0),
        ] {
            let mut payload = string(name);
            payload.extend(words(&[id, 0, 100]));
            objects.push((9, payload));
        }
        objects.push((8, words(&[4, 4, 12])));
        let mut table = string(b"nZ1test");
        table.extend(words(&[4]));
        objects.push((7, table));
        objects.extend([
            (6, words(&[3, 3, 13])),
            (4, words(&[1, 1, 6, 6])),
            (5, words(&[4, 4, 14])),
            (3, words(&[0, 8])),
            (3, words(&[370, 8])),
            (3, words(&[1, 8])),
        ]);
        let mut dirs = Vec::new();
        let mut payload = Vec::new();
        for (class, fields) in objects {
            dirs.extend(rec(1, words(&[class])));
            payload.extend(rec(class, fields));
        }
        for size in [16, 12, 16] {
            dirs.extend(rec(2, words(&[size])));
        }
        let pool = words(&[0, 1, 2, 3, 5, 5, 5, 7, 7, 7, 7]);
        for (word, value) in [
            (0, dirs.len()),
            (1, 15),
            (3, 10),
            (4, payload.len()),
            (5, 12),
            (6, pool.len()),
            (7, 3),
        ] {
            put(&mut b, word * 4, value as u32);
        }
        b.extend(dirs);
        b.extend(words(&[38, 15]));
        for id in 0..15u16 {
            b.extend(id.to_le_bytes());
        }
        b.extend(words(&[9]));
        b.extend(payload);
        b.extend(pool);
        b
    }

    #[test]
    fn graph_rewrite_splits_shared_difficulties_and_leaves_crate_loot_untouched() {
        let before = shared_fixture();
        let p = Pack::parse(&before).unwrap();
        let out = materialize(&before, &expand(&before, &Library::default()).unwrap()).unwrap();
        let v = Pack::parse(&out).unwrap();
        assert_eq!(v.val(10, 14).unwrap(), 8);
        assert_eq!(items(&v, 5).unwrap(), items(&p, 5).unwrap());
        assert_eq!(v.val(9, 14).unwrap(), v.val(11, 14).unwrap());
        for profile in v.array(v.val(9, 14).unwrap()).unwrap() {
            assert_eq!(
                (v.val(profile, 2).unwrap(), v.val(profile, 3).unwrap()),
                (1, 1)
            );
            for field in [4, 5] {
                for (difficulty, table) in v
                    .array(v.val(profile, field).unwrap())
                    .unwrap()
                    .into_iter()
                    .enumerate()
                {
                    let entries = items(&v, table).unwrap();
                    let mut totals = [0u32; 4];
                    for i in &entries {
                        totals[i.group().unwrap()] += i.rate;
                    }
                    totals[entries[0].group().unwrap()] += 1;
                    assert_eq!(totals, TARGETS[difficulty]);
                }
            }
        }
    }

    #[test]
    fn scripted_empty_and_guaranteed_rewards_are_exempt() {
        assert!(!eligible(b"comonEPIC01", &[item(b"weapon", 1, 100)]));
        assert!(!eligible(b"nZ1boss", &[item(b"weapon", 1, 10000)]));
        assert!(!eligible(b"nZ1empty", &[]));
        assert!(!eligible(b"nZ1quest", &[item(b"impItem", 0, 500)]));
    }
}
