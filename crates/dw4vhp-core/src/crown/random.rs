//! Experimental enemy randomiser. For every stage it copies a few extra enemy models into the
//! stage's model pack, then points each regular generator at one of the models the stage now
//! carries. The new enemy row is the one in the chosen model's family whose HP and attack are
//! nearest the original's, so a stage keeps its difficulty. Bosses, generators that stage
//! scripts wait on by name, and kill-count stages are left as shipped; cutscenes that wait on
//! an intro enemy by name get their name constants renamed to the model now spawned.
use super::graft::{self, Retarget, INTRO_STAGES};
use super::lua_names;
use super::*;
use crate::{rowgroup::ROW_GROUPS, table::EnemyTable};
use std::{collections::BTreeSet, env};

pub struct Options {
    pub seed: u64,
    /// How many new models to try to copy into each stage.
    pub donors: usize,
    /// For testing: make the Goblin Pass exit lead to this stage instead of the Abyss of Grief.
    pub warp: Option<String>,
    /// Randomise only these model packs (others still supply models); `None` randomises every stage.
    pub only: Option<Vec<usize>>,
}

/// Ordinary enemies: the models that may be copied into other stages.
const REGULAR: &[&str] = &[
    "e_goburi", "e_nume", "e_scum", "e_ogre", "e_mecha1", "e_cockatri", "e_mummy", "e_mecha2",
    "e_tyrano", "e_minotaru", "e_otama", "e_geko", "e_maridevi", "e_mecha3", "e_infermo",
    "e_megadra", "e_rare", "e_impdra", "e_mecha4", "e_mecha5",
];
/// Enemies a generator may be moved off, though they are never copied elsewhere: Pharaohmon
/// summons Mummymon from its own script, and Tortamon loads from a separate file.
const REPLACEABLE: &[&str] = &["e_pharaoh", "e_torta"];
/// Stage packs whose enemy scripts are not the generic ones, so models are never copied from them.
const FORBIDDEN_SOURCES: &[usize] = &[449, 457, 464, 603, 607, 786];
/// The developer world (603), the practice stage (607) and the final bosses: left as shipped.
const SKIPPED: &[usize] = &[603, 607, 730, 737, 744, 751, 758, 765, 786];
/// Generators stage scripts depend on, per model pack: boss fights, waits on a named enemy,
/// script-side spawns. Leaving them alone keeps every door and cutscene working.
const PINNED: &[(usize, &[&str])] = &[
    (187, &["GENE-08", "GENE-01"]),
    (210, &["GENE-12"]),
    (241, &["GENE-13", "INTRO-01"]),
    (265, &["GENE-15"]),
    (296, &["GENE-11"]),
    (319, &["GENE-19"]),
    (349, &["GENE-24"]),
    (394, &["GENE-23", "INTRO-01"]),
    (425, &["GENE-21", "GENE-20"]),
    (441, &["GENE-06"]),
    (510, &["GENE-11"]),
    (518, &["GENE-11"]),
    (526, &["GENE-11"]),
    (556, &["GENE-22", "GENE-16", "GENE-17", "GENE-18", "GENE-19", "GENE-20", "GENE-21"]),
    (579, &["GENE-22", "GENE-20", "GENE-21"]),
    (587, &["GENE-22", "GENE-23"]),
    (595, &["GENE-22", "GENE-23", "GENE-24"]),
    (621, &["GENE-38"]),
    (635, &["GENE-011", "GENE-013", "GENE-016"]),
    (649, &["GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"]),
    (686, &["GENE-131", "GENE-139", "GENE-165", "GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"]),
];

/// Kill-count quest stages: their clear counts kills in the hit scripts of these models and the
/// enemy supply is fixed, so enemies only trade places among them and nothing is copied in.
const RESTRICTED: &[(usize, &[&str])] = &[
    (457, &["e_mummy", "e_mecha2"]),
    (464, &["e_mummy", "e_mecha2"]),
];

/// Models that are a stage's boss though they are ordinary enemies elsewhere.
const BOSS_REGULAR: &[(usize, &str)] = &[(319, "e_mecha4"), (394, "e_mecha5")];

fn family(row: u32) -> Option<&'static str> {
    ROW_GROUPS
        .iter()
        .find(|g| g.rows.iter().any(|&r| r as u32 == row))
        .map(|g| g.model)
}

