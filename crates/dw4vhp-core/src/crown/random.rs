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
pub const RULES_VERSION: u32 = 9;
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

pub(super) use super::encounters::REGULAR;
use super::encounters::{
    BOSS_REGULAR, FORBIDDEN_SOURCES, PROTECTIONS, REPLACEABLE, RESTRICTED, SKIPPED,
};

pub(super) fn family(row: u32) -> Option<&'static str> {
    ROW_GROUPS
        .iter()
        .find(|g| g.rows.iter().any(|&r| r as u32 == row))
        .map(|g| g.model)
}

fn replaceable(model: &str) -> bool {
    REGULAR.contains(&model) || REPLACEABLE.contains(&model)
}

/// Match combat strength independently of EXP, which comes from the original spawn.
pub(super) fn nearest_row(table: &EnemyTable, row: u32, model: &str) -> Option<u32> {
    let group = ROW_GROUPS.iter().find(|g| g.model == model)?;
    let row = row as usize;
    // Product ratios rank combat like the previous sum of logarithms. Integer
    // cross-products make ties deterministic across Windows and Linux.
    let distance = |a: i32, b: i32| {
        let (a, b) = (a.max(1) as u64, b.max(1) as u64);
        (a.max(b), a.min(b))
    };
    let combat = |r: usize| {
        let hp = distance(table.hp[r], table.hp[row]);
        let atk = distance(table.stat[r][0] as i32, table.stat[row][0] as i32);
        (hp.0 * atk.0, hp.1 * atk.1)
    };
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
            let (a_num, a_den) = combat(a);
            let (b_num, b_den) = combat(b);
            (u128::from(a_num) * u128::from(b_den))
                .cmp(&(u128::from(b_num) * u128::from(a_den)))
                .then(a.cmp(&b))
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
    pub(super) rewards: BTreeMap<usize, super::rewards::StageRewards>,
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
    pub(super) fn reward_plan(&self, iso: &[u8]) -> Result<super::rewards::Plan> {
        super::rewards::Plan::build(iso, &self.table, &self.rewards)
    }
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
            for (generator, sequence) in &plan.sequences {
                hash.update(format!("seq {pack} {generator} {sequence}\n").as_bytes());
            }
        }
        for (stage, rewards) in &self.rewards {
            for (row, sources) in &rewards.rows {
                hash.update(format!("xp {stage} {row} {sources:?}\n").as_bytes());
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
            "rewardVariants": self.rewards.values().map(|r| r.rows.len()).sum::<usize>(),
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
            rewards: BTreeMap::new(),
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
            let mut pinned: BTreeSet<String> = PROTECTIONS
                .iter()
                .filter(|rule| rule.pack_entry == e)
                .flat_map(|rule| rule.generators.iter().map(|n| n.to_string()))
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
        let mut rt = rt.clone();
        for stage in self.rewards.values() {
            if let Some(rows) = stage.targets.get(&entry) {
                rt.rows.extend(rows.clone());
            }
        }
        let (out, n) = graft::retarget_rows(b, &rt)?;
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
        let tiers = stage
            .gens
            .iter()
            .map(|&pack| Ok((pack, generators(&Pack::parse(data(pack)?)?)?)))
            .collect::<Result<Vec<_>>>()?;
        // Never reuse a row referenced by any original difficulty pack. Only
        // globally unfilled table slots are eligible for local variants.
        let used: BTreeSet<_> = tiers
            .iter()
            .flat_map(|(_, gs)| gs.iter().map(|g| g.row as usize))
            .collect();
        let free: Vec<_> = (0..self.table.len())
            .filter(|r| self.table.hp[*r] == 0 && !used.contains(r))
            .collect();
        let mut rewards = super::rewards::StageRewards::new(stage.gens.clone(), free);
        for (pack, _) in &tiers {
            self.plans.insert(*pack, Retarget::default());
        }
        // Commit a generator across all difficulty packs together. In the rare
        // event that local slots run out, retain its original enemy and reward.
        for name in choice.keys().cloned().collect::<Vec<_>>() {
            let model = &choice[&name];
            let mut attempt = rewards.clone();
            let mut selected = Vec::new();
            let mut fits = true;
            for (pack, gs) in &tiers {
                let Some(g) = gs.iter().find(|g| g.name == name) else {
                    continue;
                };
                let Some(combat) = nearest_row(&self.table, g.row, model) else {
                    continue;
                };
                let Some(target) = attempt.assign(combat as usize, g.row as usize) else {
                    fits = false;
                    break;
                };
                attempt
                    .targets
                    .entry(*pack)
                    .or_default()
                    .insert(name.clone(), target as u32);
                selected.push((*pack, combat));
            }
            if fits {
                rewards = attempt;
                for (pack, combat) in selected {
                    let plan = self.plans.get_mut(&pack).unwrap();
                    plan.rows.insert(name.clone(), combat);
                    // Native SQ_NENTR keeps SQ_SWON on the normal combat
                    // branch instead of seeking donor-stage healing positions.
                    if model == "e_minotaru" {
                        plan.sequences.insert(name.clone(), "SQ_NENTR".into());
                    }
                }
            } else {
                choice.remove(&name);
                for &(gen, original) in &sites {
                    if gen == name {
                        names.remove(original);
                    }
                }
                self.log.push(format!(
                    "stage {entry}: retain {name}; local reward variants full"
                ));
            }
        }
        if !rewards.rows.is_empty() {
            self.log.push(format!(
                "stage {entry}: {} original-spawn EXP variants",
                rewards.rows.len()
            ));
            self.rewards.insert(entry, rewards);
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
#[path = "random/encounter_tests.rs"]
mod dependency_audit;
#[cfg(all(test, feature = "devtools"))]
#[path = "random/probes.rs"]
mod probe;
#[cfg(test)]
#[path = "random/tests.rs"]
mod tests;
