//! Experimental enemy randomiser. For every stage it copies a few extra enemy models into the
//! stage's model pack, then points each regular generator at one of the models the stage now
//! carries. The new enemy row is the one in the chosen model's family whose HP and attack are
//! nearest the original's, so a stage keeps its difficulty. Bosses, generators that stage
//! scripts wait on by name, and kill-count stages are left as shipped; cutscenes that wait on
//! an intro enemy by name get their name constants renamed to the model now spawned.
//! Authored spawn sequences keep their original enemies; imported models carry their audio.
use super::graft::{self, Retarget, INTRO_STAGES};
use super::lua_names;
use super::*;
use crate::{rowgroup::ROW_GROUPS, table::EnemyTable};
use std::{collections::BTreeSet, env};

/// Bumped whenever a change alters what a seed produces, so a seed is only comparable
/// between images with the same rules version.
pub const RULES_VERSION: u32 = 7;
/// New models tried per stage by the shipped patch.
pub const DONORS: usize = 3;

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
pub(super) const REGULAR: &[&str] = &[
    "e_goburi",
    "e_nume",
    "e_scum",
    "e_ogre",
    "e_mecha1",
    "e_cockatri",
    "e_mummy",
    "e_mecha2",
    "e_tyrano",
    "e_minotaru",
    "e_otama",
    "e_geko",
    "e_maridevi",
    "e_mecha3",
    "e_infermo",
    "e_megadra",
    "e_rare",
    "e_impdra",
    "e_mecha4",
    "e_mecha5",
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
    (
        556,
        &[
            "GENE-22", "GENE-16", "GENE-17", "GENE-18", "GENE-19", "GENE-20", "GENE-21",
        ],
    ),
    (579, &["GENE-22", "GENE-20", "GENE-21"]),
    (587, &["GENE-22", "GENE-23"]),
    (595, &["GENE-22", "GENE-23", "GENE-24"]),
    (621, &["GENE-38"]),
    (635, &["GENE-011", "GENE-013", "GENE-016"]),
    (
        649,
        &["GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"],
    ),
    (
        686,
        &[
            "GENE-131", "GENE-139", "GENE-165", "GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04",
            "GENE-s05",
        ],
    ),
];

/// Kill-count quest stages: their clear counts kills in the hit scripts of these models and the
/// enemy supply is fixed, so enemies only trade places among them and nothing is copied in.
const RESTRICTED: &[(usize, &[&str])] = &[
    // Timed quest: each native hit script updates the per-player kill counter.
    (449, &["e_mummy", "e_mecha1", "e_minotaru", "e_pharaoh"]),
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
    let (hp, atk) = (
        table.hp[row].max(1) as f64,
        table.stat[row][0].max(1) as f64,
    );
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
                (table.hp[r].max(1) as f64 / hp).ln().abs()
                    + (table.stat[r][0].max(1) as f64 / atk).ln().abs()
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

/// Each tier lists (original family, selected family, maximum live count).
/// Tiers are alternatives, while generators within a tier share the same pool.
fn shared_capacity(
    tiers: impl IntoIterator<Item = Vec<(&'static str, &'static str, u32)>>,
    residents: &BTreeMap<String, u32>,
) -> BTreeMap<String, usize> {
    let mut demand: BTreeMap<String, usize> = BTreeMap::new();
    for tier in tiers {
        let mut totals: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for (from, to, count) in tier {
            let (all, incoming) = totals.entry(to).or_default();
            *all += count as usize;
            if from != to {
                *incoming += count as usize;
            }
        }
        for (model, (all, incoming)) in totals {
            // Existing native pools also serve scripts (e.g. summons). Keep
            // their authored headroom when adding generators of another family.
            let reserve = residents.get(model).map_or(0, |n| *n as usize + incoming);
            let need = demand.entry(model.to_string()).or_default();
            *need = (*need).max(all.max(reserve));
        }
    }
    demand
}

/// Stage scripts can freeze, remove, damage or wait on enemies by model name.
/// Keep those models in their scripted encounters. Intro scripts with explicit
/// retargeting support are handled separately, and model-owned callbacks travel
/// with their model rather than constraining unrelated generators.
fn scripted_models(p: &Pack, entry: usize) -> Result<BTreeSet<&'static str>> {
    let intro = INTRO_STAGES.iter().find(|s| s.model_entry == entry);
    let mut protected = BTreeSet::new();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaData").is_err() {
            continue;
        }
        let Some(name) = graft::name_of(p, p.rec(id)?).and_then(|s| std::str::from_utf8(s).ok())
        else {
            continue;
        };
        if intro.is_some_and(|i| i.script == name)
            || graft::MODEL_SCRIPTS
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            continue;
        }
        let obj = p.val(id, 3)?;
        let size = p.val(obj, 2)? as usize;
        let range = p
            .memory
            .get(p.val(obj, 3)? as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing stage script"))?;
        need(size <= range.len(), "stage script exceeds memory")?;
        let constants = lua_names::constants(&p.b[range.start..range.start + size])?;
        if !constants
            .iter()
            .any(|c| c.text == b"MODELCTRL" || c.text == b"ENEMY")
        {
            continue;
        }
        for &model in REGULAR.iter().chain(REPLACEABLE) {
            if constants.iter().any(|c| c.text == model.as_bytes()) {
                // Mangrove's boss-start wait and boss-end cleanup address the
                // native add groups (GENE-131/139 and GENE-s01..s05), all pinned
                // above. Their models and original instance pools stay loaded.
                // Protecting those families across the whole map also locked
                // every unrelated Otamamon/Gekomon patrol out of randomisation.
                if entry == 686
                    && matches!(
                        (name, model),
                        ("mate00.lua", "e_otama") | ("tim_g_orbz03.lua", "e_geko" | "e_otama")
                    )
                {
                    continue;
                }
                protected.insert(model);
            }
        }
    }
    Ok(protected)
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
    audio_sources: BTreeMap<&'static str, usize>,
    stages: BTreeMap<usize, Stage>,
    plans: BTreeMap<usize, Retarget>,
    pub(super) sounds: BTreeMap<usize, Vec<u8>>,
    pub log: Vec<String>,
    stats: Stats,
}

/// What the randomiser did, for the image signature.
#[derive(Default)]
struct Stats {
    stages: usize,
    generators: usize,
    kept: usize,
    copied: usize,
    approximate: usize,
    unattached: usize,
    /// Every decision, in a fixed order: hashed so two images can be compared for equal plans.
    trace: Vec<String>,
}

fn rarities(table: &EnemyTable, model: &str) -> BTreeSet<u8> {
    ROW_GROUPS
        .iter()
        .find(|g| g.model == model)
        .map(|g| g.rows.iter().map(|&r| table.rarity[r]).collect())
        .unwrap_or_default()
}