fn replaceable(model: &str) -> bool {
    REGULAR.contains(&model) || REPLACEABLE.contains(&model)
}

/// The row of `model`'s family with the same rarity whose HP and attack are nearest to `row`'s.
pub(super) fn nearest_row(table: &EnemyTable, row: u32, model: &str) -> Option<u32> {
    let group = ROW_GROUPS.iter().find(|g| g.model == model)?;
    let row = row as usize;
    let (hp, atk) = (table.hp[row].max(1) as f64, table.stat[row][0].max(1) as f64);
    group
        .rows
        .iter()
        .copied()
        .filter(|r| {
            table.rarity[*r] == table.rarity[row]
                && table.hp[*r] > 0
                && !crate::rowgroup::COLLAPSE_SKIP_ROWS.contains(r)
        })
        .min_by(|&a, &b| {
            let score = |r: usize| {
                (table.hp[r].max(1) as f64 / hp).ln().abs() + (table.stat[r][0].max(1) as f64 / atk).ln().abs()
            };
            score(a).total_cmp(&score(b)).then(a.cmp(&b))
        })
        .map(|r| r as u32)
}

struct Rng(u64);
impl Rng {
    fn new(seed: u64, salt: u64) -> Self {
        Self(seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, (self.next() % (i as u64 + 1)) as usize);
        }
    }
}

/// A generator as the planner sees it.
struct Gen {
    name: String,
    row: u32,
    seq: bool,
    start: String,
    pos: [f32; 3],
    /// Most instances the generator keeps alive at once, over the player counts.
    most: u32,
}

fn generators(p: &Pack) -> Result<Vec<Gen>> {
    let mut out = Vec::new();
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let r = &p.objects[*index];
        if p.classes[r.class].name != "beGeneraterData" {
            continue;
        }
        let name = graft::name_of(p, r).map(|n| String::from_utf8_lossy(n).into_owned());
        let Some(name) = name else { continue };
        let seq = r.field(graft::GENERATOR_SEQ)?;
        let seq = seq.len() > 4 && p.b.get(seq.start + 4).is_some_and(|c| *c != 0);
        let mut most = 0;
        for pd in p.array(p.val(id as u32, GENERATOR_PLAYERS)?)? {
            most = most.max(p.val(pd, PLAYER_MAX)?);
        }
        let at = r.field(3)?;
        let mut pos = [0f32; 3];
        for (k, v) in pos.iter_mut().enumerate() {
            *v = f32::from_bits(u32at(p.b, at.start + 4 * k)?);
        }
        let start = r.field(9)?;
        let start = String::from_utf8_lossy(&p.b[start.start + 4..start.end])
            .trim_end_matches('\0')
            .to_string();
        out.push(Gen {
            start,
            pos,
            name,
            row: p.val(id as u32, graft::GENERATOR_ROW)?,
            seq,
            most,
        });
    }
    Ok(out)
}
const GENERATOR_PLAYERS: u16 = 14;
const PLAYER_MAX: u16 = 3;

struct Donor {
    source: usize,
    ram: u32,
    profile: graft::Profile,
    /// Rarities (crown tiers) the model has enemy rows for.
    rarities: BTreeSet<u8>,
}

#[derive(Clone)]
struct Stage {
    /// Generator packs, one per difficulty tier.
    gens: Vec<usize>,
    /// Regular models the pack already carries, with their instance counts.
    residents: BTreeMap<String, u32>,
    pinned: BTreeSet<String>,
    pinned_families: BTreeSet<&'static str>,
}

