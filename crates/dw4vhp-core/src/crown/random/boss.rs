//! Optional boss shuffle, applied after the final patrol selections. It covers
//! the dungeon arenas and the repeat overworld and quest fights; the world
//! bosses are not part of it.
use super::*;
use serde_json::json;

#[path = "boss_intro.rs"]
pub(in crate::crown) mod intro;

pub(in crate::crown) struct Arena {
    pub(in crate::crown) entry: usize,
    pub(super) model: &'static str,
    /// The generators that spawn this encounter. They change together.
    pub(super) generators: &'static [&'static str],
    /// The stage script starts only one of `generators` (a randomly chosen
    /// strength variant), rather than possibly having several alive at once.
    pub(super) alternatives: bool,
    /// The script that starts the encounter. In dungeon arenas it also shows
    /// the boss's information card.
    pub(super) intro: &'static str,
    /// A reviewed command `intro` sends the boss that a replacement may lack.
    /// It is then mapped to the replacement's activation.
    pub(super) command: Option<&'static str>,
    /// Floors whose patrol families a replacement must not duplicate.
    pub(super) floors: &'static [usize],
    pub(super) name: &'static str,
}

impl Arena {
    /// Dungeon arenas show the boss's information card; the repeat fights do not.
    pub(super) fn card(&self) -> bool {
        ARENAS.iter().any(|a| a.entry == self.entry)
    }
}

/// Dungeon boss arenas, in card-bank order. Each is also the import source of its family.
pub(in crate::crown) const ARENAS: &[Arena] = &[
    Arena {
        entry: 187,
        floors: &[179, 187],
        model: "e_blosso",
        generators: &["GENE-08"],
        alternatives: false,
        intro: "matg08.lua",
        command: None,
        name: "Humid Cave B2",
    },
    Arena {
        entry: 210,
        floors: &[195, 203, 210],
        model: "e_mammos",
        generators: &["GENE-12"],
        alternatives: false,
        intro: "matg11.lua",
        command: Some("SQ_WALKIN"),
        name: "Cliff Dungeon B3",
    },
    Arena {
        entry: 241,
        floors: &[233, 241],
        model: "e_skulgrey",
        generators: &["GENE-13"],
        alternatives: false,
        intro: "matg12.lua",
        command: None,
        name: "Sand Labyrinth B2",
    },
    Arena {
        entry: 265,
        floors: &[249, 257, 265],
        model: "e_anomalo",
        generators: &["GENE-15"],
        alternatives: false,
        intro: "matg15.lua",
        command: None,
        name: "Ancient Ruins B3",
    },
    Arena {
        entry: 296,
        floors: &[288, 296],
        model: "e_tonogeko",
        generators: &["GENE-11"],
        alternatives: false,
        intro: "matg10.lua",
        command: Some("SQ_SWON"),
        name: "Geko Path B2",
    },
    Arena {
        entry: 319,
        floors: &[304, 312, 319],
        model: "e_mecha4",
        generators: &["GENE-19"],
        alternatives: false,
        intro: "matg19.lua",
        command: None,
        name: "Vine Tunnel B3",
    },
    Arena {
        entry: 349,
        floors: &[327, 335, 342, 349],
        model: "e_diabolo",
        generators: &["GENE-24"],
        alternatives: false,
        intro: "matg24.lua",
        command: None,
        name: "Vein B4",
    },
    Arena {
        entry: 394,
        floors: &[379, 387, 394],
        model: "e_mecha5",
        generators: &["GENE-23"],
        alternatives: false,
        intro: "matg23.lua",
        command: None,
        name: "Mecha Nest B3",
    },
    Arena {
        entry: 425,
        floors: &[402, 410, 417, 425],
        model: "e_ldknight",
        generators: &["GENE-21"],
        alternatives: false,
        intro: "matg21.lua",
        command: None,
        name: "Electro Mine B4",
    },
];

