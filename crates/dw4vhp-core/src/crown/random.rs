//! Experimental enemy randomiser. For every stage it copies a few extra enemy models into the
//! stage's model pack, then points each regular generator at one of the models the stage now
//! carries. Local rows inherit the original encounter's combat stats and rewards, with
//! bounded elemental differences from the selected family. An optional second pass shuffles
//! supported dungeon bosses after patrol planning. Other bosses, generators that stage
//! scripts wait on by name, and kill-count stages are left as shipped; cutscenes that wait on
//! an intro enemy by name get their name constants renamed to the model now spawned.
//! Model-specific entrances stay native; normal activations and reviewed script groups
//! follow replacements. Imported models carry their audio.
use super::graft::{self, Retarget, INTRO_STAGES};
use super::lua_names;
use super::*;
use crate::{rowgroup::ROW_GROUPS, table::EnemyTable};
use std::{collections::BTreeSet, env};

/// Bumped whenever a change alters what a seed produces, so a seed is only comparable
/// between images with the same rules version.
pub const RULES_VERSION: u32 = 28;
/// New models tried per stage by the shipped patch.
pub const DONORS: usize = 4;

mod distribution;

/// Mangrove's sequential enemy waves have model-named waits/cleanup in stage Lua.
/// Their completion callbacks belong to the generators, so they survive retargeting.
const MANGROVE_WAVES: &[(&str, &str)] = &[("GENE-139", "e_otama"), ("GENE-131", "e_geko")];

pub struct Options {
    pub seed: u64,
    /// Also shuffle compatible dungeon bosses after patrol planning.
    pub random_bosses: bool,
    /// How many new models to try to copy into each stage.
    pub donors: usize,
    /// For testing: make the Goblin Pass exit lead to this stage instead of the Abyss of Grief.
    pub warp: Option<String>,
    /// Randomise only these model packs (others still supply models); `None` randomises every stage.
    pub only: Option<Vec<usize>>,
    /// Analysis only: make every choice exactly as an image build would, but skip byte
    /// work that cannot change a choice (audio, capacity, reclaim, script edits,
    /// verification). The result reports the choices; it cannot be written as an image.
    pub picks_only: bool,
}