pub(super) struct Randomiser {
    opts: Options,
    table: EnemyTable,
    /// Models by name and whether the source hangs them as a scene of their own.
    donors: BTreeMap<(&'static str, bool), Donor>,
    stages: BTreeMap<usize, Stage>,
    plans: BTreeMap<usize, Retarget>,
    pub log: Vec<String>,
}

fn rarities(table: &EnemyTable, model: &str) -> BTreeSet<u8> {
    ROW_GROUPS
        .iter()
        .find(|g| g.model == model)
        .map(|g| g.rows.iter().map(|&r| table.rarity[r]).collect())
        .unwrap_or_default()
}

impl Randomiser {
    /// Find every stage and, for each regular model, the stage pack to copy it from.
    pub(super) fn scan(
        afs: &[u8],
        entries: &[(usize, usize)],
        table: EnemyTable,
        opts: Options,
    ) -> Result<Self> {
        let mut me = Self {
            opts,
            table,
            donors: BTreeMap::new(),
            stages: BTreeMap::new(),
            plans: BTreeMap::new(),
            log: Vec::new(),
        };
        let data = |e: usize| entries.get(e).and_then(|&(o, l)| afs.get(o..o + l));
        for (e, &(_, len)) in entries.iter().enumerate() {
            if !(1_500_000..=12_000_000).contains(&len) || SKIPPED.contains(&e) {
                continue;
            }
            let Some(d) = data(e) else { continue };
            if memchr::memmem::find(d, b"KIND_ENEMY\0").is_none() {
                continue;
            }
            let gens: Vec<usize> = (e + 1..=e + 7)
                .filter(|&g| {
                    data(g).is_some_and(|x| x.len() < 1_500_000 && memchr::memmem::find(x, b"beGeneraterData\0").is_some())
                })
                .collect();
            if gens.is_empty() {
                continue;
            }
            let p = Pack::parse(d)?;
            let mut residents = BTreeMap::new();
            for (id, index) in p.ids.iter().enumerate() {
                let Some(index) = index else { continue };
                let r = &p.objects[*index];
                if p.classes[r.class].name == "beModelCtrlInfo"
                    && r.field(7).is_ok_and(|f| p.b[f.start + 4..f.end].starts_with(b"KIND_ENEMY\0"))
                {
                    if let Some(name) = graft::name_of(&p, r) {
                        let name = String::from_utf8_lossy(name).into_owned();
                        residents.insert(name, p.val(p.val(id as u32, 6)?, 2)?);
                    }
                }
            }
            let pinned: BTreeSet<String> = PINNED
                .iter()
                .filter(|(entry, _)| *entry == e)
                .flat_map(|(_, names)| names.iter().map(|n| n.to_string()))
                .collect();
            let first = generators(&Pack::parse(data(gens[0]).unwrap())?)?;
            // Only bosses keep their model away from the other generators: a generator a
            // script merely waits on can still share its model with the rest of the stage.
            let pinned_families = first
                .iter()
                .filter(|g| pinned.contains(&g.name))
                .filter_map(|g| family(g.row))
                .filter(|f| !replaceable(f) || BOSS_REGULAR.contains(&(e, *f)))
                .collect();
            let stage = Stage {
                gens,
                residents,
                pinned,
                pinned_families,
            };
            let own_scene = !graft::has_group(&p, b"EM01");
            for &m in REGULAR {
                if me.donors.contains_key(&(m, own_scene))
                    || FORBIDDEN_SOURCES.contains(&e)
                    || !stage.residents.contains_key(m)
                    || stage.pinned_families.contains(m)
                {
                    continue;
                }
                if let Ok(profile) = graft::profile(&p, m) {
                    me.donors.insert(
                        (m, own_scene),
                        Donor {
                            source: e,
                            ram: profile.ram,
                            rarities: rarities(&me.table, m),
                            profile,
                        },
                    );
                }
            }
            me.stages.insert(e, stage);
        }
        need(!me.stages.is_empty(), "no stages found")?;
        Ok(me)
    }

    pub(super) fn plans_for(&self, entry: usize) -> bool {
        self.plans.contains_key(&entry)
    }

    pub(super) fn is_stage(&self, entry: usize) -> bool {
        self.stages.contains_key(&entry)
    }

    pub(super) fn retarget(&self, entry: usize, b: &[u8]) -> Result<Option<Vec<u8>>> {
        let Some(rt) = self.plans.get(&entry) else {
            return Ok(None);
        };
        let (out, n) = graft::retarget_rows(b, rt)?;
        need(n == rt.rows.len(), "a planned generator is missing from its pack")?;
        Ok(Some(out))
    }

