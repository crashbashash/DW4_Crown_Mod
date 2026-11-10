//! Optional dungeon boss shuffle, applied after the final patrol selections.
use super::*;
use serde_json::json;

#[path = "boss_intro.rs"]
pub(in crate::crown) mod intro;

pub(in crate::crown) struct Arena {
    pub(in crate::crown) entry: usize,
    pub(super) model: &'static str,
    pub(super) generator: &'static str,
    pub(super) intro: &'static str,
    pub(super) floors: &'static [usize],
    pub(super) name: &'static str,
}

pub(in crate::crown) const ARENAS: &[Arena] = &[
    Arena {
        entry: 187,
        floors: &[179, 187],
        model: "e_blosso",
        generator: "GENE-08",
        intro: "matg08.lua",
        name: "Humid Cave B2",
    },
    Arena {
        entry: 210,
        floors: &[195, 203, 210],
        model: "e_mammos",
        generator: "GENE-12",
        intro: "matg11.lua",
        name: "Cliff Dungeon B3",
    },
    Arena {
        entry: 241,
        floors: &[233, 241],
        model: "e_skulgrey",
        generator: "GENE-13",
        intro: "matg12.lua",
        name: "Sand Labyrinth B2",
    },
    Arena {
        entry: 265,
        floors: &[249, 257, 265],
        model: "e_anomalo",
        generator: "GENE-15",
        intro: "matg15.lua",
        name: "Ancient Ruins B3",
    },
    Arena {
        entry: 296,
        floors: &[288, 296],
        model: "e_tonogeko",
        generator: "GENE-11",
        intro: "matg10.lua",
        name: "Geko Path B2",
    },
    Arena {
        entry: 319,
        floors: &[304, 312, 319],
        model: "e_mecha4",
        generator: "GENE-19",
        intro: "matg19.lua",
        name: "Vine Tunnel B3",
    },
    Arena {
        entry: 349,
        floors: &[327, 335, 342, 349],
        model: "e_diabolo",
        generator: "GENE-24",
        intro: "matg24.lua",
        name: "Vein B4",
    },
    Arena {
        entry: 394,
        floors: &[379, 387, 394],
        model: "e_mecha5",
        generator: "GENE-23",
        intro: "matg23.lua",
        name: "Mecha Nest B3",
    },
    Arena {
        entry: 425,
        floors: &[402, 410, 417, 425],
        model: "e_ldknight",
        generator: "GENE-21",
        intro: "matg21.lua",
        name: "Electro Mine B4",
    },
];

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
    let mut changes = BTreeMap::new();
    // Blossomon's SQ_NENTR configures normal-enemy death labels, while
    // SQ_ENTR selects its boss callback. Keep the reviewed fallback pool, but
    // prefer the boss activation when the donor provides it.
    let activation = if sequences.contains("SQ_ENTR") {
        "SQ_ENTR"
    } else {
        "SQ_NENTR"
    };
    if arena.entry == 210 && !sequences.contains("SQ_WALKIN") {
        changes.insert("SQ_WALKIN".into(), activation.into());
    }
    if arena.entry == 296 && !sequences.contains("SQ_SWON") {
        changes.insert("SQ_SWON".into(), activation.into());
    }
    if !changes.is_empty() {
        need(
            sequences.contains("SQ_NENTR"),
            "boss lacks a portable activation",
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
    pub(in crate::crown) fn randomise_bosses(
        &mut self,
        iso: &[u8],
        afs: &[u8],
        entries: &[(usize, usize)],
        edits: &mut [Vec<Edit>],
        crown_mod: bool,
        progress: &mut dyn FnMut(crate::writer::Phase, u64, u64),
    ) -> Result<()> {
        // Each difficulty checks donors for, then applies, every arena.
        let steps = 3 * 2 * ARENAS.len() as u64;
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
        for arena in ARENAS {
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
            files.insert(
                arena.entry,
                Files {
                    models: vec![normal, model_extra[0].clone(), model_extra[1].clone()],
                    sounds: vec![normal_sound, sound_extra[0].clone(), sound_extra[1].clone()],
                    packs: [stage.gens[1], stage.gens[0], stage.gens[2]],
                },
            );
        }
        let mut report = Vec::new();
        for difficulty in 0..3 {
            let mut slots = Vec::new();
            let mut rejected = Vec::new();
            for arena in ARENAS {
                let f = &files[&arena.entry];
                let pack = f.packs[difficulty];
                let native_generators = generators(&Pack::parse(data(pack))?)?;
                let boss = native_generators
                    .iter()
                    .find(|g| g.name == arena.generator)
                    .ok_or_else(|| bad("missing boss generator"))?;
                need(
                    family(boss.row) == Some(arena.model),
                    "boss identity differs",
                )?;
                let mut patrols = BTreeSet::new();
                for &floor in arena.floors {
                    let stage = self
                        .stages
                        .get(&floor)
                        .ok_or_else(|| bad("missing dungeon patrol stage"))?;
                    need(stage.gens.len() == 3, "missing dungeon difficulty packs")?;
                    let patrol_pack = stage.gens[[1, 0, 2][difficulty]];
                    for g in generators(&Pack::parse(data(patrol_pack))?)? {
                        if floor == arena.entry && g.name == arena.generator {
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
                for donor in ARENAS {
                    if donor.model == arena.model || patrols.contains(donor.model) {
                        continue;
                    }
                    match prepare_model(&f.models[difficulty], data(donor.entry), arena, donor.model, boss.most as usize) {
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
            for (arena, slot) in ARENAS.iter().zip(&slots) {
                let to = &choices[&slot.id];
                let f = files.get_mut(&arena.entry).unwrap();
                let pack = f.packs[difficulty];
                let original = generators(&Pack::parse(data(pack))?)?
                    .into_iter()
                    .find(|g| g.name == arena.generator)
                    .unwrap();
                let changed = to != arena.model;
                if changed {
                    need(
                        !slot.patrols.contains(to),
                        "boss duplicates a selected patrol",
                    )?;
                    let donor = ARENAS.iter().find(|d| d.model == to).unwrap();
                    let all_packs = self.stages[&arena.entry].gens.clone();
                    let used = all_packs
                        .iter()
                        .map(|&p| generators(&Pack::parse(data(p))?))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .flatten()
                        .map(|g| g.row as usize)
                        .collect::<BTreeSet<_>>();
                    let free = (0..self.table.len())
                        .filter(|r| self.table.hp[*r] == 0 && !used.contains(r))
                        .collect();
                    let rewards = self.rewards.entry(arena.entry).or_insert_with(|| {
                        super::super::rewards::StageRewards::new(all_packs, free)
                    });
                    let combat = nearest_row(&self.table, original.row, to)
                        .ok_or_else(|| bad("missing boss rarity"))?;
                    let row = rewards
                        .assign(combat as usize, original.row as usize)
                        .ok_or_else(|| bad("no local boss stat row"))?;
                    rewards
                        .targets
                        .entry(pack)
                        .or_default()
                        .insert(arena.generator.to_string(), row as u32);
                    if !self.opts.picks_only {
                        let current = materialize(data(pack), &edits[pack])?;
                        let rt = Retarget {
                            rows: BTreeMap::from([(arena.generator.to_string(), row as u32)]),
                            ..Default::default()
                        };
                        let (generator_bytes, count) = graft::retarget_rows(&current, &rt)?;
                        need(count == 1, "boss retarget count differs")?;
                        let before = Pack::parse(&current)?;
                        let mut expected = current.clone();
                        let field = before
                            .rec(before.named("beGeneraterData", arena.generator)?)?
                            .field(graft::GENERATOR_ROW)?;
                        expected[field].copy_from_slice(&(row as u32).to_le_bytes());
                        need(
                            generator_bytes == expected,
                            "boss retarget changed native callbacks or counts",
                        )?;
                        edits[pack] = vec![Edit {
                            range: 0..entries[pack].1,
                            bytes: generator_bytes,
                        }];
                    }
                    self.plans
                        .entry(pack)
                        .or_default()
                        .rows
                        .insert(arena.generator.to_string(), combat);
                    // Model preparation never alters the localized stat table;
                    // this shared reward plan applies vanilla encounter stats once.
                    if !self.opts.picks_only {
                        let mut model = prepare_model(
                            &f.models[difficulty],
                            data(donor.entry),
                            arena,
                            to,
                            original.most as usize,
                        )?;
                        model = intro::retarget(&model, arena, to, message_bases[&arena.entry])?;
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
                    arena.entry, arena.generator
                ));
                eprintln!(
                    "BOSS IMAGE tier {difficulty} {}: {} -> {to}",
                    arena.entry, arena.model
                );
                bosses.push(json!({"stage": arena.entry, "area": arena.name, "generator": arena.generator,
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
        let evidence = json!({"prototypeVersion": 3, "bossIntroText": true,
            "introTextMode": "localizedUnicodeIndexes", "arenaVictoryAliases": true,
            "arenaHealthDisplay": true, "seed": self.opts.seed, "crownMod": crown_mod,
            "scope": "nine dungeon boss encounters; extra overworld and quest bosses remain native",
            "worldBossesFixed": true, "tiers": report});
        self.boss_report = Some(evidence);
        self.boss_summary = Some(json!({
            "encounters": ARENAS.len(), "changed": changed,
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