pub(super) use super::encounters::REGULAR;
use super::encounters::{
    replacement_allowed, BOSS_REGULAR, FAMILY_REMAPS, FORBIDDEN_SOURCES, PROTECTIONS, REPLACEABLE,
    RESTRICTED, SKIPPED,
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

/// Select a donor model row of matching rarity. Local variants inherit the
/// original spawn's strength; this row supplies the model and elemental flavour.
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

const RNG_STEP: u64 = 0x9E37_79B9_7F4A_7C15;

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

struct Rng(u64);
impl Rng {
    /// Hash the seed and salt into the start state. XORing them directly left small
    /// seeds on one shared stream: `next` steps by the multiplier used for the salt,
    /// so salts differing by `k` started `k` draws apart.
    fn new(seed: u64, salt: u64) -> Self {
        let seed = mix(seed.wrapping_add(RNG_STEP));
        Self(mix(seed.wrapping_add(salt.wrapping_mul(RNG_STEP))))
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(RNG_STEP);
        mix(self.0)
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
    entrance: String,
    start: String,
    pos: [f32; 3],
    /// Most instances the generator keeps alive at once, over the player counts.
    most: u32,
    population: u64,
}

/// Normal entrances can move only to models implementing the same named sequence.
/// Geometry-specific waits, falls and jumps remain native.
fn portable_entrance(g: &Gen) -> bool {
    matches!(g.entrance.as_str(), "SQ_NENTR" | "SQ_SWON")
}

fn protected_entrance(g: &Gen) -> bool {
    g.seq && !portable_entrance(g)
}

fn model_sequences(p: &Pack, model: &str) -> Result<BTreeSet<String>> {
    let info = p.named("beModelCtrlInfo", model)?;
    let mut sequences = BTreeSet::new();
    for data in p.array(p.val(info, 5)?)? {
        if p.class(data, "beModelCtrlInfoDataSeq").is_ok() {
            for sequence in p.array(p.val(data, 4)?)? {
                if let Some(name) = graft::name_of(p, p.rec(sequence)?) {
                    sequences.insert(String::from_utf8_lossy(name).into_owned());
                }
            }
        }
    }
    Ok(sequences)
}

/// Give every generator of `group` a local variant row on each difficulty, or
/// leave the plans and rewards unchanged if any of them has no free row.
fn commit_group(
    table: &EnemyTable,
    plans: &mut BTreeMap<usize, Retarget>,
    rewards: &mut super::rewards::StageRewards,
    tiers: &[(usize, Vec<Gen>)],
    choice: &BTreeMap<String, String>,
    group: &[String],
) -> bool {
    let mut attempt = rewards.clone();
    let mut selected = Vec::new();
    for name in group {
        let model = &choice[name];
        for (pack, gs) in tiers {
            let Some(g) = gs.iter().find(|g| g.name == *name) else {
                continue;
            };
            let Some(combat) = nearest_row(table, g.row, model) else {
                return false;
            };
            let Some(target) = attempt.assign(combat as usize, g.row as usize) else {
                return false;
            };
            attempt
                .targets
                .entry(*pack)
                .or_default()
                .insert(name.clone(), target as u32);
            selected.push((*pack, name.clone(), combat));
        }
    }
    *rewards = attempt;
    for (pack, name, combat) in selected {
        let model = &choice[&name];
        let plan = plans.get_mut(&pack).unwrap();
        plan.rows.insert(name.clone(), combat);
        // Native SQ_NENTR keeps SQ_SWON on the normal combat
        // branch instead of seeking donor-stage healing positions.
        if model == "e_minotaru" {
            plan.sequences.insert(name, "SQ_NENTR".into());
        } else if matches!(model.as_str(), "e_pharaoh" | "e_arachne") {
            // Use each model's real activation. Pharaohmon's NENTR
            // omits CHECK; Arukenimon only provides SQ_ENTR.
            plan.sequences.insert(name, "SQ_ENTR".into());
        }
    }
    true
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
                // Mangrove's wave wait and cleanup are explicitly retargeted
                // below. They must not pin the wave or unrelated patrol families.
                // ShogunGekomon and the scene's NPCs retain their native scripts.
                if entry == 686
                    && matches!(
                        (name, model),
                        ("mate00.lua", "e_otama") | ("tim_g_orbz03.lua", "e_geko" | "e_otama")
                    )
                {
                    continue;
                }
                if FAMILY_REMAPS.iter().any(|rule| {
                    rule.entry == entry
                        && rule.scripts.contains(&name)
                        && rule.models.contains(&model)
                }) {
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
        let sequence = r.field(graft::GENERATOR_SEQ)?;
        let entrance = String::from_utf8_lossy(&p.b[sequence.start + 4..sequence.end])
            .trim_end_matches('\0')
            .to_string();
        let seq = !entrance.is_empty();
        let mut most = 0;
        let mut population = 0;
        for pd in p.array(p.val(id as u32, GENERATOR_PLAYERS)?)? {
            most = most.max(p.val(pd, PLAYER_MAX)?);
            population = population.max(u64::from(p.val(pd, 2)?));
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
            entrance,
            most,
            population: population.max(u64::from(most)).max(1),
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
    /// Named sequences and projectile models from the source pack, read once:
    /// the source bytes never change while stages are planned.
    sequences: BTreeSet<String>,
    companions: Vec<String>,
    /// Source contents, which key analysis plans.
    fingerprint: [u8; 16],
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
    pub(super) opts: Options,
    pub(super) table: EnemyTable,
    /// Models by name and whether the source hangs them as a scene of their own.
    donors: BTreeMap<(&'static str, bool), Donor>,
    audio_sources: BTreeMap<&'static str, usize>,
    /// Split-file enemy bodies assembled with their home-stage dependencies.
    prepared_sources: BTreeMap<usize, Vec<u8>>,
    stages: BTreeMap<usize, Stage>,
    plans: BTreeMap<usize, Retarget>,
    pub(super) rewards: BTreeMap<usize, super::rewards::StageRewards>,
    pub(super) sounds: BTreeMap<usize, Vec<u8>>,
    /// Hard and Very Hard resources appended after the original archive entries.
    pub(super) extra: BTreeMap<usize, [Vec<u8>; 2]>,
    pub log: Vec<String>,
    stats: Stats,
    boss_summary: Option<serde_json::Value>,
    pub(super) boss_report: Option<serde_json::Value>,
    /// First appended message row of each card stage's localized packs.
    pub(super) card_bases: BTreeMap<usize, usize>,
    /// Stages whose introductions show an appended family card.
    pub(super) card_stages: BTreeSet<usize>,
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
        let summary = serde_json::json!({
            "rulesVersion": RULES_VERSION,
            "seed": self.opts.seed,
            "donorsPerStage": self.opts.donors,
            "difficultySpecificEnemies": true,
            "stages": self.stats.stages,
            "generators": self.stats.generators,
            "keptOriginal": self.stats.kept,
            "modelsCopied": self.stats.copied,
            "approximateEffects": self.stats.approximate,
            "undrawnEffects": self.stats.unattached,
            "planDigest": digest,
            "rewardVariants": self.rewards.values().map(|r| r.rows.len()).sum::<usize>(),
        });
        let mut summary = summary;
        if let Some(bosses) = &self.boss_summary {
            summary["bosses"] = bosses.clone();
        }
        summary
    }

    /// Find every stage and, for each regular model, the stage pack to copy it from.
    pub(super) fn scan(
        afs: &[u8],
        entries: &[(usize, usize)],
        table: EnemyTable,
        opts: Options,
    ) -> Result<Self> {
        let _timing = crate::profiling::measure("random.scan");
        let mut me = Self {
            opts,
            table,
            donors: BTreeMap::new(),
            audio_sources: BTreeMap::new(),
            prepared_sources: BTreeMap::new(),
            stages: BTreeMap::new(),
            plans: BTreeMap::new(),
            rewards: BTreeMap::new(),
            sounds: BTreeMap::new(),
            extra: BTreeMap::new(),
            log: Vec::new(),
            stats: Stats::default(),
            boss_summary: None,
            boss_report: None,
            card_bases: BTreeMap::new(),
            card_stages: BTreeSet::new(),
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
            // Model-specific entrances (cannon waits, train falls, jumps) belong to the original
            // model's animation and movement controller. Keep those spawns intact.
            for &pack in &gens {
                pinned.extend(
                    generators(&Pack::parse(data(pack).unwrap())?)?
                        .into_iter()
                        .filter(protected_entrance)
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
            if e == super::split_models::STAGE {
                // Its native one-instance body is loaded by the stage script.
                // Donors elsewhere use the embedded, expandable graph below.
                pinned_families.insert(super::split_models::MODEL);
            }
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
                            sequences: model_sequences(&p, m)?,
                            companions: graft::companion_models(&p, None, &[m.to_string()])?,
                            fingerprint: graft::source_fingerprint(d),
                            profile,
                        },
                    );
                }
            }
            me.stages.insert(e, stage);
        }
        need(!me.stages.is_empty(), "no stages found")?;
        let stage = super::split_models::STAGE;
        let model = super::split_models::MODEL;
        let source = super::split_models::arukenimon_source(
            data(stage).ok_or_else(|| bad("missing Arukenimon home stage"))?,
            data(super::split_models::ASSET).ok_or_else(|| bad("missing Arukenimon body"))?,
            data(super::split_models::GROUP_TEMPLATE)
                .ok_or_else(|| bad("missing enemy group template"))?,
        )?;
        let p = Pack::parse(&source)?;
        let profile = graft::profile(&p, model)?;
        let audio = super::split_models::AUDIO;
        need(
            super::sound::complete(
                data(audio).ok_or_else(|| bad("missing Arukenimon audio"))?,
                model,
            )?,
            "incomplete Arukenimon audio",
        )?;
        me.audio_sources.insert(model, audio);
        me.donors.insert(
            (model, false),
            Donor {
                source: stage,
                ram: profile.ram,
                rarities: rarities(&me.table, model),
                sequences: model_sequences(&p, model)?,
                companions: graft::companion_models(&p, None, &[model.to_string()])?,
                fingerprint: graft::source_fingerprint(&source),
                profile,
            },
        );
        me.prepared_sources.insert(stage, source);
        Ok(me)
    }

    /// Every choice that decides what a player meets: generator rows and entrances per
    /// difficulty pack, local stat/reward variants, imported models and boss swaps.
    pub(super) fn picks(&self) -> serde_json::Value {
        let plans: BTreeMap<_, _> = self
            .plans
            .iter()
            .map(|(pack, plan)| {
                (
                    pack.to_string(),
                    serde_json::json!({"rows": plan.rows, "sequences": plan.sequences}),
                )
            })
            .collect();
        let rewards: BTreeMap<_, _> = self
            .rewards
            .iter()
            .map(|(entry, r)| {
                let rows: BTreeMap<_, _> = r
                    .rows
                    .iter()
                    .map(|(row, v)| (row.to_string(), [v.combat, v.reward]))
                    .collect();
                (
                    entry.to_string(),
                    serde_json::json!({"rows": rows, "targets": r.targets}),
                )
            })
            .collect();
        let choices: Vec<_> = self
            .stats
            .trace
            .iter()
            .filter(|line| !line.starts_with("reclaim "))
            .collect();
        serde_json::json!({
            "rulesVersion": RULES_VERSION,
            "seed": self.opts.seed,
            "randomBosses": self.opts.random_bosses,
            "plans": plans,
            "rewards": rewards,
            "choices": choices,
            "bossReport": self.boss_report,
        })
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
        let gens = self.stages.get(&entry).map(|stage| stage.gens.clone());
        if let Some(mut gens) = gens.filter(|gens| gens.len() == 3).filter(|_| {
            self.opts
                .only
                .as_ref()
                .is_none_or(|only| only.contains(&entry))
        }) {
            // Native archive order is Hard, Normal, Very Hard. Plan in playthrough
            // order so each later difficulty prefers families not used before.
            gens.swap(0, 1);
            let stages_before = self.stats.stages;
            let mut previous = BTreeSet::new();
            let mut variants = Vec::new();
            for &pack in &gens {
                self.sounds.remove(&(entry - 1));
                self.stats.trace.push(format!("difficulty {entry} {pack}"));
                let bytes =
                    self.plan_stage(entry, dst.clone(), afs, entries, Some(pack), &previous)?;
                let model = self.finish_stage(entry, bytes)?;
                let sounds = self.sounds.remove(&(entry - 1)).unwrap_or_else(|| {
                    let (start, len) = entries[entry - 1];
                    afs[start..start + len].to_vec()
                });
                if let Some(plan) = self.plans.get(&pack) {
                    previous.extend(
                        plan.rows
                            .values()
                            .filter_map(|row| family(*row))
                            .map(str::to_string),
                    );
                }
                variants.push((pack, model, sounds));
            }
            let split = gens.iter().any(|pack| self.plans.contains_key(pack));
            self.stats.stages = stages_before + usize::from(split);
            let (_, normal_model, normal_sound) = variants.remove(0);
            self.sounds.insert(entry - 1, normal_sound);
            if split {
                for &pack in &gens {
                    self.plans.entry(pack).or_default();
                }
                let (_, hard_model, hard_sound) = variants.remove(0);
                let (_, very_hard_model, very_hard_sound) = variants.remove(0);
                self.extra.insert(entry, [hard_model, very_hard_model]);
                self.extra.insert(entry - 1, [hard_sound, very_hard_sound]);
            }
            return Ok(normal_model);
        }
        let dst = self.plan_stage(entry, dst, afs, entries, None, &BTreeSet::new())?;
        self.finish_stage(entry, dst)
    }

    pub(super) fn validate_tiers(&self, iso: &[u8]) -> Result<()> {
        super::difficulty::validate_tiers(
            iso,
            self.stages.values().map(|stage| stage.gens.as_slice()),
        )
    }

    #[cfg(test)]
    fn model_for_generator<'a>(&'a self, entry: usize, pack: usize, normal: &'a [u8]) -> &'a [u8] {
        let stage = &self.stages[&entry];
        match (
            stage.gens.len(),
            stage.gens.iter().position(|p| *p == pack),
            self.extra.get(&entry),
        ) {
            (3, Some(0), Some(extra)) => &extra[0],
            (3, Some(2), Some(extra)) => &extra[1],
            _ => normal,
        }
    }

    /// Analysis may skip byte work after the choices, except in boss arenas:
    /// boss replacement copies into the finished arena pack.
    fn skip_bytes(&self, entry: usize) -> bool {
        self.opts.picks_only
            && !(self.opts.random_bosses && boss::ARENAS.iter().any(|a| a.entry == entry))
    }

    fn finish_stage(&self, entry: usize, mut dst: Vec<u8>) -> Result<Vec<u8>> {
        if self.skip_bytes(entry) {
            return Ok(dst);
        }
        if super::encounters::ranged_stage(entry)
            && self
                .opts
                .only
                .as_ref()
                .is_none_or(|only| only.contains(&entry))
        {
            dst = super::ranged_ai::apply(&dst, entry)?;
        }
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
        donor: &Donor,
        dst: &[u8],
        data: &impl Fn(usize) -> Result<&'a [u8]>,
    ) -> Result<Vec<u8>> {
        let audio = *self
            .audio_sources
            .get(model)
            .ok_or_else(|| bad(&format!("{model}: no complete audio source")))?;
        super::sound::graft_enemy(
            dst,
            data(donor.source - 1)?,
            data(audio)?,
            model,
            &donor.companions,
        )
    }

    /// Copy donors, choose generators and retain the resources used by the resulting stage.
    fn plan_stage(
        &mut self,
        entry: usize,
        mut dst: Vec<u8>,
        afs: &[u8],
        entries: &[(usize, usize)],
        tier: Option<usize>,
        previous: &BTreeSet<String>,
    ) -> Result<Vec<u8>> {
        let Some(mut stage) = self.stages.get(&entry).cloned() else {
            return Ok(dst);
        };
        let _timing = crate::profiling::measure("random.plan_stage");
        let skip_bytes = self.skip_bytes(entry);
        let _stage_timing = crate::profiling::measure_owned(format!(
            "stage.{entry}.tier.{}",
            tier.unwrap_or(entry)
        ));
        let all_gens = stage.gens.clone();
        if let Some(pack) = tier {
            need(
                stage.gens.contains(&pack),
                "difficulty generator does not belong to stage",
            )?;
            stage.gens = vec![pack];
        }
        if self
            .opts
            .only
            .as_ref()
            .is_some_and(|only| !only.contains(&entry))
        {
            return Ok(dst);
        }
        let data = |e: usize| {
            self.prepared_sources
                .get(&e)
                .map(Vec::as_slice)
                .or_else(|| entries.get(e).and_then(|&(o, l)| afs.get(o..o + l)))
                .ok_or_else(|| bad("stage entry outside archive"))
        };
        if entry == 628 {
            // River enemies use standalone scenes. Supply the ordinary shared
            // group so projectile donors from grouped stages can render there.
            dst = graft::add_enemy_group(&dst, data(187)?, "e_nume")?;
        }
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
        let mut rng = Rng::new(self.opts.seed, tier.unwrap_or(entry) as u64);
        let first = generators(&Pack::parse(data(stage.gens[0])?)?)?;
        let mut required: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for &pack in &stage.gens {
            for g in generators(&Pack::parse(data(pack)?)?)? {
                if portable_entrance(&g) {
                    required.entry(g.name).or_default().insert(g.entrance);
                }
            }
        }
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
            .filter(|(m, _)| replacement_allowed(entry, m))
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
            .filter(|(m, _)| replacement_allowed(entry, m))
            .filter(|_| allowed.is_none())
            .collect();
        rng.shuffle(&mut candidates);
        // Prefer families not seen on earlier difficulties, retaining the
        // ordinary compatibility fallback when fewer novel donors fit.
        candidates.sort_by_key(|(model, _)| previous.contains(*model));
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
        // Picks for stages whose bytes nothing later reads are planned against a
        // summary of the pack instead of real copies. DW4_VIRTUAL_CHECK=1 also runs
        // the real copies and stops at the first disagreement.
        let virtual_only = self.skip_bytes(entry);
        let check = virtual_only && env::var_os("DW4_VIRTUAL_CHECK").is_some();
        let mut virt = if virtual_only {
            Some(graft::VirtualDestination::new(&Pack::parse(&dst)?)?)
        } else {
            None
        };
        for (m, own_scene) in candidates {
            // Off-platform stages have a restricted ranged pool: exhaust the
            // shuffled candidates before retaining natives after failed grafts.
            if added >= self.opts.donors
                || (!super::encounters::ranged_stage(entry) && tried >= self.opts.donors + 8)
            {
                break;
            }
            let donor = &self.donors[&(m, own_scene)];
            if pool.iter().any(|(model, _, _)| model == m) {
                continue;
            }
            let why = match &virt {
                Some(v) if !check => donor.profile.misfit_in(v),
                _ => {
                    let why = donor.profile.misfit(&Pack::parse(&dst)?);
                    if let Some(v) = &virt {
                        need(
                            donor.profile.misfit_in(v) == why,
                            &format!("virtual misfit differs: stage {entry} {m}"),
                        )?;
                    }
                    why
                }
            };
            if let Some(why) = why {
                self.log
                    .push(format!("stage {entry}: {m} does not fit: {why}"));
                continue;
            }
            let sequences = &donor.sequences;
            if !first.iter().any(|g| {
                !stage.pinned.contains(&g.name)
                    && !sites.iter().any(|(name, _)| *name == g.name)
                    && family(g.row)
                        .is_some_and(|f| usable(f) && !stage.pinned_families.contains(f))
                    && (donor.ram >= g.most || matches!(m, "e_pharaoh" | "e_arachne"))
                    && donor.rarities.contains(&self.table.rarity[g.row as usize])
                    && required.get(&g.name).is_none_or(|needed| {
                        needed.iter().all(|s| {
                            sequences.contains(s)
                                || (matches!(m, "e_pharaoh" | "e_arachne")
                                    && sequences.contains("SQ_ENTR"))
                        })
                    })
            }) {
                continue;
            }
            tried += 1;
            if let Some(v) = virt.as_mut() {
                let missing = donor.profile.missing_in(&*v);
                let outcome = graft::virtual_graft(
                    v,
                    (donor.fingerprint, data(donor.source)?),
                    &[m.to_string()],
                    &missing,
                );
                if !check {
                    match outcome {
                        Ok(_) => {
                            self.stats.copied += 1;
                            self.stats
                                .trace
                                .push(format!("copy {entry} {m} {}", donor.source));
                            imports.push((m, own_scene));
                            pool.push((m.to_string(), donor.ram, donor.rarities.clone()));
                            added += 1;
                            self.log.push(format!(
                                "stage {entry}: + {m} from {} (pack now {} of 65535 objects)",
                                donor.source,
                                v.id_count()
                            ));
                        }
                        Err(err) => self
                            .log
                            .push(format!("stage {entry}: {m} not copied: {err}")),
                    }
                    continue;
                }
                let real = Pack::parse(&dst)?.ids.len();
                let missing_real = donor.profile.missing(&Pack::parse(&dst)?);
                let base = if missing_real.is_empty() {
                    dst.clone()
                } else {
                    add_classes(&dst, &missing_real)?
                };
                let expected = graft::graft(&base, data(donor.source)?, &[m.to_string()]).and_then(
                    |(edits, _)| Ok(Pack::parse(&materialize(&base, &edits)?)?.ids.len() - real),
                );
                need(
                    outcome.as_ref().ok() == expected.as_ref().ok()
                        && outcome.is_err() == expected.is_err(),
                    &format!(
                        "virtual graft differs: stage {entry} {m}: virtual {:?}, real {:?}",
                        outcome.as_ref().map_err(ToString::to_string),
                        expected.as_ref().map_err(ToString::to_string)
                    ),
                )?;
            }
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
                    if !skip_bytes {
                        let sound_entry = entry - 1;
                        let current = self
                            .sounds
                            .get(&sound_entry)
                            .map(Vec::as_slice)
                            .unwrap_or(data(sound_entry)?);
                        let sounds = self.import_sounds(m, donor, current, &data)?;
                        self.sounds.insert(sound_entry, sounds);
                    }
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

        let model_pack = Pack::parse(&dst)?;
        let entrances = pool
            .iter()
            .map(|(model, _, _)| {
                // A copied model carries its source's sequence records unchanged.
                let imported = imports.iter().find(|(m, _)| *m == model.as_str());
                let sequences = match imported {
                    Some(key) if virt.is_some() && !check => self.donors[key].sequences.clone(),
                    _ => model_sequences(&model_pack, model)?,
                };
                Ok((model.clone(), sequences))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        // This difficulty decides its own generators and scripted targets.
        let mut choice: BTreeMap<String, String> = BTreeMap::new();
        let family_remap = FAMILY_REMAPS.iter().find(|rule| rule.entry == entry);
        // A retained native family must also remain exclusive to its scripted
        // group. Otherwise an intro/patrol could be erased by the group's cleanup.
        let mut reserved: BTreeSet<String> = family_remap
            .into_iter()
            .flat_map(|rule| rule.models.iter().map(|m| m.to_string()))
            .collect();
        let mut groups = Vec::new();
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        let eligible = |g: &Gen, pool: &[(String, u32, BTreeSet<u8>)], skip: &BTreeSet<String>| {
            let rarity = self.table.rarity[g.row as usize];
            let own = family(g.row);
            let mut options = pool
                .iter()
                .filter(|(m, ram, r)| {
                    (*ram >= g.most || matches!(m.as_str(), "e_pharaoh" | "e_arachne"))
                        && r.contains(&rarity)
                        && !skip.contains(m)
                        && required.get(&g.name).is_none_or(|needed| {
                            needed.iter().all(|sequence| {
                                entrances[m].contains(sequence)
                                    || (matches!(m.as_str(), "e_pharaoh" | "e_arachne")
                                        && matches!(sequence.as_str(), "SQ_SWON" | "SQ_NENTR")
                                        && entrances[m].contains("SQ_ENTR"))
                            })
                        })
                })
                .map(|(m, _, _)| m.clone())
                .collect::<Vec<_>>();
            // Ordinary native families are compatibility fallbacks, not extra
            // random choices competing with the four imported families.
            let imported: Vec<_> = options
                .iter()
                .filter(|m| !stage.residents.contains_key(m.as_str()))
                .cloned()
                .collect();
            if !imported.is_empty() {
                options = imported;
            }
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
        if let Some(rule) = family_remap {
            for &original in rule.models {
                let members: Vec<_> = first
                    .iter()
                    .filter(|g| family(g.row) == Some(original))
                    .collect();
                if members.is_empty()
                    || stage.pinned_families.contains(original)
                    || members.iter().any(|g| stage.pinned.contains(&g.name))
                {
                    continue;
                }
                let options: Vec<_> = eligible(members[0], &pool, &reserved)
                    .into_iter()
                    .filter(|model| {
                        members
                            .iter()
                            .all(|g| eligible(g, &pool, &reserved).contains(model))
                    })
                    .collect();
                if options.is_empty() {
                    continue;
                }
                let mut picks = options;
                if entry == super::ranged_ai::ENTRY {
                    rng.shuffle(&mut picks);
                    picks.truncate(super::river_mix::MAX_FAMILIES);
                } else {
                    let pick = picks[(rng.next() % picks.len() as u64) as usize].clone();
                    picks = vec![pick];
                }
                reserved.extend(picks.iter().cloned());
                let mut members = members;
                if picks.len() > 1 {
                    rng.shuffle(&mut members);
                }
                let mut group = Vec::new();
                for (index, g) in members.into_iter().enumerate() {
                    choice.insert(g.name.clone(), picks[index % picks.len()].clone());
                    group.push(g.name.clone());
                }
                groups.push(group);
            }
        }
        let mut why: BTreeMap<String, usize> = BTreeMap::new();
        let mut slots = Vec::new();
        for g in &first {
            // Introductions choose after the patrols, from the families they use.
            if choice.contains_key(&g.name)
                || stage.pinned.contains(&g.name)
                || sites.iter().any(|(name, _)| *name == g.name)
            {
                *why.entry("pinned or site".into()).or_default() += 1;
                continue;
            }
            let Some(fam) = family(g.row) else {
                *why.entry("not an enemy row".into()).or_default() += 1;
                continue;
            };
            if !usable(fam)
                || stage.pinned_families.contains(fam)
                || family_remap.is_some_and(|rule| rule.models.contains(&fam))
            {
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
            slots.push(distribution::Slot {
                name: g.name.clone(),
                options,
                weight: g.population,
            });
        }
        let weights = first
            .iter()
            .map(|g| (g.name.clone(), g.population))
            .collect();
        choice.extend(distribution::assign(slots, &choice, &weights, &mut rng));
        if env::var("DW4_RANDOM_WHY").is_ok() {
            self.log.push(format!(
                "stage {entry}: skipped {why:?}; pool {:?}",
                pool.iter()
                    .map(|(m, r, s)| format!("{m}:{r}:{s:?}"))
                    .collect::<Vec<_>>()
            ));
        }
        // A stage made entirely of Pharaohmon has no non-recursive summon
        // family. Retain one compatible ordinary patrol in that case.
        if allowed.is_none() && choice.values().any(|m| m == "e_pharaoh") {
            let companion = |m: &str| {
                m != "e_pharaoh"
                    && REGULAR.contains(&m)
                    && !reserved.contains(m)
                    && !stage.pinned_families.contains(m)
                    && entrances.get(m).is_some_and(|s| {
                        s.contains("SQ_NENTR") || s.contains("SQ_ENTR") || s.contains("SQ_SWON")
                    })
            };
            if !first.iter().any(|g| {
                choice
                    .get(&g.name)
                    .map(String::as_str)
                    .or_else(|| family(g.row))
                    .is_some_and(companion)
            }) {
                if let Some(g) = first.iter().find(|g| {
                    choice.get(&g.name).is_some_and(|m| m == "e_pharaoh")
                        && family(g.row).is_some_and(companion)
                        && !sites.iter().any(|(name, _)| *name == g.name)
                        && !family_remap
                            .is_some_and(|r| family(g.row).is_some_and(|m| r.models.contains(&m)))
                }) {
                    choice.remove(&g.name);
                } else {
                    // There is no safe stage-local summon source for imports.
                    // Native Pharaohmon can retain its authored summon callback.
                    choice.retain(|_, m| m != "e_pharaoh");
                    names.retain(|_, m| m != "e_pharaoh");
                }
            }
        }
        let tiers = stage
            .gens
            .iter()
            .map(|&pack| Ok((pack, generators(&Pack::parse(data(pack)?)?)?)))
            .collect::<Result<Vec<_>>>()?;
        // Never reuse a row referenced by any original difficulty pack. Only
        // globally unfilled table slots are eligible for local variants.
        let used: BTreeSet<_> = all_gens
            .iter()
            .map(|pack| generators(&Pack::parse(data(*pack)?)?))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .map(|g| g.row as usize)
            .collect();
        let free: Vec<_> = (0..self.table.len())
            .filter(|r| self.table.hp[*r] == 0 && !used.contains(r))
            .collect();
        let mut rewards = self
            .rewards
            .get(&entry)
            .cloned()
            .unwrap_or_else(|| super::rewards::StageRewards::new(all_gens, free));
        for (pack, _) in &tiers {
            self.plans.insert(*pack, Retarget::default());
        }
        // Scripted family groups commit atomically across every generator and
        // difficulty. If local slots run out, the entire group remains native.
        let grouped: BTreeSet<_> = groups.iter().flatten().cloned().collect();
        groups.extend(
            choice
                .keys()
                .filter(|name| !grouped.contains(*name))
                .map(|name| vec![name.clone()]),
        );
        for group in groups {
            if !commit_group(
                &self.table,
                &mut self.plans,
                &mut rewards,
                &tiers,
                &choice,
                &group,
            ) {
                for name in group {
                    choice.remove(&name);
                    self.log.push(format!(
                        "stage {entry}: retain {name}; local variants unavailable"
                    ));
                }
            }
        }
        // Introductions preview the finished stage: each shows a family its
        // enemies use on this difficulty, preferring imports and families
        // with a retail card. Protected encounters' families stay outside the
        // reach of the introduction's model-wide freeze and cleanup commands.
        let protected: BTreeSet<&str> = first
            .iter()
            .filter(|g| stage.pinned.contains(&g.name))
            .filter_map(|g| family(g.row))
            .collect();
        let enemies: BTreeSet<String> = first
            .iter()
            .filter(|g| {
                !stage.pinned.contains(&g.name) && !sites.iter().any(|(name, _)| *name == g.name)
            })
            .filter_map(|g| {
                choice
                    .get(&g.name)
                    .cloned()
                    .or_else(|| family(g.row).map(str::to_string))
            })
            .collect();
        let carded = |orig: &str| {
            self.card_bases.contains_key(&entry) && boss::intro::has_intro_card(entry, orig)
        };
        let mut shown = BTreeSet::new();
        for &(gen, orig) in &sites {
            let Some(g) = first.iter().find(|g| g.name == gen) else {
                continue;
            };
            let candidates = |imported: bool, used: bool| {
                pool.iter()
                    .filter(|(m, _, _)| stage.residents.contains_key(m) != imported)
                    .filter(|(m, _, _)| enemies.contains(m) == used)
                    .filter(|(m, _, _)| m == orig || !protected.contains(m.as_str()))
                    .cloned()
                    .collect::<Vec<_>>()
            };
            let imports = eligible(g, &candidates(true, true), &reserved);
            let natives = eligible(g, &candidates(false, true), &reserved);
            let has_card =
                |m: &String| m == orig || (carded(orig) && boss::intro::has_family_card(m));
            let mut order = Vec::new();
            for (list, card) in [
                (&imports, true),
                (&natives, true),
                (&imports, false),
                (&natives, false),
            ] {
                let mut tier: Vec<String> = list
                    .iter()
                    .filter(|m| has_card(m) == card)
                    .cloned()
                    .collect();
                rng.shuffle(&mut tier);
                // Two introductions on one stage preview different families when they can.
                tier.sort_by_key(|m| shown.contains(m));
                order.extend(tier);
            }
            // Without a compatible stage family, an exclusive resident the
            // stage's enemies do not use keeps the introduction randomised.
            let mut spare = eligible(g, &candidates(false, false), &reserved);
            rng.shuffle(&mut spare);
            order.extend(spare);
            for pick in order {
                choice.insert(gen.to_string(), pick.clone());
                let group = [gen.to_string()];
                if commit_group(
                    &self.table,
                    &mut self.plans,
                    &mut rewards,
                    &tiers,
                    &choice,
                    &group,
                ) {
                    let preview = enemies.contains(&pick);
                    shown.insert(pick.clone());
                    if !preview {
                        reserved.insert(pick.clone());
                    }
                    if pick != orig {
                        names.insert(orig.to_string(), pick.clone());
                    }
                    self.log.push(format!(
                        "stage {entry}: intro {gen} {} {pick}",
                        if preview {
                            "previews"
                        } else {
                            "uses exclusive"
                        }
                    ));
                    break;
                }
                choice.remove(gen);
            }
            if !choice.contains_key(gen) {
                self.log
                    .push(format!("stage {entry}: retain native intro {gen}"));
            }
        }
        // Introduction cards follow their models when retail text exists.
        let picks = sites
            .iter()
            .filter(|(_, orig)| carded(orig))
            .map(|&(gen, orig)| (orig, choice.get(gen).map_or(orig, String::as_str)))
            .collect::<Vec<_>>();
        let cards = match self.card_bases.get(&entry) {
            Some(&base) if !picks.is_empty() => boss::intro::intro_cards(entry, &picks, base)?,
            _ => BTreeMap::new(),
        };
        if !cards.is_empty() {
            self.card_stages.insert(entry);
            self.stats
                .trace
                .push(format!("intro-cards {entry} {picks:?}"));
        }
        // A model pool belongs to the stage, not to an individual generator.
        // Sum the largest authored party count for every user of that model,
        // including pinned/unchanged generators, and take the maximum over the
        // difficulty packs (only one difficulty is loaded at a time).
        let mut capacity_tiers = Vec::new();
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
            capacity_tiers.push(tier);
        }
        let native_demand = shared_capacity(capacity_tiers.clone(), &BTreeMap::new());
        let demand = shared_capacity(capacity_tiers, &stage.residents);
        // Candidate imports make selection independent of how many generators
        // eventually use each family. Rebuild without unused candidates: even
        // unreachable objects are allocated by the game's IGB loader.
        if skip_bytes {
            // Record the retained imports as the rebuild below does.
            if imports
                .iter()
                .any(|(model, _)| demand.get(*model).copied().unwrap_or(0) == 0)
            {
                self.stats.copied = original_stats.0;
                self.stats.trace.truncate(original_stats.3);
                for (model, scene) in &imports {
                    if demand.get(*model).copied().unwrap_or(0) == 0 {
                        self.log.push(format!("stage {entry}: omit unused {model}"));
                        pool.retain(|(m, _, _)| m != model);
                        continue;
                    }
                    self.stats.copied += 1;
                    let source = self.donors[&(*model, *scene)].source;
                    self.stats
                        .trace
                        .push(format!("copy {entry} {model} {source}"));
                }
            }
        } else if imports
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
                sounds = self.import_sounds(model, donor, &sounds, &data)?;
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
        for model in assigned.iter().filter(|_| !skip_bytes) {
            let needed = demand.get(model).copied().unwrap_or(0);
            let old = super::capacity::slots(&Pack::parse(&dst)?, model)?;
            if needed > old {
                dst = super::capacity::expand(&dst, model, needed)?;
                self.log.push(format!(
                    "stage {entry}: {model} shared slots {old} -> {needed}"
                ));
            }
        }
        // Summons use the final selected families, not speculative imports.
        // Keep counter-aware quests and scripted exclusive families authored.
        if allowed.is_none()
            && !stage.pinned_families.contains("e_pharaoh")
            && demand.get("e_pharaoh").copied().unwrap_or(0) > 0
        {
            let mut summons = Vec::new();
            for model in demand.keys().map(String::as_str) {
                if model == "e_pharaoh"
                    || !REGULAR.contains(&model)
                    || reserved.contains(model)
                    || stage.pinned_families.contains(model)
                    || !entrances.get(model).is_some_and(|s| {
                        s.contains("SQ_NENTR") || s.contains("SQ_ENTR") || s.contains("SQ_SWON")
                    })
                {
                    continue;
                }
                let mut attempt = rewards.clone();
                let mut rows = Vec::new();
                for original in [116, 120, 124] {
                    if let Some(combat) = nearest_row(&self.table, original, model) {
                        if let Some(row) = attempt.assign(combat as usize, original as usize) {
                            rows.push(row);
                        }
                    }
                }
                if rows.len() != 3 {
                    // Crowded stages can consume every local variant row on
                    // their generators. Reuse a selected encounter per tier
                    // rather than failing the build or using unbalanced donors.
                    rows = tiers
                        .iter()
                        .filter_map(|(pack, gs)| {
                            gs.iter().find_map(|g| {
                                let combat =
                                    self.plans[pack].rows.get(&g.name).copied().unwrap_or(g.row);
                                if family(combat) != Some(model) {
                                    return None;
                                }
                                Some(
                                    rewards
                                        .targets
                                        .get(pack)
                                        .and_then(|targets| targets.get(&g.name))
                                        .copied()
                                        .unwrap_or(combat)
                                        as usize,
                                )
                            })
                        })
                        .collect();
                    if rows.len() == 1 && tier.is_some() {
                        rows.resize(3, rows[0]);
                    }
                    if rows.len() != 3 {
                        continue;
                    }
                    self.log.push(format!(
                        "stage {entry}: {model} summons reuse selected encounter rows {rows:?}"
                    ));
                } else {
                    rewards = attempt;
                }
                let base = if skip_bytes {
                    0
                } else {
                    let base =
                        super::capacity::slots(&Pack::parse(&dst)?, model)?.max(demand[model]);
                    dst = super::capacity::expand(&dst, model, base + super::pharaoh::LIMIT)?;
                    base
                };
                summons.push(super::pharaoh::Target {
                    model: model.to_string(),
                    entrance: if entrances[model].contains("SQ_NENTR") {
                        "SQ_NENTR"
                    } else if entrances[model].contains("SQ_ENTR") {
                        "SQ_ENTR"
                    } else {
                        "SQ_SWON"
                    }
                    .into(),
                    base,
                    rows: rows.try_into().unwrap(),
                });
            }
            need(
                !summons.is_empty(),
                &format!("stage {entry}: no compatible Pharaohmon summons"),
            )?;
            if !skip_bytes {
                dst = super::pharaoh::rewrite(&dst, &summons, entry)?;
            }
            self.log.push(format!(
                "stage {entry}: Pharaohmon rolls each summon from {summons:?}"
            ));
        }
        if !rewards.rows.is_empty() {
            self.log.push(format!(
                "stage {entry}: {} original-spawn EXP variants",
                rewards.rows.len()
            ));
            self.rewards.insert(entry, rewards);
        }
        // Everything below until the statistics edits only bytes for choices already made.
        if !skip_bytes {
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
                let mut names = names.clone();
                names.extend(cards);
                if !names.is_empty() {
                    let more =
                        lua_names::rename_in_pack(&dst, &[intro.script.to_string()], &names)?;
                    dst = materialize(&dst, &more)?;
                }
            }
            if let Some(rule) = family_remap {
                if entry == super::ranged_ai::ENTRY {
                    // Reward fallback has finalized the assignments. Cleanup and
                    // freeze must cover every family actually used by the banks.
                    let families = first
                        .iter()
                        .filter(|g| family(g.row) == Some("e_nume"))
                        .map(|g| {
                            choice
                                .get(&g.name)
                                .cloned()
                                .unwrap_or_else(|| "e_nume".into())
                        })
                        .collect();
                    let edits = super::river_mix::remap_scripts(&dst, rule.scripts, &families)?;
                    dst = materialize(&dst, &edits)?;
                } else {
                    let names = rule
                        .models
                        .iter()
                        .map(|&original| {
                            let replacement = first
                                .iter()
                                .find(|g| family(g.row) == Some(original))
                                .and_then(|g| choice.get(&g.name))
                                .cloned()
                                .unwrap_or_else(|| original.to_string());
                            (original.to_string(), replacement)
                        })
                        .collect();
                    let scripts = rule
                        .scripts
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>();
                    let edits = lua_names::rename_in_pack(&dst, &scripts, &names)?;
                    dst = materialize(&dst, &edits)?;
                }
            }
            if entry == 686 {
                // Do this after reward-slot fallback has finalized the choices, so
                // a retained native wave keeps its native wait/cleanup target.
                // Wave families may also appear in ordinary patrols, as they do in
                // retail; reserving both would unnecessarily narrow patrol variety.
                let waves: BTreeMap<String, String> = MANGROVE_WAVES
                    .iter()
                    .map(|&(generator, original)| {
                        (
                            original.to_string(),
                            choice
                                .get(generator)
                                .cloned()
                                .unwrap_or_else(|| original.into()),
                        )
                    })
                    .collect();
                for script in ["mate00.lua", "tim_g_orbz03.lua"] {
                    let names = waves
                        .iter()
                        .filter(|(original, _)| script != "mate00.lua" || *original == "e_otama")
                        .map(|(a, b)| (a.clone(), b.clone()))
                        .collect();
                    let edits = lua_names::rename_in_pack(&dst, &[script.into()], &names)?;
                    dst = materialize(&dst, &edits)?;
                }
            }
            if allowed.is_none() {
                for model in stage.residents.keys().filter(|m| replaceable(m)) {
                    if stage.pinned_families.contains(model.as_str()) {
                        continue;
                    }
                    let count = native_demand.get(model).copied().unwrap_or(0);
                    let mut external_script = false;
                    for &pack in &stage.gens {
                        external_script |=
                            super::reclaim::script_dependency(data(pack)?, model, count == 0)?;
                    }
                    if external_script {
                        continue;
                    }
                    let before = dst.len();
                    let reviewed_intro = intro
                        .filter(|_| {
                            sites.iter().any(|(gen, original)| {
                                choice.get(*gen).map(String::as_str).unwrap_or(original) == model
                            })
                        })
                        .map(|site| site.script);
                    let reclaimed = if count == 0 {
                        super::reclaim::unused(&dst, model)
                    } else if !super::reclaim::pool_script_dependency(
                        &dst,
                        model,
                        false,
                        reviewed_intro,
                    )? {
                        super::reclaim::shrink(&dst, model, count).map(Some)
                    } else {
                        Ok(None)
                    };
                    match reclaimed {
                        Ok(Some(bytes)) if bytes.len() < before => {
                            let saved = before - bytes.len();
                            dst = bytes;
                            if count == 0 {
                                pool.retain(|(m, _, _)| m != model);
                            }
                            self.log.push(format!("stage {entry}: reclaim native {model}, {count} required slots, {saved} serialized bytes saved"));
                            self.stats
                                .trace
                                .push(format!("reclaim {entry} {model} {count} {saved}"));
                        }
                        Err(error) => self.log.push(format!(
                            "stage {entry}: retain native {model} resources: {error}"
                        )),
                        _ => {}
                    }
                }
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

#[path = "random/boss.rs"]
pub(super) mod boss;
#[cfg(all(test, feature = "devtools"))]
#[path = "random/boss_audit.rs"]
mod boss_audit;
#[cfg(all(test, feature = "devtools"))]
#[path = "random/boss_image.rs"]
mod boss_image;
#[path = "random/boss_plan.rs"]
mod boss_plan;
#[cfg(test)]
#[path = "random/encounter_tests.rs"]
mod dependency_audit;
#[cfg(all(test, feature = "devtools"))]
#[path = "random/probes.rs"]
mod probe;
#[cfg(test)]
#[path = "random/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "random/train_tests.rs"]
mod train_tests;

#[cfg(test)]
mod pharaoh_tests;