/// Repeat fights against dungeon bosses: overworld rematches and quest bosses.
/// They have no information card. Quest stages with separate difficulty
/// variants (`e`, `h`, `v`) are separate encounters.
pub(in crate::crown) const REPEATS: &[Arena] = &[
    Arena {
        entry: 621,
        floors: &[621],
        model: "e_blosso",
        generators: &["GENE-38"],
        alternatives: false,
        intro: "mate01.lua",
        command: None,
        name: "Abyss of Grief",
    },
    Arena {
        entry: 649,
        floors: &[649],
        model: "e_skulgrey",
        generators: &["GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"],
        alternatives: true,
        intro: "mata05.lua",
        command: None,
        name: "Grief Wasteland",
    },
    Arena {
        entry: 686,
        floors: &[686],
        model: "e_tonogeko",
        generators: &["GENE-s01", "GENE-s02", "GENE-s03", "GENE-s04", "GENE-s05"],
        alternatives: true,
        intro: "gene131.lua",
        command: None,
        name: "Mangrove Forest",
    },
    Arena {
        entry: 441,
        floors: &[433, 441],
        model: "e_mammos",
        generators: &["GENE-06"],
        alternatives: false,
        intro: "matg06.lua",
        command: Some("SQ_WALKIN"),
        name: "Quest 1, stage 2",
    },
    Arena {
        entry: 510,
        floors: &[486, 510],
        model: "e_anomalo",
        generators: &["GENE-11"],
        alternatives: false,
        intro: "matg11.lua",
        command: None,
        name: "Quest 4, stage 2 (Normal)",
    },
    Arena {
        entry: 518,
        floors: &[494, 518],
        model: "e_anomalo",
        generators: &["GENE-11"],
        alternatives: false,
        intro: "matg11.lua",
        command: None,
        name: "Quest 4, stage 2 (Hard)",
    },
    Arena {
        entry: 526,
        floors: &[502, 526],
        model: "e_anomalo",
        generators: &["GENE-11"],
        alternatives: false,
        intro: "matg11.lua",
        command: None,
        name: "Quest 4, stage 2 (Very Hard)",
    },
    Arena {
        entry: 556,
        floors: &[548, 556],
        model: "e_tonogeko",
        generators: &["GENE-22"],
        alternatives: false,
        intro: "matg15.lua",
        command: None,
        name: "Quest 7, stage 2",
    },
    Arena {
        entry: 579,
        floors: &[564, 572, 579],
        model: "e_ldknight",
        generators: &["GENE-22"],
        alternatives: false,
        intro: "matg22.lua",
        command: None,
        name: "Quest 8, stage 3 (Normal)",
    },
    Arena {
        entry: 587,
        floors: &[564, 572, 587],
        model: "e_ldknight",
        generators: &["GENE-22", "GENE-23"],
        alternatives: false,
        intro: "matg22.lua",
        command: None,
        name: "Quest 8, stage 3 (Hard)",
    },
    Arena {
        entry: 595,
        floors: &[564, 572, 595],
        model: "e_ldknight",
        generators: &["GENE-22", "GENE-23", "GENE-24"],
        alternatives: false,
        intro: "matg22.lua",
        command: None,
        name: "Quest 8, stage 3 (Very Hard)",
    },
];

/// Every shuffled boss encounter: the dungeon arenas, then the repeat fights.
pub(in crate::crown) fn sites() -> impl Iterator<Item = &'static Arena> {
    ARENAS.iter().chain(REPEATS)
}

/// Vine Tunnel's MR04 is the only dungeon boss without a health bar, so its
/// combat callback neither updates nor closes the bar every other arena opens.
/// The final dungeon's MR04 uses the same callback plus those two calls.
const BAR_CALLBACKS: &[(&str, usize)] = &[("e_mecha4", 786)];

/// The import source of a boss family, with a combat callback that drives the
/// boss health bar of the arenas it can replace.
pub(super) fn donor_source<'a>(
    model: &str,
    data: &impl Fn(usize) -> &'a [u8],
) -> Result<std::borrow::Cow<'a, [u8]>> {
    let source = data(source(model)?.entry);
    let Some(&(_, entry)) = BAR_CALLBACKS.iter().find(|(m, _)| *m == model) else {
        return Ok(source.into());
    };
    let hit = format!("hit_{model}.lua");
    let with_bar = Pack::parse(data(entry))?;
    need(
        !hud_names(&with_bar, model)?.is_empty(),
        "boss health-bar callback missing",
    )?;
    let callback = script(&with_bar, &hit)?.to_vec();
    let edits = lua_names::rewrite_in_pack(source, std::slice::from_ref(&hit), |_, _| {
        Ok(callback.clone())
    })?;
    Ok(materialize(source, &edits)?.into())
}