impl Randomiser {
    /// The settings and results the image signature records: enough to rebuild the same
    /// image from the original disc, and a digest of every decision to confirm that it did.
    pub(super) fn summary(&self) -> serde_json::Value {
        use sha1::{Digest, Sha1};
        let mut hash = Sha1::new();
        for line in &self.stats.trace {
            hash.update(line.as_bytes());
            hash.update(b"\n");
        }
        for (pack, plan) in &self.plans {
            for (generator, row) in &plan.rows {
                hash.update(format!("row {pack} {generator} {row}\n").as_bytes());
            }
        }
        let digest = hash
            .finalize()
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        serde_json::json!({
            "rulesVersion": RULES_VERSION,
            "seed": self.opts.seed,
            "donorsPerStage": self.opts.donors,
            "stages": self.stats.stages,
            "generators": self.stats.generators,
            "keptOriginal": self.stats.kept,
            "modelsCopied": self.stats.copied,
            "approximateEffects": self.stats.approximate,
            "undrawnEffects": self.stats.unattached,
            "planDigest": digest,
        })
    }

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
            audio_sources: BTreeMap::new(),
            stages: BTreeMap::new(),
            plans: BTreeMap::new(),
            sounds: BTreeMap::new(),
            log: Vec::new(),
            stats: Stats::default(),
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
                    data(g).is_some_and(|x| {
                        x.len() < 1_500_000
                            && memchr::memmem::find(x, b"beGeneraterData\0").is_some()
                    })
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
                    && r.field(7)
                        .is_ok_and(|f| p.b[f.start + 4..f.end].starts_with(b"KIND_ENEMY\0"))
                {
                    if let Some(name) = graft::name_of(&p, r) {
                        let name = String::from_utf8_lossy(name).into_owned();
                        residents.insert(name, p.val(p.val(id as u32, 6)?, 2)?);
                    }
                }
            }
            let mut pinned: BTreeSet<String> = PINNED
                .iter()
                .filter(|(entry, _)| *entry == e)
                .flat_map(|(_, names)| names.iter().map(|n| n.to_string()))
                .collect();
            let first = generators(&Pack::parse(data(gens[0]).unwrap())?)?;
            // Authored entrances (cannon waits, train falls, jumps) belong to the original
            // model's animation and movement controller. Keep those spawns intact.
            for &pack in &gens {
                pinned.extend(
                    generators(&Pack::parse(data(pack).unwrap())?)?
                        .into_iter()
                        .filter(|g| g.seq)
                        .map(|g| g.name),
                );
            }
            // Only bosses keep their model away from the other generators: a generator a
            // script merely waits on can still share its model with the rest of the stage.
            let mut pinned_families: BTreeSet<_> = first
                .iter()
                .filter(|g| pinned.contains(&g.name))
                .filter_map(|g| family(g.row))
                .filter(|f| !replaceable(f) || BOSS_REGULAR.contains(&(e, *f)))
                .collect();
            pinned_families.extend(scripted_models(&p, e)?);
            let stage = Stage {
                gens,
                residents,
                pinned,
                pinned_families,
            };
            for &model in REGULAR {
                if !me.audio_sources.contains_key(model)
                    && stage.residents.contains_key(model)
                    && super::sound::complete(
                        data(e - 1).ok_or_else(|| bad("missing source sounds"))?,
                        model,
                    )?
                {
                    me.audio_sources.insert(model, e - 1);
                }
            }
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
        need(
            n == rt.rows.len(),
            "a planned generator is missing from its pack",
        )?;
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

    fn import_sounds<'a>(
        &self,
        model: &str,
        source: usize,
        dst: &[u8],
        data: &impl Fn(usize) -> Result<&'a [u8]>,
    ) -> Result<Vec<u8>> {
        let audio = *self
            .audio_sources
            .get(model)
            .ok_or_else(|| bad(&format!("{model}: no complete audio source")))?;
        let companions =
            graft::companion_models(&Pack::parse(data(source)?)?, None, &[model.to_string()])?;
        super::sound::graft_enemy(dst, data(source - 1)?, data(audio)?, model, &companions)
    }

    /// Copy donors, choose generators and retain the resources used by the resulting stage.
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
        if self
            .opts
            .only
            .as_ref()
            .is_some_and(|only| !only.contains(&entry))
        {
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
            .map(|s| {
                s.sites
                    .iter()
                    .copied()
                    .filter(|(g, model)| {
                        !stage.pinned.contains(*g) && !stage.pinned_families.contains(*model)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut rng = Rng::new(self.opts.seed, entry as u64);
        let first = generators(&Pack::parse(data(stage.gens[0])?)?)?;
        let allowed = RESTRICTED
            .iter()
            .find(|(e, _)| *e == entry)
            .map(|(_, m)| *m);
        let usable = |f: &str| match allowed {
            Some(a) => a.contains(&f),
            None => replaceable(f),
        };
        let movable = first.iter().any(|g| {
            !stage.pinned.contains(&g.name)
                && family(g.row).is_some_and(|f| usable(f) && !stage.pinned_families.contains(f))
        });
        if !movable {
            self.log
                .push(format!("stage {entry}: nothing to randomise"));
            return Ok(dst);
        }

        // debugging: DW4_RANDOM_ADDCLASS=beModelCtrlMOVEEX only adds the class definition
        if let Ok(want) = env::var("DW4_RANDOM_ADDCLASS") {
            let dp = Pack::parse(&dst)?;
            let defs = self
                .donors
                .values()
                .flat_map(|d| d.profile.classes.iter())
                .filter(|c| {
                    want.split(',').any(|w| w == c.class.name)
                        && !dp.classes.iter().any(|d| d.name == c.class.name)
                })
                .fold(BTreeMap::new(), |mut m, c| {
                    m.insert(c.class.name.clone(), c.clone());
                    m
                });
            let add: Vec<ClassDef> = defs.into_values().collect();
            if !add.is_empty() {
                dst = add_classes(&dst, &add)?;
            }
            self.log.push(format!(
                "stage {entry}: added classes {:?}",
                add.iter().map(|c| c.class.name.clone()).collect::<Vec<_>>()
            ));
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
            .filter(|(m, _)| {
                !stage.residents.contains_key(*m) && !stage.pinned_families.contains(m)
            })
            .filter(|_| allowed.is_none())
            .collect();
        rng.shuffle(&mut candidates);
        // debugging: DW4_RANDOM_FORCE=e_otama,e_mecha4 tries those models first
        if let Ok(force) = env::var("DW4_RANDOM_FORCE") {
            let first: Vec<_> = force.split(',').collect();
            candidates
                .sort_by_key(|(m, _)| first.iter().position(|f| f == m).unwrap_or(usize::MAX));
        }
        let original_model = dst.clone();
        let original_sound = self.sounds.get(&(entry - 1)).cloned();
        let original_stats = (
            self.stats.copied,
            self.stats.approximate,
            self.stats.unattached,
            self.stats.trace.len(),
        );
        let mut imports = Vec::new();
        let (mut added, mut tried) = (0, 0);
        let enough =
            |pool: &Vec<_>, added: usize| added >= self.opts.donors && pool.len() > sites.len();
        for (m, own_scene) in candidates {
            if enough(&pool, added) || tried >= self.opts.donors + 8 {
                break;
            }
            let donor = &self.donors[&(m, own_scene)];
            if let Some(why) = donor.profile.misfit(&Pack::parse(&dst)?) {
                self.log
                    .push(format!("stage {entry}: {m} does not fit: {why}"));
                continue;
            }
            tried += 1;
            let missing = donor.profile.missing(&Pack::parse(&dst)?);
            let base = if missing.is_empty() {
                dst.clone()
            } else {
                add_classes(&dst, &missing)?
            };
            match graft::graft(&base, data(donor.source)?, &[m.to_string()]) {
                Ok((edits, report)) => {
                    if !report.unattached.is_empty() {
                        self.log.push(format!(
                            "stage {entry}: {m} has effect nodes the stage cannot draw: {:?}",
                            report.unattached
                        ));
                    }
                    let sound_entry = entry - 1;
                    let current = self
                        .sounds
                        .get(&sound_entry)
                        .map(Vec::as_slice)
                        .unwrap_or(data(sound_entry)?);
                    let sounds = self.import_sounds(m, donor.source, current, &data)?;
                    self.sounds.insert(sound_entry, sounds);
                    self.stats.copied += 1;
                    self.stats.approximate += report.approximate.len();
                    self.stats.unattached += report.unattached.len();
                    self.stats
                        .trace
                        .push(format!("copy {entry} {m} {}", donor.source));
                    if !report.approximate.is_empty() {
                        self.log.push(format!(
                            "stage {entry}: {m} effects hung approximately: {:?}",
                            report.approximate
                        ));
                    }
                    dst = materialize(&base, &edits)?;
                    imports.push((m, own_scene));
                    pool.push((m.to_string(), donor.ram, donor.rarities.clone()));
                    added += 1;
                    let used = Pack::parse(&dst).map_or(0, |p| p.ids.len());
                    self.log.push(format!(
                        "stage {entry}: + {m} from {} (pack now {used} of 65535 objects)",
                        donor.source
                    ));
                }
                Err(err) => self
                    .log
                    .push(format!("stage {entry}: {m} not copied: {err}")),
            }
        }
        if pool.len() < 2 {
            self.log.push(format!(
                "stage {entry}: pool of {} models, left as shipped",
                pool.len()
            ));
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
            let other = options
                .iter()
                .filter(|m| Some(m.as_str()) != own)
                .cloned()
                .collect::<Vec<_>>();
            if other.is_empty() {
                options
            } else {
                other
            }
        };
        for &(gen, orig) in &sites {
            let Some(g) = first.iter().find(|g| g.name == gen) else {
                continue;
            };
            let options = eligible(g, &pool, &reserved);
            if options.is_empty() {
                continue;
            }
            let pick = options[(rng.next() % options.len() as u64) as usize].clone();
            reserved.insert(pick.clone());
            choice.insert(gen.to_string(), pick.clone());
            names.insert(orig.to_string(), pick);
        }
        let mut why: BTreeMap<String, usize> = BTreeMap::new();
        for g in &first {
            if choice.contains_key(&g.name) || stage.pinned.contains(&g.name) {
                *why.entry("pinned or site".into()).or_default() += 1;
                continue;
            }
            let Some(fam) = family(g.row) else {
                *why.entry("not an enemy row".into()).or_default() += 1;
                continue;
            };
            if !usable(fam) || stage.pinned_families.contains(fam) {
                *why.entry(format!("family {fam} unusable or pinned"))
                    .or_default() += 1;
                continue;
            }
            let options = eligible(g, &pool, &reserved);
            if options.is_empty() {
                *why.entry(format!(
                    "no donor fits (rarity {}, ram {})",
                    self.table.rarity[g.row as usize], g.most
                ))
                .or_default() += 1;
                continue;
            }
            let pick = options[(rng.next() % options.len() as u64) as usize].clone();
            choice.insert(g.name.clone(), pick);
        }
        if env::var("DW4_RANDOM_WHY").is_ok() {
            self.log.push(format!(
                "stage {entry}: skipped {why:?}; pool {:?}",
                pool.iter()
                    .map(|(m, r, s)| format!("{m}:{r}:{s:?}"))
                    .collect::<Vec<_>>()
            ));
        }
        for &pack in &stage.gens {
            let mut rt = Retarget::default();
            for g in generators(&Pack::parse(data(pack)?)?)? {
                let Some(model) = choice.get(&g.name) else {
                    continue;
                };
                let Some(row) = nearest_row(&self.table, g.row, model) else {
                    continue;
                };
                rt.rows.insert(g.name.clone(), row);
            }
            self.plans.insert(pack, rt);
        }
        // A model pool belongs to the stage, not to an individual generator.
        // Sum the largest authored party count for every user of that model,
        // including pinned/unchanged generators, and take the maximum over the
        // difficulty packs (only one difficulty is loaded at a time).
        let mut tiers = Vec::new();
        for &pack in &stage.gens {
            let mut tier = Vec::new();
            for g in generators(&Pack::parse(data(pack)?)?)? {
                let row = self.plans[&pack]
                    .rows
                    .get(&g.name)
                    .copied()
                    .unwrap_or(g.row);
                if let (Some(from), Some(to)) = (family(g.row), family(row)) {
                    tier.push((from, to, g.most));
                }
            }
            tiers.push(tier);
        }
        let demand = shared_capacity(tiers, &stage.residents);
        // Candidate imports make selection independent of how many generators
        // eventually use each family. Rebuild without unused candidates: even
        // unreachable objects are allocated by the game's IGB loader.
        if imports
            .iter()
            .any(|(model, _)| demand.get(*model).copied().unwrap_or(0) == 0)
        {
            dst = original_model;
            let mut sounds = original_sound.unwrap_or(data(entry - 1)?.to_vec());
            self.stats.copied = original_stats.0;
            self.stats.approximate = original_stats.1;
            self.stats.unattached = original_stats.2;
            self.stats.trace.truncate(original_stats.3);
            for (model, scene) in imports {
                if demand.get(model).copied().unwrap_or(0) == 0 {
                    self.log.push(format!("stage {entry}: omit unused {model}"));
                    pool.retain(|(m, _, _)| m != model);
                    continue;
                }
                let donor = &self.donors[&(model, scene)];
                let missing = donor.profile.missing(&Pack::parse(&dst)?);
                if !missing.is_empty() {
                    dst = add_classes(&dst, &missing)?;
                }
                let (edits, report) =
                    graft::graft(&dst, data(donor.source)?, &[model.to_string()])?;
                dst = materialize(&dst, &edits)?;
                sounds = self.import_sounds(model, donor.source, &sounds, &data)?;
                self.stats.copied += 1;
                self.stats.approximate += report.approximate.len();
                self.stats.unattached += report.unattached.len();
                self.stats
                    .trace
                    .push(format!("copy {entry} {model} {}", donor.source));
            }
            self.sounds.insert(entry - 1, sounds);
        }
        let assigned: BTreeSet<_> = choice.values().cloned().collect();
        for model in &assigned {
            let needed = demand.get(model).copied().unwrap_or(0);
            let old = super::capacity::slots(&Pack::parse(&dst)?, model)?;
            if needed > old {
                dst = super::capacity::expand(&dst, model, needed)?;
                self.log.push(format!(
                    "stage {entry}: {model} shared slots {old} -> {needed}"
                ));
            }
        }
        if assigned.contains(super::height::MODEL)
            && !stage.residents.contains_key(super::height::MODEL)
        {
            let edits = super::height::imperial(&dst)?;
            dst = materialize(&dst, &edits)?;
            self.log.push(format!(
                "stage {entry}: imported Imperialdramon model and collision lowered by 13"
            ));
        }
        // More enemy instances can also exceed the base crown inventory bound.
        let p = Pack::parse(&dst)?;
        let mut enemy_slots = 0usize;
        for (id, ix) in p.ids.iter().enumerate() {
            if let Some(ix) = ix {
                let r = &p.objects[*ix];
                if p.classes[r.class].name == "beModelCtrlInfo"
                    && r.field(7)
                        .is_ok_and(|f| p.b[f.start + 4..f.end].starts_with(b"KIND_ENEMY\0"))
                {
                    enemy_slots += p.array(p.val(id as u32, 6)?)?.len();
                }
            }
        }
        if graft::find_named(&p, "beModelCtrlInfo", "i_rareicon")?.is_some() {
            dst = super::capacity::expand(
                &dst,
                "i_rareicon",
                (enemy_slots + 4).max(CAPACITY as usize),
            )?;
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
        self.stats.stages += 1;
        self.stats.generators += choice.len();
        for g in &first {
            if let Some(m) = choice.get(&g.name) {
                self.stats.kept += usize::from(family(g.row) == Some(m.as_str()));
                self.stats
                    .trace
                    .push(format!("pick {entry} {} {m}", g.name));
            }
        }
        let tally = pool
            .iter()
            .map(|(m, _, _)| format!("{m}x{}", choice.values().filter(|v| *v == m).count()))
            .collect::<Vec<_>>()
            .join(" ");
        self.log.push(format!(
            "stage {entry}: {} generators over {} models ({tally}), intro {:?}; enemy generators {} ({} names)",
            choice.len(),
            pool.len(),
            names,
            first.iter().filter(|g| family(g.row).is_some()).count(),
            first.iter().filter(|g| family(g.row).is_some()).map(|g| &g.name).collect::<BTreeSet<_>>().len()
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
        for (r, hp, atk) in [
            (0, 50, 16),
            (1, 58, 18),
            (2, 74, 54),
            (3, 77, 56),
            (114, 200, 66),
            (115, 229, 78),
            (116, 241, 84),
            (117, 268, 98),
        ] {
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
    fn shared_generators_need_the_sum_and_tiers_are_alternatives() {
        let demand = shared_capacity(
            [
                vec![
                    ("e_goburi", "e_tyrano", 4),
                    ("e_goburi", "e_tyrano", 4),
                    ("e_ogre", "e_tyrano", 1),
                ],
                vec![("e_goburi", "e_tyrano", 2), ("e_goburi", "e_tyrano", 2)],
            ],
            &BTreeMap::new(),
        );
        assert_eq!(
            demand["e_tyrano"], 9,
            "two four-enemy waves plus the nearby singleton"
        );
        let native = BTreeMap::from([("e_mummy".to_string(), 12)]);
        let demand = shared_capacity(
            [vec![("e_goburi", "e_mummy", 4), ("e_mummy", "e_mummy", 1)]],
            &native,
        );
        assert_eq!(
            demand["e_mummy"], 16,
            "keep native capacity for script summons as well as new generators"
        );
    }

    #[test]
    fn cannon_and_train_entrances_survive_randomisation() {
        let Some(path) = env::var_os("DW4_RETAIL_USA_ISO") else {
            return;
        };
        let iso = std::fs::read(path).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + i * 8).unwrap() as usize,
                    u32at(afs, 12 + i * 8).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let data = |e: usize| {
            let (o, l) = entries[e];
            &afs[o..o + l]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let mut r = Randomiser::scan(
            afs,
            &entries,
            EnemyTable::read(&iso, &layout).unwrap(),
            Options {
                seed: 7,
                donors: 0,
                warp: None,
                only: None,
            },
        )
        .unwrap();
        let train = *r
            .stages
            .iter()
            .find(|(_, s)| s.gens.contains(&659))
            .unwrap()
            .0;
        let mut protected = 0;
        let mut changed = 0;
        for e in [614, train] {
            r.stage(e, data(e).to_vec(), afs, &entries).unwrap();
            for &pack in &r.stages[&e].gens {
                let original = Pack::parse(data(pack)).unwrap();
                let out = r
                    .retarget(pack, data(pack))
                    .unwrap()
                    .unwrap_or_else(|| data(pack).to_vec());
                let out = Pack::parse(&out).unwrap();
                let after = generators(&out).unwrap();
                for g in generators(&original).unwrap() {
                    let new = after.iter().find(|n| n.name == g.name).unwrap();
                    if g.seq {
                        assert_eq!(new.row, g.row, "entrance {} in {pack}", g.name);
                        assert!(new.seq);
                        protected += 1;
                    } else if new.row != g.row {
                        changed += 1;
                    }
                }
            }
        }
        assert!(
            protected > 50,
            "cover the cannon and train entrances on every difficulty"
        );
        assert!(changed > 0, "ordinary spawns still randomise");
    }

    #[test]
    fn mangrove_seed_1278675365_changes_regular_spawns_and_keeps_scripted_groups() {
        for variable in [
            "DW4_RETAIL_USA_ISO",
            "DW4_RETAIL_EUROPE_ISO",
            "DW4_RETAIL_JAPAN_ISO",
            "DW4_RETAIL_GC_USA_ISO",
            "DW4_RETAIL_GC_JAPAN_ISO",
        ] {
            let Some(path) = env::var_os(variable) else {
                continue;
            };
            let iso = std::fs::read(path).unwrap();
            let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
                let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
                (f.offset as usize, f.size as usize)
            } else {
                let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (f.lba as usize * 2048, f.size as usize)
            };
            let afs = &iso[base..base + size];
            let entries: Vec<_> = (0..u32at(afs, 4).unwrap() as usize)
                .map(|i| {
                    (
                        u32at(afs, 8 + 8 * i).unwrap() as usize,
                        u32at(afs, 12 + 8 * i).unwrap() as usize,
                    )
                })
                .collect();
            let data = |i: usize| {
                let (o, n) = entries[i];
                &afs[o..o + n]
            };
            let layout = crate::release::detect(&iso)
                .unwrap()
                .layout_in(&iso)
                .unwrap();
            let mut r = Randomiser::scan(
                afs,
                &entries,
                EnemyTable::read(&iso, &layout).unwrap(),
                Options {
                    seed: 1278675365,
                    donors: 3,
                    warp: None,
                    only: Some(vec![686]),
                },
            )
            .unwrap();
            let stage = r.stages[&686].clone();
            let initial = materialize(data(686), &expand(data(686)).unwrap()).unwrap();
            let out = r.stage(686, initial, afs, &entries).unwrap();
            let mut changed = 0;
            let mut protected = BTreeSet::new();
            for pack in &stage.gens {
                let source = Pack::parse(data(*pack)).unwrap();
                let patched = r
                    .retarget(*pack, data(*pack))
                    .unwrap()
                    .unwrap_or_else(|| data(*pack).to_vec());
                let result = Pack::parse(&patched).unwrap();
                for g in generators(&source).unwrap() {
                    let id = source.named("beGeneraterData", &g.name).unwrap();
                    let new = result.val(id, graft::GENERATOR_ROW).unwrap();
                    if stage.pinned.contains(&g.name) || g.seq {
                        assert_eq!(g.row, new, "{variable}: scripted group {}", g.name);
                        let a = source.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                        let b = result.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                        assert_eq!(source.b[a], result.b[b]);
                        protected.insert(g.name);
                    } else if g.row != new {
                        changed += 1;
                    }
                }
            }
            assert!(
                changed > 0,
                "{variable}: Mangrove ordinary enemies all remained vanilla"
            );
            for name in [
                "GENE-131", "GENE-139", "GENE-165", "GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04",
                "GENE-s05",
            ] {
                assert!(protected.contains(name), "missing protected group {name}");
            }
            let native = Pack::parse(data(686)).unwrap();
            let result = Pack::parse(&out).unwrap();
            for model in ["e_geko", "e_otama", "e_torta", "e_tonogeko"] {
                // Tortamon can be loaded from its separate model file.
                if graft::find_named(&native, "beModelCtrlInfo", model)
                    .unwrap()
                    .is_some()
                {
                    assert!(
                        graft::find_named(&result, "beModelCtrlInfo", model)
                            .unwrap()
                            .is_some(),
                        "native scripted model {model} must remain loaded"
                    );
                }
            }
            for id in 0..native.ids.len() as u32 {
                if native.class(id, "beLuaData").is_ok() {
                    let object = native.val(id, 3).unwrap();
                    let mem = native.val(object, 3).unwrap();
                    assert_eq!(
                        native.b[native.memory[mem as usize].clone().unwrap()],
                        result.b[result.memory[mem as usize].clone().unwrap()]
                    );
                }
            }
            eprintln!("MANGROVE {variable}: {changed} ordinary rows changed; {} scripted groups and all original scripts preserved", protected.len());
        }
    }

    #[test]
    fn seed_1520_gamecube_humid_cave_omits_unused_imports() {
        let Some(path) = env::var_os("DW4_RETAIL_GC_USA_ISO") else {
            return;
        };
        let iso = std::fs::read(path).unwrap();
        let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
        let afs = &iso[f.offset as usize..(f.offset + f.size) as usize];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let data = |e: usize| {
            let (o, l) = entries[e];
            &afs[o..o + l]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let mut r = Randomiser::scan(
            afs,
            &entries,
            EnemyTable::read(&iso, &layout).unwrap(),
            Options {
                seed: 1520,
                donors: 3,
                warp: None,
                only: Some(vec![179, 187]),
            },
        )
        .unwrap();
        let original = materialize(data(187), &expand(data(187)).unwrap()).unwrap();
        let out = r.stage(187, original.clone(), afs, &entries).unwrap();
        let p = Pack::parse(&out).unwrap();
        assert!(graft::find_named(&p, "beModelCtrlInfo", "e_nume")
            .unwrap()
            .is_some());
        assert!(graft::find_named(&p, "beModelCtrlInfo", "e_mecha3")
            .unwrap()
            .is_none());
        assert_eq!(super::super::capacity::slots(&p, "e_mecha4").unwrap(), 6);
        assert_eq!(super::super::capacity::slots(&p, "e_infermo").unwrap(), 12);
        assert_eq!(r.stats.copied, 2);
        for &pack in &r.stages[&187].gens {
            let bytes = r.retarget(pack, data(pack)).unwrap().unwrap();
            let target = Pack::parse(&bytes).unwrap();
            let mut counts = BTreeMap::new();
            for g in generators(&target).unwrap() {
                if r.plans[&pack].rows.contains_key(&g.name) {
                    *counts.entry(family(g.row).unwrap()).or_insert(0) += 1;
                }
            }
            assert_eq!(counts, BTreeMap::from([("e_mecha4", 2), ("e_infermo", 1)]));
        }
        // Optional private fixtures for replaying a user-supplied savestate.
        // Keep the export directory outside the source checkout.
        if let Some(dir) = env::var_os("DW4_TEST_STAGE_EXPORT") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("187.igb"), &out).unwrap();
            std::fs::write(dir.join("186.igb"), &r.sounds[&186]).unwrap();
            let first = materialize(data(179), &expand(data(179)).unwrap()).unwrap();
            let first = r.stage(179, first, afs, &entries).unwrap();
            std::fs::write(dir.join("179.igb"), first).unwrap();
            std::fs::write(dir.join("178.igb"), &r.sounds[&178]).unwrap();
        }
        eprintln!(
            "Humid Cave B2: {} -> {} model bytes; MR04/Infermon assignments and pools retained",
            original.len(),
            out.len()
        );
    }

    #[test]
    fn seed_1520_gamecube_shared_capacity_covers_every_generator() {
        let Some(path) = env::var_os("DW4_RETAIL_GC_USA_ISO") else {
            return;
        };
        let iso = std::fs::read(path).unwrap();
        let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
        let afs = &iso[f.offset as usize..(f.offset + f.size) as usize];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect::<Vec<_>>();
        let data = |e: usize| {
            let (o, l) = entries[e];
            &afs[o..o + l]
        };
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let mut r = Randomiser::scan(
            afs,
            &entries,
            EnemyTable::read(&iso, &layout).unwrap(),
            Options {
                seed: 1520,
                donors: 3,
                warp: None,
                only: Some(vec![614]),
            },
        )
        .unwrap();
        let initial = materialize(data(614), &expand(data(614)).unwrap()).unwrap();
        let before = Pack::parse(&initial).unwrap();
        let scene_blocks: Vec<_> = (0..before.ids.len() as u32)
            .filter(|&id| before.class(id, "igNodeList").is_ok())
            .map(|id| (id, before.val(id, 4).unwrap()))
            .collect();
        let out = r.stage(614, initial, afs, &entries).unwrap();
        let p = Pack::parse(&out).unwrap();
        for (list, block) in scene_blocks {
            assert_eq!(
                p.val(list, 4).unwrap(),
                block,
                "scene list {list}: imported enemies must not orphan the original backing block"
            );
        }
        let mut checked = 0;
        for &pack in &r.stages[&614].gens {
            let retargeted = r.retarget(pack, data(pack)).unwrap().unwrap();
            let after = generators(&Pack::parse(&retargeted).unwrap()).unwrap();
            let mut demand = BTreeMap::<&str, usize>::new();
            for g in &after {
                if let Some(m) = family(g.row) {
                    if REGULAR.contains(&m) {
                        *demand.entry(m).or_default() += g.most as usize;
                    }
                }
            }
            for (model, count) in demand {
                if r.plans[&pack]
                    .rows
                    .values()
                    .any(|&row| family(row) == Some(model))
                {
                    assert!(
                        super::super::capacity::slots(&p, model).unwrap() >= count,
                        "{pack} {model}: need {count}"
                    );
                    checked += 1;
                }
            }
            // Maxima must still be the authored maxima, for all four party sizes.
            let original = Pack::parse(data(pack)).unwrap();
            let patched = Pack::parse(&retargeted).unwrap();
            for (id, ix) in original.ids.iter().enumerate() {
                if let Some(ix) = ix {
                    if original.classes[original.objects[*ix].class].name == "beGeneraterPlayerData"
                    {
                        assert_eq!(
                            original.val(id as u32, 3).unwrap(),
                            patched.val(id as u32, 3).unwrap()
                        );
                    }
                }
            }
        }
        assert!(checked >= 9);
        assert!(
            super::super::capacity::slots(&p, "e_tyrano").unwrap() > 4,
            "the imported four-slot Tyrannomon pool must grow"
        );
        let mut total = 4;
        for (id, ix) in p.ids.iter().enumerate() {
            if let Some(ix) = ix {
                let rec = &p.objects[*ix];
                if p.classes[rec.class].name == "beModelCtrlInfo"
                    && rec
                        .field(7)
                        .is_ok_and(|f| p.b[f.start + 4..f.end].starts_with(b"KIND_ENEMY\0"))
                {
                    total += p.array(p.val(id as u32, 6).unwrap()).unwrap().len();
                }
            }
        }
        assert!(super::super::capacity::slots(&p, "i_rareicon").unwrap() >= total);
        eprintln!(
            "seed 1520: Tyrannomon slots {}, crown slots {}",
            super::super::capacity::slots(&p, "e_tyrano").unwrap(),
            super::super::capacity::slots(&p, "i_rareicon").unwrap()
        );
    }

    #[test]
    fn every_pool_model_has_an_effect_number() {
        for m in REGULAR {
            assert!(
                graft::EFFECT_NUMBERS.iter().any(|(n, _)| n == m),
                "{m} has no effect number"
            );
        }
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

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_scripts -- --ignored --nocapture`
    /// Every `xxx_<model>.lua` script of the stage packs, by prefix and model.
    #[test]
    #[ignore]
    fn probe_scripts() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let n = u32at(afs, 4).unwrap() as usize;
        let mut seen: BTreeMap<(String, String), BTreeSet<usize>> = BTreeMap::new();
        for e in 0..n {
            let (o, l) = (
                u32at(afs, 8 + 8 * e).unwrap() as usize,
                u32at(afs, 12 + 8 * e).unwrap() as usize,
            );
            if !(1_500_000..=12_000_000).contains(&l)
                || memchr::memmem::find(&afs[o..o + l], b"KIND_ENEMY\0").is_none()
            {
                continue;
            }
            let Ok(p) = Pack::parse(&afs[o..o + l]) else {
                continue;
            };
            for id in 0..p.ids.len() as u32 {
                let Some(i) = p.ids[id as usize] else {
                    continue;
                };
                let r = &p.objects[i];
                if p.classes[r.class].name != "beLuaData" {
                    continue;
                }
                let Some(name) = graft::name_of(&p, r) else {
                    continue;
                };
                let name = String::from_utf8_lossy(name).into_owned();
                if let Some((prefix, rest)) = name.split_once('_') {
                    if let Some(model) = rest.strip_suffix(".lua") {
                        if (model.starts_with("e_") || model.starts_with("f_")) && prefix.len() == 3
                        {
                            seen.entry((prefix.to_string(), model.to_string()))
                                .or_default()
                                .insert(e);
                        }
                    }
                }
            }
        }
        let mut by_prefix: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for ((prefix, model), entries) in &seen {
            by_prefix
                .entry(prefix.clone())
                .or_default()
                .push(format!("{model}x{}", entries.len()));
        }
        for (prefix, models) in by_prefix {
            println!("{prefix}: {}", models.join(" "));
        }
    }

    /// `DW4_ENTRY=716 DW4_MATCH=026 ... cargo test probe_bullets -- --ignored --nocapture`: everything
    /// in one stage pack whose name has the text, and the string constants of the scripts of `DW4_MODEL`.
    #[test]
    #[ignore]
    fn probe_bullets() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let e: usize = std::env::var("DW4_ENTRY").unwrap().parse().unwrap();
        let (o, l) = (
            u32at(afs, 8 + 8 * e).unwrap() as usize,
            u32at(afs, 12 + 8 * e).unwrap() as usize,
        );
        let p = Pack::parse(&afs[o..o + l]).unwrap();
        let needle = std::env::var("DW4_MATCH").unwrap();
        for id in 0..p.ids.len() as u32 {
            let Some(i) = p.ids[id as usize] else {
                continue;
            };
            let r = &p.objects[i];
            let Some(name) = graft::name_of(&p, r).map(|x| String::from_utf8_lossy(x).into_owned())
            else {
                continue;
            };
            if name.contains(&needle) {
                println!("OBJ {id} {} {name}", p.classes[r.class].name);
            }
        }
        for (class, name) in [
            ("beAction2Info", "x_026eblst01a"),
            ("igSwitch", "y_026emagc01a"),
        ] {
            if let Ok(Some(id)) = graft::find_named(&p, class, name) {
                let switch = if class == "igSwitch" {
                    id
                } else {
                    p.val(id, 7).unwrap()
                };
                let mut level = vec![switch];
                for depth in 0..5 {
                    let mut next = Vec::new();
                    for h in 0..p.ids.len() as u32 {
                        let Some(hi) = p.ids[h as usize] else {
                            continue;
                        };
                        let hr = &p.objects[hi];
                        for (n, memory) in graft::ref_slots(&p, hr) {
                            if memory {
                                continue;
                            }
                            let v = hr.val(p.b, n).unwrap();
                            let hit = level.contains(&v)
                                || (p.class(v, "igNodeList").is_ok()
                                    && p.array(v).unwrap().iter().any(|c| level.contains(c)));
                            if hit && !next.contains(&h) {
                                println!(
                                    "HOLDER {name} d{depth} <- {h} {} {:?} slot {n}",
                                    p.classes[hr.class].name,
                                    graft::name_of(&p, hr)
                                        .map(|x| String::from_utf8_lossy(x).into_owned())
                                );
                                next.push(h);
                            }
                        }
                    }
                    if next.len() > 12 {
                        break;
                    }
                    level = next;
                }
                match graft::parent_group(&p, switch) {
                    Ok((group, _)) => println!(
                        "PARENT {name} -> {:?}",
                        graft::name_of(&p, p.rec(group).unwrap())
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                    ),
                    Err(e) => println!("PARENT {name} -> none ({e:?})"),
                }
            }
        }
        for h in 0..p.ids.len() as u32 {
            let Some(hi) = p.ids[h as usize] else {
                continue;
            };
            let hr = &p.objects[hi];
            let class = p.classes[hr.class].name.as_str();
            let name = graft::name_of(&p, hr)
                .map(|x| String::from_utf8_lossy(x).into_owned())
                .unwrap_or_default();
            if (class == "igGroup" || class == "igAttrSet") && (name == "EF01" || name == "EF02") {
                for (n, memory) in graft::ref_slots(&p, hr) {
                    if memory {
                        continue;
                    }
                    let v = hr.val(p.b, n).unwrap();
                    if v != u32::MAX && p.class(v, "igNodeList").is_ok() {
                        let kids: Vec<String> = p
                            .array(v)
                            .unwrap()
                            .iter()
                            .map(|&c| {
                                let r = p.rec(c).unwrap();
                                format!(
                                    "{c}:{}:{}",
                                    p.classes[r.class].name,
                                    graft::name_of(&p, r)
                                        .map(|x| String::from_utf8_lossy(x).into_owned())
                                        .unwrap_or_default()
                                )
                            })
                            .collect();
                        println!("GROUP {h} {class} {name} slot {n} kids {kids:?}");
                    }
                }
            }
        }
        {
            fn tree(p: &Pack, id: u32, depth: usize) {
                let r = p.rec(id).unwrap();
                let name = graft::name_of(p, r)
                    .map(|x| String::from_utf8_lossy(x).into_owned())
                    .unwrap_or_default();
                println!(
                    "TREE {}{id} {} {name}",
                    "  ".repeat(depth),
                    p.classes[r.class].name
                );
                if depth > 5 {
                    return;
                }
                for (n, memory) in graft::ref_slots(p, r) {
                    if memory {
                        continue;
                    }
                    let v = r.val(p.b, n).unwrap();
                    if v != u32::MAX && p.class(v, "igNodeList").is_ok() {
                        for c in p.array(v).unwrap() {
                            tree(p, c, depth + 1)
                        }
                    }
                }
            }
            if let Ok(t) = std::env::var("DW4_TREE") {
                for id in t.split(',') {
                    tree(&p, id.parse().unwrap(), 0)
                }
            }
        }
        if let Ok(model) = std::env::var("DW4_MODEL") {
            for prefix in graft::MODEL_SCRIPTS {
                let name = format!("{prefix}{model}.lua");
                let Ok(Some(id)) = graft::find_named(&p, "beLuaData", &name) else {
                    continue;
                };
                let object = p.val(id, 3).unwrap();
                let (size, mem) = (
                    p.val(object, 2).unwrap() as usize,
                    p.val(object, 3).unwrap(),
                );
                let range = p.memory[mem as usize].as_ref().unwrap();
                let raw = &p.b[range.start..range.start + size];
                let consts = super::lua_names::constants(raw).unwrap();
                let strings: Vec<String> = consts
                    .iter()
                    .map(|c| String::from_utf8_lossy(&c.text).into_owned())
                    .collect();
                println!("LUA {name} {strings:?}");
            }
        }
    }

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_audit -- --ignored --nocapture`: for every enemy model in
    /// every stage, the named objects its copied records and scripts mention by name but the copy leaves behind.
    #[test]
    #[ignore]
    fn probe_audit() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let n = u32at(afs, 4).unwrap() as usize;
        // model -> (class, name) -> stages missing it
        let mut missing: BTreeMap<String, BTreeMap<(String, String), BTreeSet<usize>>> =
            BTreeMap::new();
        for e in 0..n {
            let (o, l) = (
                u32at(afs, 8 + 8 * e).unwrap() as usize,
                u32at(afs, 12 + 8 * e).unwrap() as usize,
            );
            if !(1_500_000..=12_000_000).contains(&l)
                || memchr::memmem::find(&afs[o..o + l], b"KIND_ENEMY\0").is_none()
            {
                continue;
            }
            let Ok(sp) = Pack::parse(&afs[o..o + l]) else {
                continue;
            };
            let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
            let mut named: BTreeMap<String, Vec<u32>> = BTreeMap::new();
            for id in 0..sp.ids.len() as u32 {
                let Some(i) = sp.ids[id as usize] else {
                    continue;
                };
                if let Some(name) = graft::name_of(&sp, &sp.objects[i])
                    .map(|x| String::from_utf8_lossy(x).into_owned())
                {
                    if name.len() >= 4 {
                        named.entry(name).or_default().push(id);
                    }
                }
            }
            let models: Vec<String> = sp
                .array(root)
                .unwrap()
                .into_iter()
                .filter_map(|id| {
                    let r = sp.rec(id).ok()?;
                    (sp.classes[r.class].name == "beModelCtrlInfo")
                        .then(|| {
                            graft::name_of(&sp, r).map(|x| String::from_utf8_lossy(x).into_owned())
                        })
                        .flatten()
                })
                .filter(|m| m.starts_with("e_"))
                .collect();
            for m in models {
                let Ok((mut roots, _)) = graft::model_roots(&sp, &m) else {
                    continue;
                };
                if std::env::var("DW4_AUDIT_BARE").is_ok() {
                    roots.clear();
                    for class in [
                        "beModelCtrlInfo",
                        "beAction2Info",
                        "igAnimationDatabase",
                        "beShadow01Info",
                        "beHitLandDelivInfo",
                    ] {
                        roots.extend(graft::find_named(&sp, class, &m).ok().flatten());
                    }
                    for prefix in graft::MODEL_SCRIPTS {
                        roots.extend(
                            graft::find_named(&sp, "beLuaData", &format!("{prefix}{m}.lua"))
                                .ok()
                                .flatten(),
                        );
                    }
                }
                let Ok((ids, _)) = graft::closure(&sp, &roots) else {
                    continue;
                };
                let mut words: BTreeSet<String> = BTreeSet::new();
                for &id in &ids {
                    if let Some(range) = sp.memory.get(id as usize).and_then(|r| r.clone()) {
                        let b = &sp.b[range];
                        let mut run = Vec::new();
                        for &c in b.iter().chain(std::iter::once(&0u8)) {
                            if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' {
                                run.push(c);
                            } else {
                                if run.len() >= 4 {
                                    words.insert(String::from_utf8_lossy(&run).into_owned());
                                }
                                run.clear();
                            }
                        }
                    }
                }
                if let Some((_, number)) = graft::EFFECT_NUMBERS.iter().find(|(name, _)| *name == m)
                {
                    let mut attached = BTreeSet::new();
                    if let Ok(units) = graft::scene_units(&sp, &m) {
                        attached.extend(units);
                    }
                    for (name, hits) in &named {
                        let own = name.contains(&format!("_{number}e"))
                            || name.contains(&format!("{number}e{}", &m[2..]));
                        if own {
                            for &h in hits {
                                if !ids.contains(&h)
                                    && !name.ends_with("Shape1")
                                    && !name.contains("_0Shape")
                                {
                                    let class = sp.classes[sp.rec(h).unwrap().class].name.clone();
                                    missing
                                        .entry(m.clone())
                                        .or_default()
                                        .entry((class, format!("NUM {name}")))
                                        .or_default()
                                        .insert(e);
                                }
                            }
                        }
                    }
                    let _ = attached;
                }
                for w in words {
                    let key = w.strip_suffix(".lua").unwrap_or(&w).to_string();
                    for cand in [&w, &key] {
                        if let Some(hits) = named.get(cand) {
                            let outside: Vec<u32> =
                                hits.iter().copied().filter(|h| !ids.contains(h)).collect();
                            if !outside.is_empty() && hits.len() <= 8 {
                                let class =
                                    sp.classes[sp.rec(outside[0]).unwrap().class].name.clone();
                                missing
                                    .entry(m.clone())
                                    .or_default()
                                    .entry((class, cand.clone()))
                                    .or_default()
                                    .insert(e);
                            }
                        }
                    }
                }
            }
        }
        for (m, items) in missing {
            for ((class, name), stages) in items {
                println!("AUDIT {m} {class} {name} stages={}", stages.len());
            }
        }
    }

    /// `DW4_PROBE_ISO=<iso|gcm|ciso> cargo test probe_ids -- --ignored --nocapture`: object-ID budget of stage 417
    /// and the donors that went into it.
    #[test]
    #[ignore]
    fn probe_ids() {
        use crate::container::{detect, unpack, Format};
        let path = std::path::PathBuf::from(std::env::var("DW4_PROBE_ISO").unwrap());
        let tmp = tempfile::tempdir().unwrap();
        let raw = match detect(&path).unwrap() {
            Format::Raw => path.clone(),
            format => {
                let out = tmp.path().join("disc.iso");
                unpack(format, &path, &out, &mut |_, _| {}).unwrap();
                out
            }
        };
        let iso = std::fs::read(&raw).unwrap();
        let (base, size) = match crate::iso9660::find_file(&iso, "NDMW.AFS") {
            Ok(f) => (f.lba as usize * 2048, f.size as usize),
            Err(_) => {
                let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
                (f.offset as usize, f.size as usize)
            }
        };
        let afs = &iso[base..base + size];
        let entry = |e: usize| {
            (
                u32at(afs, 8 + 8 * e).unwrap() as usize,
                u32at(afs, 12 + 8 * e).unwrap() as usize,
            )
        };
        let ids = |e: usize| {
            let (o, l) = entry(e);
            Pack::parse(&afs[o..o + l])
                .map(|p| (p.ids.len(), l))
                .unwrap()
        };
        println!(
            "IDS destination 417: ids {} bytes {}",
            ids(417).0,
            ids(417).1
        );
        {
            let (o, l) = entry(417);
            let sp = Pack::parse(&afs[o..o + l]).unwrap();
            let mut blocks = 0;
            for id in 0..sp.ids.len() {
                match sp.ids[id] {
                    Some(i) => {
                        let r = &sp.objects[i];
                        let class = sp.classes[r.class].name.as_str();
                        println!("CLS {class}");
                        if let Some(n) = graft::name_of(&sp, r) {
                            println!("NAME {class} {}", String::from_utf8_lossy(n));
                        }
                    }
                    None => blocks += 1,
                }
            }
            println!("CLS <memory blocks> x{blocks}");
        }
        let mut total = ids(417).0;
        for (e, m) in [(614usize, "e_goburi"), (210, "e_mecha1"), (187, "e_scum")] {
            let (o, l) = entry(e);
            let sp = Pack::parse(&afs[o..o + l]).unwrap();
            let (mut roots, _) = graft::model_roots(&sp, m).unwrap();
            roots.extend(graft::scene_units(&sp, m).unwrap());
            let (closure, _) = graft::closure(&sp, &roots).unwrap();
            total += closure.len();
            println!("IDS donor {m} from {e}: pack ids {} closure {} running total {total} (limit 65535)", sp.ids.len(), closure.len());
        }
    }

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_numbers -- --ignored --nocapture`: per stage, residents and effect numbers.
    #[test]
    #[ignore]
    fn probe_numbers() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let n = u32at(afs, 4).unwrap() as usize;
        for e in 0..n {
            let (o, l) = (
                u32at(afs, 8 + 8 * e).unwrap() as usize,
                u32at(afs, 12 + 8 * e).unwrap() as usize,
            );
            if !(1_500_000..=12_000_000).contains(&l)
                || memchr::memmem::find(&afs[o..o + l], b"KIND_ENEMY\0").is_none()
            {
                continue;
            }
            let Ok(sp) = Pack::parse(&afs[o..o + l]) else {
                continue;
            };
            let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
            let (mut res, mut nums) = (BTreeSet::new(), BTreeSet::new());
            for id in sp.array(root).unwrap() {
                let r = sp.rec(id).unwrap();
                let class = sp.classes[r.class].name.as_str();
                let Some(name) =
                    graft::name_of(&sp, r).map(|x| String::from_utf8_lossy(x).into_owned())
                else {
                    continue;
                };
                if class == "beModelCtrlInfo" && (name.starts_with("e_") || name.starts_with("f_"))
                {
                    res.insert(name.clone());
                }
                if (name.starts_with("x_") || name.starts_with("y_"))
                    && name.len() > 5
                    && name.as_bytes()[5] == b'e'
                    && name[2..5].bytes().all(|c| c.is_ascii_digit())
                {
                    nums.insert(name[2..5].to_string());
                }
            }
            println!("NUM {e} {res:?} {nums:?}");
        }
    }

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_effects -- --ignored --nocapture`: effect infos stage A has that stage B lacks.
    #[test]
    #[ignore]
    fn probe_effects() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let names = |entry: usize| {
            let (o, l) = (
                u32at(afs, 8 + 8 * entry).unwrap() as usize,
                u32at(afs, 12 + 8 * entry).unwrap() as usize,
            );
            let sp = Pack::parse(&afs[o..o + l]).unwrap();
            let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
            let mut out = BTreeSet::new();
            for e in sp.array(root).unwrap() {
                let r = sp.rec(e).unwrap();
                let class = sp.classes[r.class].name.clone();
                let named = graft::name_of(&sp, r)
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .unwrap_or_default();
                if std::env::var("DW4_ALL").is_ok() && named.contains("026") {
                    out.insert(format!("ANY {class}:{named}"));
                }
                if ["beParticleCtrl2Info", "ParticleInfo", "beActionStarterInfo"]
                    .contains(&class.as_str())
                {
                    out.insert(format!(
                        "{class}:{}",
                        graft::name_of(&sp, r)
                            .map(|n| String::from_utf8_lossy(n).into_owned())
                            .unwrap_or_default()
                    ));
                }
            }
            out
        };
        let a: usize = std::env::var("DW4_A").unwrap().parse().unwrap();
        let b: usize = std::env::var("DW4_B").unwrap().parse().unwrap();
        let (sa, sb) = (names(a), names(b));
        println!("only in {a}: {:?}", sa.difference(&sb).collect::<Vec<_>>());
        println!("only in {b}: {:?}", sb.difference(&sa).collect::<Vec<_>>());
    }

    /// `DW4_RETAIL_USA_ISO=... DW4_PROBE_MODEL=e_infermo DW4_PROBE_ENTRY=387 cargo test probe_names -- --ignored --nocapture`
    /// Which infos of the source stage does the model refer to by name, without owning them?
    #[test]
    #[ignore]
    fn probe_names() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let model = std::env::var("DW4_PROBE_MODEL").unwrap();
        let entry: usize = std::env::var("DW4_PROBE_ENTRY").unwrap().parse().unwrap();
        let (o, l) = (
            u32at(afs, 8 + 8 * entry).unwrap() as usize,
            u32at(afs, 12 + 8 * entry).unwrap() as usize,
        );
        let sp = Pack::parse(&afs[o..o + l]).unwrap();
        let mut roots = vec![];
        for class in [
            "beModelCtrlInfo",
            "beAction2Info",
            "igAnimationDatabase",
            "beShadow01Info",
            "beHitLandDelivInfo",
        ] {
            roots.push(graft::find_named(&sp, class, &model).unwrap().unwrap());
        }
        for prefix in ["hit_", "mag_"] {
            if let Some(id) =
                graft::find_named(&sp, "beLuaData", &format!("{prefix}{model}.lua")).unwrap()
            {
                roots.push(id);
            }
        }
        let (ids, _) = graft::closure(&sp, &roots).unwrap();
        // every string the closure carries: string fields and Lua source constants
        let mut words: BTreeSet<String> = BTreeSet::new();
        for &id in &ids {
            if let Some(i) = sp.ids[id as usize] {
                let r = &sp.objects[i];
                for (n, range) in &r.fields {
                    let c = &sp.classes[r.class];
                    if c.fields.iter().any(|&(t, nn, _)| nn == *n && t == 4) && range.len() > 4 {
                        let t = String::from_utf8_lossy(&sp.b[range.start + 4..range.end]);
                        words.insert(t.trim_end_matches('\0').to_string());
                    }
                }
            } else if let Some(r) = sp.memory[id as usize].clone() {
                for w in sp.b[r].split(|c| !(c.is_ascii_alphanumeric() || *c == b'_')) {
                    if w.len() >= 4 {
                        words.insert(String::from_utf8_lossy(w).into_owned());
                    }
                }
            }
        }
        println!("{} ids, {} distinct strings", ids.len(), words.len());
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for &id in &ids {
            if let Some(i) = sp.ids[id as usize] {
                *by.entry(sp.classes[sp.objects[i].class].name.clone())
                    .or_default() += 1;
            }
        }
        println!("classes: {by:?}");
        let mut scripts = vec![];
        for id in 0..sp.ids.len() as u32 {
            if let Some(i) = sp.ids[id as usize] {
                let r = &sp.objects[i];
                if sp.classes[r.class].name == "beLuaData" {
                    if let Some(n) = graft::name_of(&sp, r) {
                        let n = String::from_utf8_lossy(n).into_owned();
                        if n.contains(model.trim_start_matches("e_")) {
                            scripts.push(n);
                        }
                    }
                }
            }
        }
        println!("scripts naming the model: {scripts:?}");
        for name in &scripts {
            if name.starts_with("hit_") {
                continue;
            }
            let id = graft::find_named(&sp, "beLuaData", name).unwrap().unwrap();
            let object = sp.val(id, 3).unwrap();
            let (size, mem) = (
                sp.val(object, 2).unwrap() as usize,
                sp.val(object, 3).unwrap(),
            );
            let range = sp.memory[mem as usize].clone().unwrap();
            let code = &sp.b[range.start..range.start + size];
            let words: Vec<String> = code
                .split(|c| !(c.is_ascii_graphic() || *c == b' '))
                .filter(|w| w.len() >= 3)
                .map(|w| String::from_utf8_lossy(w).into_owned())
                .collect();
            println!("{name} ({size} B): {words:?}");
        }
        let root = u32at(sp.b, sp.table + 8 + 2 * sp.ids.len()).unwrap();
        for e in sp.array(root).unwrap() {
            if ids.contains(&e) {
                continue;
            }
            let r = sp.rec(e).unwrap();
            let name = graft::name_of(&sp, r)
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            if !name.is_empty() && words.contains(&name) {
                println!("  refers to {} {name}", sp.classes[r.class].name);
            }
        }
        let mut effects: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for e in sp.array(root).unwrap() {
            let r = sp.rec(e).unwrap();
            let class = sp.classes[r.class].name.clone();
            if [
                "beParticleCtrl2Info",
                "ParticleInfo",
                "beActionStarterInfo",
                "beHitLandModelInfo",
                "beSwitchCtrlInfo",
                "beTextureCtrlInfo",
            ]
            .contains(&class.as_str())
            {
                let name = graft::name_of(&sp, r)
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .unwrap_or_default();
                effects.entry(class).or_default().push(name);
            }
        }
        println!("effect infos in the stage: {effects:?}");
        println!(
            "strings: {:?}",
            words
                .iter()
                .filter(|w| w.len() > 3)
                .take(400)
                .collect::<Vec<_>>()
        );
    }

    /// `DW4_RETAIL_USA_ISO=... cargo test probe_classes -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_classes() {
        let iso = std::fs::read(std::env::var("DW4_RETAIL_USA_ISO").unwrap()).unwrap();
        let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let base = file.lba as usize * 2048;
        let afs = &iso[base..base + file.size as usize];
        let ent = |i: usize| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        };
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
            println!(
                "w2 {w2}: classes at/after it: {tail:?}; total {}",
                pk.classes.len()
            );
            let used: std::collections::BTreeSet<usize> =
                pk.objects.iter().map(|r| r.class).collect();
            println!(
                "object classes in tail: {:?}",
                tail.iter()
                    .filter(|(i, _)| used.contains(i))
                    .collect::<Vec<_>>()
            );
            let dirc: std::collections::BTreeSet<usize> = pk.dirs.iter().map(|r| r.class).collect();
            println!("dir classes {dirc:?}");
        }
        let (o, l) = ent(665);
        let dst = &afs[o..o + l];
        let (so, sl) = ent(210);
        let src = Pack::parse(&afs[so..so + sl]).unwrap();
        let (classes, _, end) = class_table(dst).unwrap();
        println!(
            "665: {} classes, table ends at {end}, next bytes {:?}",
            classes.len(),
            &dst[end..end + 16]
        );
        let want = ["beModelCtrlAIMAP"];
        let defs = class_defs(src.b).unwrap();
        let add: Vec<ClassDef> = want
            .iter()
            .filter(|w| !classes.iter().any(|c| &c.name == *w))
            .map(|w| defs.iter().find(|c| &c.class.name == w).unwrap().clone())
            .collect();
        let out = add_classes(dst, &add).unwrap();
        let p = Pack::parse(&out).unwrap();
        println!(
            "after: {} classes; ids {} vs {}",
            p.classes.len(),
            p.ids.len(),
            Pack::parse(dst).unwrap().ids.len()
        );
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
            .map(|i| {
                (
                    u32at(afs, 8 + 8 * i).unwrap() as usize,
                    u32at(afs, 12 + 8 * i).unwrap() as usize,
                )
            })
            .collect();
        let layout = crate::release::detect(&iso)
            .unwrap()
            .layout_in(&iso)
            .unwrap();
        let table = EnemyTable::read(&iso, &layout).unwrap();
        let r = Randomiser::scan(
            afs,
            &entries,
            table,
            Options {
                seed: 1,
                donors: 3,
                warp: None,
                only: None,
            },
        )
        .unwrap();
        println!(
            "donors: {:?}",
            r.donors
                .iter()
                .map(|(m, d)| (*m, d.source, d.ram, d.profile.scene.clone()))
                .collect::<Vec<_>>()
        );
        for (&e, st) in &r.stages {
            let (o, l) = entries[e];
            let dp = Pack::parse(&afs[o..o + l]).unwrap();
            let em01 = graft::has_group(&dp, b"EM01");
            let mut ok = vec![];
            let mut no = vec![];
            for ((m, _), d) in &r.donors {
                if st.residents.contains_key(*m) {
                    continue;
                }
                match d.profile.misfit(&dp) {
                    None => ok.push(*m),
                    Some(w) => no.push(format!("{m}:{}", w.chars().take(28).collect::<String>())),
                }
            }
            println!(
                "stage {e}: EM01={em01} residents={:?} fit={} [{}] nofit={}",
                st.residents.keys().collect::<Vec<_>>(),
                ok.len(),
                ok.join(","),
                if ok.is_empty() {
                    no.join(" ")
                } else {
                    no.len().to_string()
                }
            );
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
        let ent = |i: usize| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        };
        let (o, l) = ent(622);
        let p = Pack::parse(&afs[o..o + l]).unwrap();
        let _ = n;
        for cls in [
            "beGeneraterData",
            "beGeneraterPlayerData",
            "beGeneraterPlayerDataList",
        ] {
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

#[cfg(test)]
mod dependency_audit {
    use super::*;

    #[test]
    fn every_scripted_spawn_and_quest_dependency_survives() {
        for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
            let Some(path) = env::var_os(variable) else {
                continue;
            };
            let iso = std::fs::read(path).unwrap();
            let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
                let f = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
                (f.offset as usize, f.size as usize)
            } else {
                let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (f.lba as usize * 2048, f.size as usize)
            };
            let afs = &iso[base..base + size];
            let entries = (0..u32at(afs, 4).unwrap() as usize)
                .map(|i| {
                    (
                        u32at(afs, 8 + 8 * i).unwrap() as usize,
                        u32at(afs, 12 + 8 * i).unwrap() as usize,
                    )
                })
                .collect::<Vec<_>>();
            let data = |e: usize| {
                let (o, l) = entries[e];
                &afs[o..o + l]
            };
            let layout = crate::release::detect(&iso)
                .unwrap()
                .layout_in(&iso)
                .unwrap();
            let mut r = Randomiser::scan(
                afs,
                &entries,
                EnemyTable::read(&iso, &layout).unwrap(),
                Options {
                    seed: 1520,
                    donors: 0,
                    warp: None,
                    only: None,
                },
            )
            .unwrap();
            for (e, models) in [
                (265, vec!["e_mummy", "e_pharaoh", "e_mecha1", "e_mecha2"]),
                (628, vec!["e_nume"]),
                (649, vec!["e_mummy", "e_tyrano"]),
                (686, vec!["e_torta"]),
            ] {
                for model in models {
                    assert!(
                        r.stages[&e].pinned_families.contains(model),
                        "{e} {model} is controlled by stage scripts"
                    );
                }
            }
            let (mut protected, mut sequences, mut changed) = (0, 0, 0);
            let mut kinds = BTreeSet::new();
            for (e, stage) in r.stages.clone() {
                let initial = if memchr::memmem::find(data(e), b"i_rareicon").is_some() {
                    materialize(data(e), &expand(data(e)).unwrap()).unwrap()
                } else {
                    data(e).to_vec()
                };
                let out = r
                    .stage(e, initial, afs, &entries)
                    .unwrap_or_else(|err| panic!("{variable} {e}: {err}"));
                for pack in &stage.gens {
                    let original = Pack::parse(data(*pack)).unwrap();
                    let after = r
                        .retarget(*pack, data(*pack))
                        .unwrap()
                        .unwrap_or_else(|| data(*pack).to_vec());
                    let after = Pack::parse(&after).unwrap();
                    let target = generators(&after).unwrap();
                    for g in generators(&original).unwrap() {
                        let target = target.iter().find(|n| n.name == g.name).unwrap();
                        let must_keep = g.seq
                            || stage.pinned.contains(&g.name)
                            || family(g.row).is_some_and(|m| stage.pinned_families.contains(m));
                        if must_keep {
                            assert_eq!(target.row, g.row, "{variable} {pack} {}", g.name);
                            let id = original.named("beGeneraterData", &g.name).unwrap();
                            let f = original
                                .rec(id)
                                .unwrap()
                                .field(graft::GENERATOR_SEQ)
                                .unwrap();
                            let t = after.rec(id).unwrap().field(graft::GENERATOR_SEQ).unwrap();
                            assert_eq!(original.b[f.clone()], after.b[t]);
                            protected += 1;
                            if g.seq && family(g.row).is_some_and(|m| m.starts_with("e_")) {
                                sequences += 1;
                                kinds.insert(
                                    String::from_utf8_lossy(&original.b[f.start + 4..f.end])
                                        .trim_end_matches('\0')
                                        .to_string(),
                                );
                            }
                        } else if target.row != g.row {
                            changed += 1;
                        }
                        if e == 449 && target.row != g.row {
                            assert!(["e_mummy", "e_mecha1", "e_minotaru", "e_pharaoh"]
                                .contains(&family(target.row).unwrap()));
                        }
                    }
                }
                if e == 449 {
                    assert!(
                        !r.stats.trace.iter().any(|s| s.starts_with("copy 449 ")),
                        "counter quest must use native hit scripts"
                    );
                }
                // All native model-owned scripts remain byte-identical, including
                // kill-counter scripts and summons; capacity edits only add slots.
                let p = Pack::parse(data(e)).unwrap();
                let q = Pack::parse(&out).unwrap();
                for id in 0..p.ids.len() as u32 {
                    if p.class(id, "beLuaData").is_ok()
                        && graft::name_of(&p, p.rec(id).unwrap())
                            .is_some_and(|n| n.starts_with(b"hit_") || n.starts_with(b"cal_"))
                    {
                        let obj = p.val(id, 3).unwrap();
                        let mem = p.val(obj, 3).unwrap();
                        assert_eq!(
                            p.b[p.memory[mem as usize].clone().unwrap()],
                            q.b[q.memory[mem as usize].clone().unwrap()]
                        );
                    }
                }
            }
            assert!(
                protected > 500 && sequences > 500 && changed > 100,
                "{variable}: {protected} protected, {sequences} entrances, {changed} changed"
            );
            assert!(kinds.is_superset(&BTreeSet::from([
                "SQ_WAIT".into(),
                "SQ_FALL".into(),
                "SQ_JUMP".into(),
                "SQ_JUMP1".into(),
                "SQ_ENTRHS".into(),
                "SQ_NENTR".into(),
                "SQ_ATCK1".into(),
                "SQ_SWON".into()
            ])));
            eprintln!("SCRIPT AUDIT {variable}: {protected} protected rows, {sequences} enemy entrances, {changed} ordinary rows changed; {kinds:?}");
        }
    }
}