    /// Randomise the stage (see [`Self::plan_stage`]) and apply the test warp to the Goblin Pass.
    pub(super) fn stage(
        &mut self,
        entry: usize,
        dst: Vec<u8>,
        afs: &[u8],
        entries: &[(usize, usize)],
    ) -> Result<Vec<u8>> {
        let mut dst = self.plan_stage(entry, dst, afs, entries)?;
        if let (614, Some(target)) = (entry, self.opts.warp.clone()) {
            let names = BTreeMap::from([("stag0102".to_string(), target)]);
            let more = lua_names::rename_in_pack(&dst, &["matz02.lua".to_string()], &names)?;
            dst = materialize(&dst, &more)?;
        }
        Ok(dst)
    }

    /// Copy donors into the stage's model pack, choose each generator's model, and record the
    /// rows for the stage's generator packs. Returns the new model pack.
    fn plan_stage(
        &mut self,
        entry: usize,
        mut dst: Vec<u8>,
        afs: &[u8],
        entries: &[(usize, usize)],
    ) -> Result<Vec<u8>> {
        let Some(stage) = self.stages.get(&entry).cloned() else {
            return Ok(dst);
        };
        if self.opts.only.as_ref().is_some_and(|only| !only.contains(&entry)) {
            return Ok(dst);
        }
        let data = |e: usize| {
            entries
                .get(e)
                .and_then(|&(o, l)| afs.get(o..o + l))
                .ok_or_else(|| bad("stage entry outside archive"))
        };
        let intro = INTRO_STAGES.iter().find(|s| s.model_entry == entry);
        let sites: Vec<(&str, &str)> = intro
            .map(|s| s.sites.iter().copied().filter(|(g, _)| !stage.pinned.contains(*g)).collect())
            .unwrap_or_default();
        let mut rng = Rng::new(self.opts.seed, entry as u64);
        let first = generators(&Pack::parse(data(stage.gens[0])?)?)?;
        let allowed = RESTRICTED.iter().find(|(e, _)| *e == entry).map(|(_, m)| *m);
        let usable = |f: &str| match allowed {
            Some(a) => a.contains(&f),
            None => replaceable(f),
        };
        let movable = first.iter().any(|g| {
            !stage.pinned.contains(&g.name)
                && family(g.row).is_some_and(|f| usable(f) && !stage.pinned_families.contains(f))
        });
        if !movable {
            self.log.push(format!("stage {entry}: nothing to randomise"));
            return Ok(dst);
        }

        // debugging: DW4_RANDOM_ADDCLASS=beModelCtrlMOVEEX only adds the class definition
        if let Ok(want) = env::var("DW4_RANDOM_ADDCLASS") {
            let dp = Pack::parse(&dst)?;
            let defs = self
                .donors
                .values()
                .flat_map(|d| d.profile.classes.iter())
                .filter(|c| want.split(',').any(|w| w == c.class.name) && !dp.classes.iter().any(|d| d.name == c.class.name))
                .fold(BTreeMap::new(), |mut m, c| {
                    m.insert(c.class.name.clone(), c.clone());
                    m
                });
            let add: Vec<ClassDef> = defs.into_values().collect();
            if !add.is_empty() {
                dst = add_classes(&dst, &add)?;
            }
            self.log.push(format!("stage {entry}: added classes {:?}", add.iter().map(|c| c.class.name.clone()).collect::<Vec<_>>()));
            return Ok(dst);
        }

        // The models the stage will carry.
        let mut pool: Vec<(String, u32, BTreeSet<u8>)> = stage
            .residents
            .iter()
            .filter(|(m, _)| REGULAR.contains(&m.as_str()) || m.as_str() == "e_pharaoh")
            .filter(|(m, _)| !stage.pinned_families.contains(m.as_str()))
            .filter(|(m, _)| allowed.is_none_or(|a| a.contains(&m.as_str())))
            .map(|(m, &ram)| (m.clone(), ram, rarities(&self.table, m)))
            .collect();
        let mut candidates: Vec<(&'static str, bool)> = self
            .donors
            .keys()
            .copied()
            .filter(|(m, _)| !stage.residents.contains_key(*m) && !stage.pinned_families.contains(m))
            .filter(|_| allowed.is_none())
            .collect();
        rng.shuffle(&mut candidates);
        // debugging: DW4_RANDOM_FORCE=e_otama,e_mecha4 tries those models first
        if let Ok(force) = env::var("DW4_RANDOM_FORCE") {
            let first: Vec<_> = force.split(',').collect();
            candidates.sort_by_key(|(m, _)| first.iter().position(|f| f == m).unwrap_or(usize::MAX));
        }
        let (mut added, mut tried) = (0, 0);
        let enough = |pool: &Vec<_>, added: usize| added >= self.opts.donors && pool.len() > sites.len();
        for (m, own_scene) in candidates {
            if enough(&pool, added) || tried >= self.opts.donors + 8 {
                break;
            }
            let donor = &self.donors[&(m, own_scene)];
            if let Some(why) = donor.profile.misfit(&Pack::parse(&dst)?) {
                self.log.push(format!("stage {entry}: {m} does not fit: {why}"));
                continue;
            }
            tried += 1;
            let missing = donor.profile.missing(&Pack::parse(&dst)?);
            let base = if missing.is_empty() { dst.clone() } else { add_classes(&dst, &missing)? };
            match graft::graft(&base, data(donor.source)?, &[m.to_string()]) {
                Ok((edits, _)) => {
                    dst = materialize(&base, &edits)?;
                    pool.push((m.to_string(), donor.ram, donor.rarities.clone()));
                    added += 1;
                    self.log.push(format!("stage {entry}: + {m} from {}", donor.source));
                }
                Err(err) => self.log.push(format!("stage {entry}: {m} not copied: {err}")),
            }
        }
        if pool.len() < 2 {
            self.log.push(format!("stage {entry}: pool of {} models, left as shipped", pool.len()));
            return Ok(dst);
        }

        // The first tier decides which generators change and what they become.
        let mut choice: BTreeMap<String, String> = BTreeMap::new();
        let mut reserved: BTreeSet<String> = BTreeSet::new();
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        let eligible = |g: &Gen, pool: &[(String, u32, BTreeSet<u8>)], skip: &BTreeSet<String>| {
            let rarity = self.table.rarity[g.row as usize];
            let own = family(g.row);
            let options = pool
                .iter()
                .filter(|(m, ram, r)| *ram >= g.most && r.contains(&rarity) && !skip.contains(m))
                .map(|(m, _, _)| m.clone())
                .collect::<Vec<_>>();
            // A generator always changes model when anything else will do.
            let other = options.iter().filter(|m| Some(m.as_str()) != own).cloned().collect::<Vec<_>>();
            if other.is_empty() { options } else { other }
        };
        for &(gen, orig) in &sites {
            let Some(g) = first.iter().find(|g| g.name == gen) else { continue };
            let options = eligible(g, &pool, &reserved);
            if options.is_empty() {
                continue;
            }
            let pick = options[(rng.next() % options.len() as u64) as usize].clone();
            reserved.insert(pick.clone());
            choice.insert(gen.to_string(), pick.clone());
            names.insert(orig.to_string(), pick);
        }
        for g in &first {
            if choice.contains_key(&g.name) || stage.pinned.contains(&g.name) {
                continue;
            }
            let Some(fam) = family(g.row) else { continue };
            if !usable(fam) || stage.pinned_families.contains(fam) {
                continue;
            }
            let options = eligible(g, &pool, &reserved);
            if options.is_empty() {
                continue;
            }
            let pick = options[(rng.next() % options.len() as u64) as usize].clone();
            choice.insert(g.name.clone(), pick);
        }
        for &pack in &stage.gens {
            let mut rt = Retarget::default();
            for g in generators(&Pack::parse(data(pack)?)?)? {
                let Some(model) = choice.get(&g.name) else { continue };
                let Some(row) = nearest_row(&self.table, g.row, model) else { continue };
                rt.rows.insert(g.name.clone(), row);
                if g.seq {
                    rt.blank_seq.insert(g.name);
                }
            }
            self.plans.insert(pack, rt);
        }
        if let Some(intro) = intro {
            if !names.is_empty() {
                let more = lua_names::rename_in_pack(&dst, &[intro.script.to_string()], &names)?;
                dst = materialize(&dst, &more)?;
            }
        }
        if env::var("DW4_RANDOM_DUMP").is_ok_and(|v| v.split(',').any(|e| e == entry.to_string())) {
            for g in &first {
                if let Some(m) = choice.get(&g.name) {
                    self.log.push(format!(
                        "  {} {} {} -> {m} at ({:.0}, {:.0}, {:.0})",
                        g.name,
                        g.start,
                        family(g.row).unwrap_or("?"),
                        g.pos[0],
                        g.pos[1],
                        g.pos[2]
                    ));
                }
            }
        }
        let tally = pool
            .iter()
            .map(|(m, _, _)| {
                format!("{m}x{}", choice.values().filter(|v| *v == m).count())
            })
            .collect::<Vec<_>>()
            .join(" ");
        self.log.push(format!(
            "stage {entry}: {} generators over {} models ({tally}), intro {:?}",
            choice.len(),
            pool.len(),
            names
        ));
        Ok(dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> EnemyTable {
        let rows = 650;
        let mut t = EnemyTable {
            hp: vec![0; rows],
            stat: vec![[0; 12]; rows],
            crit: vec![0; rows],
            para: vec![0; rows],
            rarity: vec![0; rows],
        };
        // goburi tier rows 0..3 and a crown row 379; mummy tier rows 114..117 and a crown row 394
        for (r, hp, atk) in [(0, 50, 16), (1, 58, 18), (2, 74, 54), (3, 77, 56), (114, 200, 66), (115, 229, 78), (116, 241, 84), (117, 268, 98)] {
            t.hp[r] = hp;
            t.stat[r][0] = atk;
        }
        t.hp[379] = 250;
        t.stat[379][0] = 78;
        t.rarity[379] = 1;
        t.hp[394] = 1000;
        t.stat[394][0] = 332;
        t.rarity[394] = 1;
        t
    }

    #[test]
    fn nearest_row_scales_by_hp_and_attack() {
        let t = table();
        assert_eq!(nearest_row(&t, 0, "e_mummy"), Some(114));
        assert_eq!(nearest_row(&t, 3, "e_mummy"), Some(114));
        assert_eq!(nearest_row(&t, 3, "e_goburi"), Some(3));
    }

    #[test]
    fn nearest_row_keeps_the_crown_rarity() {
        let t = table();
        assert_eq!(nearest_row(&t, 379, "e_mummy"), Some(394));
        assert_eq!(nearest_row(&t, 379, "e_goburi"), Some(379));
        assert_eq!(nearest_row(&t, 379, "e_ogre"), None);
    }

    #[test]
    fn the_stream_is_deterministic_and_shuffles() {
        let mut a: Vec<u32> = (0..20).collect();
        let mut b = a.clone();
        Rng::new(7, 3).shuffle(&mut a);
        Rng::new(7, 3).shuffle(&mut b);
        assert_eq!(a, b);
        let mut c: Vec<u32> = (0..20).collect();
        Rng::new(8, 3).shuffle(&mut c);
        assert_ne!(a, c);
        a.sort();
        assert_eq!(a, (0..20).collect::<Vec<_>>());
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_classes -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_classes() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let ent = |i: usize| (u32at(afs, 8 + 8 * i).unwrap() as usize, u32at(afs, 12 + 8 * i).unwrap() as usize);
        for e in [179, 233, 288, 621, 665, 723] {
            let (o, l) = ent(e);
            let b = &afs[o..o + l];
            let (classes, _, end) = class_table(b).unwrap();
            let names = u32at(b, end).unwrap() as usize;
            println!(
                "entry {e}: words {:?} classes {} table end {end} names {names} -> dirs at {} ; w2 vs end: {}",
                (0..8).map(|i| u32at(b, 4 * i).unwrap()).collect::<Vec<_>>(),
                classes.len(),
                end + 8 + names,
                (end + 8 + names) as i64 - u32at(b, 8).unwrap() as i64
            );
        }
        {
            let (o, l) = ent(621);
            let b = &afs[o..o + l];
            let w2 = u32at(b, 8).unwrap() as usize;
            let pk = Pack::parse(b).unwrap();
            // classes whose table entry starts at or after word 2
            let mut at = b.windows(10).position(|w| w == b"\0igObject\0").unwrap() + 1;
            let mut tail = vec![];
            for (i, c) in pk.classes.iter().enumerate() {
                if at >= w2 {
                    tail.push((i, c.name.clone()));
                }
                let pad = (at + c.name.len() + 2) & !1;
                at = pad + 6 * c.fields.len();
            }
            println!("w2 {w2}: classes at/after it: {tail:?}; total {}", pk.classes.len());
            let used: std::collections::BTreeSet<usize> = pk.objects.iter().map(|r| r.class).collect();
            println!("object classes in tail: {:?}", tail.iter().filter(|(i, _)| used.contains(i)).collect::<Vec<_>>());
            let dirc: std::collections::BTreeSet<usize> = pk.dirs.iter().map(|r| r.class).collect();
            println!("dir classes {dirc:?}");
        }
        let (o, l) = ent(665);
        let dst = &afs[o..o + l];
        let (so, sl) = ent(210);
        let src = Pack::parse(&afs[so..so + sl]).unwrap();
        let (classes, _, end) = class_table(dst).unwrap();
        println!("665: {} classes, table ends at {end}, next bytes {:?}", classes.len(), &dst[end..end + 16]);
        let want = ["beModelCtrlAIMAP"];
        let defs = class_defs(src.b).unwrap();
        let add: Vec<ClassDef> = want
            .iter()
            .filter(|w| !classes.iter().any(|c| &c.name == *w))
            .map(|w| defs.iter().find(|c| &c.class.name == w).unwrap().clone())
            .collect();
        let out = add_classes(dst, &add).unwrap();
        let p = Pack::parse(&out).unwrap();
        println!("after: {} classes; ids {} vs {}", p.classes.len(), p.ids.len(), Pack::parse(dst).unwrap().ids.len());
        assert!(p.classes.iter().any(|c| c.name == "beModelCtrlAIMAP"));
    }

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_census -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_census() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let n = u32at(afs, 4).unwrap() as usize;
        let entries: Vec<(usize, usize)> = (0..n)
            .map(|i| (u32at(afs, 8 + 8 * i).unwrap() as usize, u32at(afs, 12 + 8 * i).unwrap() as usize))
            .collect();
        let layout = crate::release::detect(&iso).unwrap().layout_in(&iso).unwrap();
        let table = EnemyTable::read(&iso, &layout).unwrap();
        let r = Randomiser::scan(afs, &entries, table, Options { seed: 1, donors: 3, warp: None, only: None }).unwrap();
        println!("donors: {:?}", r.donors.iter().map(|(m, d)| (m.clone(), d.source, d.ram, d.profile.scene.clone())).collect::<Vec<_>>());
        for (&e, st) in &r.stages {
            let (o, l) = entries[e];
            let dp = Pack::parse(&afs[o..o + l]).unwrap();
            let em01 = graft::has_group(&dp, b"EM01");
            let mut ok = vec![];
            let mut no = vec![];
            for ((m, _), d) in &r.donors {
                if st.residents.contains_key(*m) { continue; }
                match d.profile.misfit(&dp) {
                    None => ok.push(*m),
                    Some(w) => no.push(format!("{m}:{}", w.chars().take(28).collect::<String>())),
                }
            }
            println!("stage {e}: EM01={em01} residents={:?} fit={} [{}] nofit={}", st.residents.keys().collect::<Vec<_>>(), ok.len(), ok.join(","), if ok.is_empty() { no.join(" ") } else { no.len().to_string() });
        }
    }

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_fields -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_fields() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let n = u32at(afs, 4).unwrap() as usize;
        let ent = |i: usize| (u32at(afs, 8 + 8 * i).unwrap() as usize, u32at(afs, 12 + 8 * i).unwrap() as usize);
        let (o, l) = ent(622);
        let p = Pack::parse(&afs[o..o + l]).unwrap();
        let _ = n;
        for cls in ["beGeneraterData", "beGeneraterPlayerData", "beGeneraterPlayerDataList"] {
            let c = p.classes.iter().find(|c| c.name == cls).unwrap();
            println!("{cls}: {:?}", c.fields);
        }
        for (id, index) in p.ids.iter().enumerate() {
            let Some(index) = index else { continue };
            let r = &p.objects[*index];
            if p.classes[r.class].name == "beGeneraterData" {
                println!("gen id {id}: {:?}", r.fields);
                let seq = r.field(8).unwrap();
                println!("{:?}", &p.b[seq.clone()]);
                break;
            }
        }
    }
}