/// The import source of a boss family: its dungeon arena.
pub(super) fn source(model: &str) -> Result<&'static Arena> {
    ARENAS
        .iter()
        .find(|a| a.model == model)
        .ok_or_else(|| bad("boss family has no source arena"))
}

pub(super) fn script<'a>(pack: &Pack<'a>, name: &str) -> Result<&'a [u8]> {
    let id = pack.named("beLuaData", name)?;
    let object = pack.val(id, 3)?;
    let size = pack.val(object, 2)? as usize;
    let range = pack
        .memory
        .get(pack.val(object, 3)? as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing boss script memory"))?;
    need(size <= range.len(), "boss script exceeds its memory")?;
    Ok(&pack.b[range.start..range.start + size])
}

/// Reject unreviewed direct dependencies on another enemy body. Attack effects
/// and projectile companions have their own supported graft paths.
fn external_enemies(source: &[u8], model: &str) -> Result<BTreeSet<String>> {
    let p = Pack::parse(source)?;
    let roots = [p.named("beModelCtrlInfo", model)?];
    let (objects, _) = graft::closure(&p, &roots)?;
    let mut names = BTreeSet::new();
    for id in objects {
        let Some(index) = p.ids[id as usize] else {
            continue;
        };
        let r = &p.objects[index];
        if !p.classes[r.class].name.starts_with("beModelCtrl") {
            continue;
        }
        for &(kind, slot, _) in &p.classes[r.class].fields {
            if kind != 4 {
                continue;
            }
            let field = r.field(slot)?;
            let text = p.b[field.start + 4..field.end]
                .split(|b| *b == 0)
                .next()
                .unwrap_or_default();
            for group in ROW_GROUPS {
                if group.model != model
                    && group.model.starts_with("e_")
                    && text == group.model.as_bytes()
                {
                    names.insert(group.model.to_string());
                }
            }
        }
    }
    for prefix in graft::MODEL_SCRIPTS {
        let name = format!("{prefix}{model}.lua");
        if graft::find_named(&p, "beLuaData", &name)?.is_none() {
            continue;
        }
        for constant in lua_names::constants(script(&p, &name)?)? {
            for group in ROW_GROUPS {
                if group.model != model
                    && group.model.starts_with("e_")
                    && constant.text == group.model.as_bytes()
                {
                    names.insert(group.model.to_string());
                }
            }
        }
    }
    Ok(names)
}

pub(super) fn hud_names(p: &Pack, model: &str) -> Result<BTreeMap<&'static str, String>> {
    let mut names = BTreeMap::new();
    let hit = format!("hit_{model}.lua");
    if graft::find_named(p, "beLuaData", &hit)?.is_none() {
        return Ok(names);
    }
    for constant in lua_names::constants(script(p, &hit)?)? {
        let Ok(name) = std::str::from_utf8(&constant.text) else {
            continue;
        };
        for prefix in ["i_bosshp", "bshp"] {
            if name
                .strip_prefix(prefix)
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                need(
                    names.insert(prefix, name.to_string()).is_none(),
                    "ambiguous boss health display",
                )?;
            }
        }
    }
    Ok(names)
}

fn retarget_hud(bytes: &[u8], arena: &Arena, to: &str) -> Result<Vec<u8>> {
    let p = Pack::parse(bytes)?;
    let destination = hud_names(&p, arena.model)?;
    let donor = hud_names(&p, to)?;
    let names = donor
        .into_iter()
        .filter_map(|(kind, old)| {
            destination
                .get(kind)
                .filter(|new| **new != old)
                .map(|new| (old, new.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    if names.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut scripts = Vec::new();
    for prefix in graft::MODEL_SCRIPTS.iter().copied().chain(["wht_"]) {
        let name = format!("{prefix}{to}.lua");
        if graft::find_named(&p, "beLuaData", &name)?.is_some()
            && lua_names::constants(script(&p, &name)?)?
                .iter()
                .any(|c| names.keys().any(|old| c.text == old.as_bytes()))
        {
            scripts.push(name);
        }
    }
    let edits = lua_names::rewrite_in_pack(bytes, &scripts, |_, raw| {
        lua_names::rename(raw, &names)?.ok_or_else(|| bad("missing donor health display"))
    })?;
    materialize(bytes, &edits)
}

pub(super) fn prepare_model(
    dst: &[u8],
    src: &[u8],
    arena: &Arena,
    to: &str,
    count: usize,
) -> Result<Vec<u8>> {
    need(
        external_enemies(src, to)?.is_empty(),
        "boss has unreviewed enemy-body dependencies",
    )?;
    let mut base = dst.to_vec();
    if graft::resident(&base, to)? {
        base = super::super::reclaim::unused(&base, to)?
            .ok_or_else(|| bad("an existing donor body still has stage dependencies"))?;
    }
    let mut out = graft_model(&base, src, arena.model, to)?;
    out = retarget_hud(&out, arena, to)?;
    // Mammothmon's walk-in and ShogunGekomon's switch-on are explicit intro
    // commands. Map only these reviewed boss commands when the donor lacks them;
    // door and camera sequences remain untouched.
    let sequences = model_sequences(&Pack::parse(&out)?, to)?;
    // Timeout and cleanup scripts (Mangrove's orb timer) defeat the boss by name.
    need(
        sequences.contains("SQ_DDMG"),
        "replacement boss lacks the scripted defeat sequence",
    )?;
    let mut changes = BTreeMap::new();
    // Blossomon's SQ_NENTR configures normal-enemy death labels, while
    // SQ_ENTR selects its boss callback. Keep the reviewed fallback pool, but
    // prefer the boss activation when the donor provides it.
    let activation = if sequences.contains("SQ_ENTR") {
        "SQ_ENTR"
    } else {
        "SQ_NENTR"
    };
    // In dungeon arenas the command fires while the information card is still
    // up and the players are locked. A replacement's activation there starts
    // its normal behaviour (Lordknightmon advanced on the locked player), so
    // it idles instead, like the bosses of the other arenas.
    let (replacement, check) = if arena.card() {
        ("SQ_WAIT", "boss lacks an idle sequence")
    } else {
        (activation, "boss lacks a portable activation")
    };
    if let Some(command) = arena.command.filter(|c| !sequences.contains(*c)) {
        changes.insert(command.into(), replacement.into());
    }
    if !changes.is_empty() {
        need(
            sequences.contains("SQ_NENTR") && sequences.contains(replacement),
            check,
        )?;
        let edits = lua_names::rename_in_pack(&out, &[arena.intro.to_string()], &changes)?;
        out = materialize(&out, &edits)?;
    }
    out = super::super::capacity::expand(&out, to, count)?;
    // Keep the dormant arena model and its original callback namespace. Some
    // arenas dispatch victory through their native model/UI callback rather
    // than through the replacement's name. These callbacks were retargeted by
    // prototype(), and the original generator no longer spawns this body.
    need(
        graft::model_is_drawn(&Pack::parse(&out)?, to)?,
        "replacement boss has no registered draw scene",
    )?;
    // Generator counts are unchanged, so the final patrol build's existing
    // crown pool already covers the arena's active actors.
    Ok(out)
}

struct Files {
    models: Vec<Vec<u8>>,
    sounds: Vec<Vec<u8>>,
    packs: [usize; 3],
}

impl Randomiser {
    /// Unfilled stat rows the stage can still give new boss variants.
    fn spare_rows<'a>(&self, entry: usize, data: &impl Fn(usize) -> &'a [u8]) -> Result<usize> {
        if let Some(rewards) = self.rewards.get(&entry) {
            return Ok(rewards.spare());
        }
        Ok(self.free_rows(entry, data)?.len())
    }

    /// Whether only `arena`'s generators in difficulty pack `boss_pack` use
    /// `row` in the stage's final generator packs. All difficulties share the
    /// stage's enemy database.
    fn exclusive_row<'a>(
        &self,
        arena: &Arena,
        boss_pack: usize,
        row: u32,
        data: &impl Fn(usize) -> &'a [u8],
    ) -> Result<bool> {
        let rewards = self.rewards.get(&arena.entry);
        for &pack in &self.stages[&arena.entry].gens {
            for g in generators(&Pack::parse(data(pack))?)? {
                if pack == boss_pack && arena.generators.contains(&g.name.as_str()) {
                    continue;
                }
                let current = rewards
                    .and_then(|r| r.targets.get(&pack))
                    .and_then(|t| t.get(&g.name))
                    .or_else(|| self.plans.get(&pack).and_then(|p| p.rows.get(&g.name)))
                    .copied()
                    .unwrap_or(g.row);
                if current == row {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn free_rows<'a>(&self, entry: usize, data: &impl Fn(usize) -> &'a [u8]) -> Result<Vec<usize>> {
        let used = self.stages[&entry]
            .gens
            .iter()
            .map(|&p| generators(&Pack::parse(data(p))?))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .map(|g| g.row as usize)
            .collect::<BTreeSet<_>>();
        Ok((0..self.table.len())
            .filter(|r| self.table.hp[*r] == 0 && !used.contains(r))
            .collect())
    }

    pub(in crate::crown) fn randomise_bosses(
        &mut self,
        iso: &[u8],
        afs: &[u8],
        entries: &[(usize, usize)],
        edits: &mut [Vec<Edit>],
        crown_mod: bool,
        progress: &mut dyn FnMut(crate::writer::Phase, u64, u64),
    ) -> Result<()> {
        let count = sites().count();
        // Each difficulty checks donors for, then applies, every encounter.
        let steps = 3 * 2 * count as u64;
        let mut step = 0;
        let phase = |step: u64| crate::writer::Phase::RandomisingBosses {
            difficulty: (step * 3 / steps).min(2) as u8,
        };
        progress(phase(0), 0, steps);
        let mut tick = || {
            step += 1;
            progress(phase(step), step, steps);
        };
        let message_bases = intro::message_bases(iso, afs, entries)?;
        let data = |entry: usize| {
            let (offset, size) = entries[entry];
            &afs[offset..offset + size]
        };
        let mut files = BTreeMap::new();
        for arena in sites() {
            let stage = self
                .stages
                .get(&arena.entry)
                .ok_or_else(|| bad("missing boss stage"))?;
            need(stage.gens.len() == 3, "missing boss difficulty packs")?;
            let normal = materialize(data(arena.entry), &edits[arena.entry])?;
            let model_extra = self
                .extra
                .get(&arena.entry)
                .cloned()
                .unwrap_or([normal.clone(), normal.clone()]);
            let normal_sound = self
                .sounds
                .get(&(arena.entry - 1))
                .cloned()
                .unwrap_or_else(|| data(arena.entry - 1).to_vec());
            let sound_extra = self
                .extra
                .get(&(arena.entry - 1))
                .cloned()
                .unwrap_or([normal_sound.clone(), normal_sound.clone()]);
            need(
                files
                    .insert(
                        arena.entry,
                        Files {
                            models: vec![normal, model_extra[0].clone(), model_extra[1].clone()],
                            sounds: vec![
                                normal_sound,
                                sound_extra[0].clone(),
                                sound_extra[1].clone(),
                            ],
                            packs: [stage.gens[1], stage.gens[0], stage.gens[2]],
                        },
                    )
                    .is_none(),
                "duplicate boss stage",
            )?;
        }
        let mut report = Vec::new();
        for difficulty in 0..3 {
            let mut slots = Vec::new();
            let mut rejected = Vec::new();
            for arena in sites() {
                let f = &files[&arena.entry];
                let pack = f.packs[difficulty];
                let native_generators = generators(&Pack::parse(data(pack))?)?;
                let mut most = 0;
                let mut exclusive = true;
                for name in arena.generators {
                    let boss = native_generators
                        .iter()
                        .find(|g| g.name == *name)
                        .ok_or_else(|| bad("missing boss generator"))?;
                    need(
                        family(boss.row) == Some(arena.model),
                        "boss identity differs",
                    )?;
                    exclusive &= self.exclusive_row(arena, pack, boss.row, &data)?;
                    let live = boss.most as usize;
                    most = if arena.alternatives {
                        most.max(live)
                    } else {
                        most + live
                    };
                }
                let mut patrols = BTreeSet::new();
                for &floor in arena.floors {
                    let stage = self
                        .stages
                        .get(&floor)
                        .ok_or_else(|| bad("missing boss patrol stage"))?;
                    need(stage.gens.len() == 3, "missing patrol difficulty packs")?;
                    let patrol_pack = stage.gens[[1, 0, 2][difficulty]];
                    for g in generators(&Pack::parse(data(patrol_pack))?)? {
                        if floor == arena.entry && arena.generators.contains(&g.name.as_str()) {
                            continue;
                        }
                        if let Some(model) = family(
                            self.plans
                                .get(&patrol_pack)
                                .and_then(|p| p.rows.get(&g.name))
                                .copied()
                                .unwrap_or(g.row),
                        ) {
                            patrols.insert(model.to_string());
                        }
                    }
                }
                let mut compatible = BTreeSet::new();
                // Every replaced generator needs a local stat row: an unfilled
                // row, or else its own row when no other generator uses it.
                let rows =
                    exclusive || self.spare_rows(arena.entry, &data)? >= arena.generators.len();
                for donor in ARENAS {
                    if donor.model == arena.model || patrols.contains(donor.model) {
                        continue;
                    }
                    let fit = if !rows {
                        Err(bad("no local boss stat rows"))
                    } else {
                        donor_source(donor.model, &data).and_then(|source| {
                            prepare_model(&f.models[difficulty], &source, arena, donor.model, most)
                        })
                    };
                    match fit {
                        Ok(_) => { compatible.insert(donor.model.to_string()); }
                        Err(error) => rejected.push(json!({"stage": arena.entry, "donor": donor.model, "reason": error.to_string()})),
                    }
                }
                slots.push(super::boss_plan::Slot {
                    id: arena.entry.to_string(),
                    original: arena.model.to_string(),
                    patrols,
                    compatible,
                });
                tick();
            }
            let choices = super::boss_plan::assign(
                &slots,
                &mut Rng::new(self.opts.seed, 0xB055_0000 + difficulty as u64),
            )?;
            let mut bosses = Vec::new();
            for (arena, slot) in sites().zip(&slots) {
                let to = &choices[&slot.id];
                let f = files.get_mut(&arena.entry).unwrap();
                let pack = f.packs[difficulty];
                let native = generators(&Pack::parse(data(pack))?)?;
                let originals = arena
                    .generators
                    .iter()
                    .map(|name| native.iter().find(|g| g.name == *name).unwrap())
                    .collect::<Vec<_>>();
                let most = originals.iter().fold(0, |most, g| {
                    if arena.alternatives {
                        most.max(g.most as usize)
                    } else {
                        most + g.most as usize
                    }
                });
                let changed = to != arena.model;
                if changed {
                    need(
                        !slot.patrols.contains(to),
                        "boss duplicates a selected patrol",
                    )?;
                    let donor = source(to)?;
                    if !self.rewards.contains_key(&arena.entry) {
                        let free = self.free_rows(arena.entry, &data)?;
                        let all_packs = self.stages[&arena.entry].gens.clone();
                        self.rewards.insert(
                            arena.entry,
                            super::super::rewards::StageRewards::new(all_packs, free),
                        );
                    }
                    let mut exclusive = BTreeMap::new();
                    for original in &originals {
                        exclusive.insert(
                            original.row,
                            self.exclusive_row(arena, pack, original.row, &data)?,
                        );
                    }
                    let rewards = self.rewards.get_mut(&arena.entry).unwrap();
                    let mut rows = BTreeMap::new();
                    let mut combats = BTreeMap::new();
                    for original in &originals {
                        let combat = nearest_row(&self.table, original.row, to)
                            .ok_or_else(|| bad("missing boss rarity"))?;
                        let row = match rewards.assign(combat as usize, original.row as usize) {
                            Some(row) => row,
                            None if exclusive[&original.row]
                                && rewards.rebind(combat as usize, original.row as usize) =>
                            {
                                original.row as usize
                            }
                            None => return Err(bad("no local boss stat row")),
                        };
                        rows.insert(original.name.clone(), row as u32);
                        combats.insert(original.name.clone(), combat);
                    }
                    rewards
                        .targets
                        .entry(pack)
                        .or_default()
                        .extend(rows.clone());
                    if !self.opts.picks_only {
                        let current = materialize(data(pack), &edits[pack])?;
                        let rt = Retarget {
                            rows: rows.clone(),
                            ..Default::default()
                        };
                        let (generator_bytes, retargeted) = graft::retarget_rows(&current, &rt)?;
                        need(retargeted == rows.len(), "boss retarget count differs")?;
                        let before = Pack::parse(&current)?;
                        let mut expected = current.clone();
                        for (name, row) in &rows {
                            let field = before
                                .rec(before.named("beGeneraterData", name)?)?
                                .field(graft::GENERATOR_ROW)?;
                            expected[field].copy_from_slice(&row.to_le_bytes());
                        }
                        need(
                            generator_bytes == expected,
                            "boss retarget changed native callbacks or counts",
                        )?;
                        edits[pack] = vec![Edit {
                            range: 0..entries[pack].1,
                            bytes: generator_bytes,
                        }];
                    }
                    self.plans.entry(pack).or_default().rows.extend(combats);
                    // Model preparation never alters the localized stat table;
                    // this shared reward plan applies vanilla encounter stats once.
                    if !self.opts.picks_only {
                        let mut model = prepare_model(
                            &f.models[difficulty],
                            &donor_source(to, &data)?,
                            arena,
                            to,
                            most,
                        )?;
                        if arena.card() {
                            model =
                                intro::retarget(&model, arena, to, message_bases[&arena.entry])?;
                        }
                        let companions = graft::companion_models(
                            &Pack::parse(data(donor.entry))?,
                            None,
                            std::slice::from_ref(to),
                        )?;
                        let sounds = super::super::sound::graft_enemy(
                            &f.sounds[difficulty],
                            data(donor.entry - 1),
                            data(donor.entry - 1),
                            to,
                            &companions,
                        )?;
                        let p = Pack::parse(&model)?;
                        need(
                            graft::find_named(&p, "beModelCtrlInfo", to)?.is_some(),
                            "replacement boss missing",
                        )?;
                        model.shrink_to_fit();
                        f.models[difficulty] = model;
                        f.sounds[difficulty] = sounds;
                    }
                }
                self.stats.trace.push(format!(
                    "boss {difficulty} {} {} {to}",
                    arena.entry,
                    arena.generators.join(",")
                ));
                eprintln!(
                    "BOSS IMAGE tier {difficulty} {}: {} -> {to}",
                    arena.entry, arena.model
                );
                bosses.push(json!({"stage": arena.entry, "area": arena.name, "generators": arena.generators,
                    "original": arena.model, "boss": to, "changed": changed, "patrols": slot.patrols, "patrolFloors": arena.floors}));
                tick();
            }
            let expected = slots
                .iter()
                .map(|s| s.original.clone())
                .collect::<BTreeSet<_>>();
            need(
                choices.values().cloned().collect::<BTreeSet<_>>() == expected,
                "boss counts differ",
            )?;
            report.push(json!({"difficulty": (["Normal", "Hard", "Very Hard"][difficulty]), "bosses": bosses, "rejected": rejected}));
        }
        let changed = report
            .iter()
            .flat_map(|r| r["bosses"].as_array().unwrap())
            .filter(|b| b["changed"] == true)
            .count();
        for (entry, mut f) in files {
            let very_hard_model = f.models.pop().unwrap();
            let hard_model = f.models.pop().unwrap();
            let normal_model = f.models.pop().unwrap();
            edits[entry] = vec![Edit {
                range: 0..entries[entry].1,
                bytes: normal_model,
            }];
            self.extra.insert(entry, [hard_model, very_hard_model]);
            let very_hard_sound = f.sounds.pop().unwrap();
            let hard_sound = f.sounds.pop().unwrap();
            self.sounds.insert(entry - 1, f.sounds.pop().unwrap());
            self.extra.insert(entry - 1, [hard_sound, very_hard_sound]);
        }
        let evidence = json!({"prototypeVersion": 4, "bossIntroText": true,
            "introTextMode": "localizedUnicodeIndexes", "arenaVictoryAliases": true,
            "arenaHealthDisplay": true, "seed": self.opts.seed, "crownMod": crown_mod,
            "scope": "dungeon boss arenas and repeat overworld and quest boss encounters; world bosses remain native",
            "worldBossesFixed": true, "tiers": report});
        self.boss_report = Some(evidence);
        self.boss_summary = Some(json!({
            "encounters": count, "changed": changed,
            "worldBossesFixed": true, "preserveEncounterCounts": true,
            "excludeSelectedPatrolFamilies": true, "bossIntroText": true,
        }));
        Ok(())
    }
}

/// Carry the donor's combat callbacks, but
/// replace its arena-specific victory callback with the destination's callback.
pub(super) fn graft_model(
    destination: &[u8],
    source: &[u8],
    from: &str,
    to: &str,
) -> Result<Vec<u8>> {
    let own = Pack::parse(destination)?;
    let original_death = script(&own, &format!("ded_{from}.lua"))?;
    let names = BTreeMap::from([
        (from.to_string(), to.to_string()),
        (format!("{from}/I<0>"), format!("{to}/I<0>")),
    ]);
    let death =
        lua_names::rename(original_death, &names)?.unwrap_or_else(|| original_death.to_vec());
    let source_death = format!("ded_{to}.lua");
    let edits = lua_names::rewrite_in_pack(source, std::slice::from_ref(&source_death), |_, _| {
        Ok(death.clone())
    })?;
    let prepared = materialize(source, &edits)?;
    let prefixes = graft::MODEL_SCRIPTS
        .iter()
        .copied()
        .chain(["ded_", "wht_"])
        .collect::<Vec<_>>();
    let profile = graft::profile_with_scripts(&Pack::parse(&prepared)?, to, &prefixes)?;
    if let Some(reason) = profile.misfit(&own) {
        return Err(bad(&reason));
    }
    let missing = profile.missing(&own);
    let base = if missing.is_empty() {
        destination.to_vec()
    } else {
        add_classes(destination, &missing)?
    };
    let (edits, report) = graft::graft_with_scripts(&base, &prepared, &[to.to_string()], &prefixes)
        .map_err(|e| bad(&format!("boss resource graft: {e}")))?;
    need(report.unattached.is_empty(), "boss effects cannot be drawn")?;
    let mut output = materialize(&base, &edits)?;
    // Retarget only scripts that actually reference the original boss. Include
    // the compound HP-meter target, which ordinary whole-model renames miss.
    let scripts = (0..own.ids.len() as u32)
        .filter(|&id| own.class(id, "beLuaData").is_ok())
        .filter_map(|id| {
            let name = graft::name_of(&own, own.rec(id).ok()?)?;
            let name = std::str::from_utf8(name).ok()?;
            let constants = lua_names::constants(script(&own, name).ok()?).ok()?;
            constants
                .iter()
                .any(|c| names.keys().any(|key| key.as_bytes() == c.text))
                .then_some(name.to_string())
        })
        .collect::<Vec<_>>();
    let edits = lua_names::rewrite_in_pack(&output, &scripts, |_, raw| {
        lua_names::rename(raw, &names)?.ok_or_else(|| bad("missing boss reference"))
    })
    .map_err(|e| bad(&format!("boss stage script rewrite: {e}")))?;
    output = materialize(&output, &edits)?;
    let result = Pack::parse(&output)?;
    // The retained destination bytecode includes its original door IDs, flags,
    // player placement, camera and rewards; only the target model changes.
    need(
        script(&result, &source_death)? == death,
        "boss victory callback differs from destination",
    )?;
    for prefix in ["hit_", "bck_", "wht_"] {
        let name = format!("{prefix}{to}.lua");
        let donor = Pack::parse(source)?;
        if graft::find_named(&donor, "beLuaData", &name)?.is_some() {
            need(
                script(&result, &name)? == script(&donor, &name)?,
                "donor combat callback changed",
            )?;
        }
    }
    Ok(output)
}
