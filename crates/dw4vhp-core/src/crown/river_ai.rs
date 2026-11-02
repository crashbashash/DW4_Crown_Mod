//! Restrict voluntary riverbank attacks in the destination pack. Other stages,
//! projectile definitions, activation, damage responses and death stay native.
use super::*;
use std::collections::BTreeSet;

pub(super) const ENTRY: usize = 628;
// MR05's native 30-unit firing band is narrower than the boat/shore gap.
// Keep firing/active mode out to 80, with a 120-unit outer band, while leaving
// the native 190-unit perception limit and turn settings intact.
const MR05_NEAR_SQUARED: f32 = 80.0 * 80.0;
const MR05_FAR_SQUARED: f32 = 120.0 * 120.0;

#[derive(Clone, Copy)]
pub(super) struct Policy {
    pub model: &'static str,
    pub fire: &'static str,
    pub allowed: &'static [&'static str],
}

pub(super) const POLICIES: &[Policy] = &[
    Policy {
        model: "e_nume",
        fire: "SQ_FIRE",
        allowed: &["SQ_FIRE"],
    },
    Policy {
        model: "e_scum",
        fire: "SQ_FIRE",
        allowed: &["SQ_FIRE"],
    },
    Policy {
        model: "e_minotaru",
        fire: "SQ_ATCK2",
        allowed: &["SQ_ATCK2", "SQ_FIRE", "SQ_FIRE1"],
    },
    Policy {
        model: "e_mecha2",
        fire: "SQ_FIRE",
        allowed: &["SQ_FIRE"],
    },
    Policy {
        model: "e_otama",
        fire: "SQ_ATCK1",
        allowed: &["SQ_ATCK1"],
    },
    Policy {
        model: "e_mecha3",
        fire: "SQ_ATCK1",
        allowed: &["SQ_ATCK1"],
    },
    Policy {
        model: "e_mecha4",
        fire: "SQ_FIRE",
        allowed: &["SQ_FIRE"],
    },
    Policy {
        model: "e_maridevi",
        fire: "SQ_MAGIC",
        allowed: &["SQ_MAGIC"],
    },
    Policy {
        model: "e_megadra",
        fire: "SQ_MAGIC",
        allowed: &["SQ_MAGIC"],
    },
    Policy {
        model: "e_infermo",
        fire: "SQ_ATCK4",
        allowed: &["SQ_ATCK4"],
    },
    Policy {
        model: "e_rare",
        fire: "SQ_FIRE1",
        allowed: &["SQ_FIRE", "SQ_FIRE1"],
    },
    Policy {
        model: "e_mecha5",
        fire: "SQ_FIRE",
        allowed: &["SQ_FIRE"],
    },
    Policy {
        model: "e_impdra",
        fire: "SQ_FIRE",
        allowed: &["SQ_FIRE"],
    },
];

impl Policy {
    fn restricted(self, name: &str) -> bool {
        (name.starts_with("SQ_ATCK")
            || name.starts_with("SQ_FIRE")
            || name == "SQ_MAGIC"
            || (self.model == "e_infermo" && matches!(name, "SQ_PART" | "SQ_BINDCK")))
            && !self.allowed.contains(&name)
    }
}

fn name<'a>(p: &'a Pack<'_>, id: u32) -> Result<&'a str> {
    std::str::from_utf8(
        graft::name_of(p, p.rec(id)?).ok_or_else(|| bad("unnamed river AI record"))?,
    )
    .map_err(|_| bad("invalid river AI name"))
}

fn add(
    p: &Pack<'_>,
    edits: &mut BTreeMap<usize, Edit>,
    range: Range<usize>,
    bytes: Vec<u8>,
) -> Result<()> {
    need(
        range.len() == bytes.len(),
        "river AI edit changes record size",
    )?;
    if p.b[range.clone()] == bytes {
        return Ok(());
    }
    if let Some(old) = edits.get(&range.start) {
        need(
            old.range == range && old.bytes == bytes,
            "conflicting shared river AI policy",
        )?;
    } else {
        edits.insert(range.start, Edit { range, bytes });
    }
    Ok(())
}

/// No donor source is edited. This runs only on the final randomized river pack,
/// including a retained Numemon fallback, after imports and pruning finish.
pub(super) fn apply(bytes: &[u8], entry: usize) -> Result<Vec<u8>> {
    if entry != ENTRY {
        return Ok(bytes.to_vec());
    }
    let p = Pack::parse(bytes)?;
    let mut edits = BTreeMap::new();
    for &policy in POLICIES {
        let Some(info) = graft::find_named(&p, "beModelCtrlInfo", policy.model)? else {
            continue;
        };
        let mut sequences = BTreeMap::new();
        let mut decisions = BTreeSet::new();
        for data in p.array(p.val(info, 5)?)? {
            match p.classes[p.rec(data)?.class].name.as_str() {
                "beModelCtrlInfoDataSeq" => {
                    for seq in p.array(p.val(data, 4)?)? {
                        sequences.insert(name(&p, seq)?.to_string(), seq);
                    }
                }
                "beModelCtrlInfoDataAIM" => {
                    if policy.model == "e_mecha5" {
                        for map in p.array(p.val(data, 4)?)? {
                            p.class(map, "beModelCtrlAIMap")?;
                            let near = f32::from_bits(p.val(map, 3)?).max(MR05_NEAR_SQUARED);
                            let far = f32::from_bits(p.val(map, 4)?).max(MR05_FAR_SQUARED);
                            let limit = f32::from_bits(p.val(map, 5)?);
                            need(
                                near.is_finite()
                                    && far.is_finite()
                                    && limit.is_finite()
                                    && near <= far
                                    && far <= limit,
                                "invalid river MR05 range bands",
                            )?;
                            add(
                                &p,
                                &mut edits,
                                p.rec(map)?.field(3)?,
                                near.to_bits().to_le_bytes().to_vec(),
                            )?;
                            add(
                                &p,
                                &mut edits,
                                p.rec(map)?.field(4)?,
                                far.to_bits().to_le_bytes().to_vec(),
                            )?;
                        }
                    }
                    for id in graft::closure(&p, &[data])?.0 {
                        if p.class(id, "beModelCtrlAIData").is_ok() {
                            decisions.insert(id);
                        }
                    }
                }
                _ => {}
            }
        }
        let fire = *sequences
            .get(policy.fire)
            .ok_or_else(|| bad("river enemy lacks verified firing sequence"))?;
        let commands = p.val(fire, 3)?;
        let labels = |list| -> Result<BTreeSet<String>> {
            p.array(list)?
                .into_iter()
                .filter(|&id| id != NULL && p.class(id, "beModelCtrlLABEL").is_ok())
                .map(|id| Ok(name(&p, id)?.to_string()))
                .collect()
        };
        let fire_labels = labels(commands)?;
        for id in decisions {
            let old = name(&p, id)?;
            if !policy.restricted(old) {
                continue;
            }
            // Infermon's bind/part checks are not firing decisions. Idle instead
            // of invoking their summon/transform callback on the riverbank.
            let replacement = if matches!(old, "SQ_PART" | "SQ_BINDCK") {
                "SQ_WAIT"
            } else {
                policy.fire
            };
            let f = p.rec(id)?.field(2)?;
            need(
                replacement.len() < f.len() - 4,
                "river firing name exceeds native field",
            )?;
            let mut raw = vec![0; f.len() - 4];
            raw[..replacement.len()].copy_from_slice(replacement.as_bytes());
            add(&p, &mut edits, f.start + 4..f.end, raw)?;
        }
        // Guard the original offensive entry points too: a callback or nested
        // transition cannot bypass the decision table and play a melee action.
        // Sharing the native firing command list retains animation/event timing.
        for (seq, id) in sequences {
            if !policy.restricted(&seq) {
                continue;
            }
            need(
                labels(p.val(id, 3)?)?.is_subset(&fire_labels),
                "river firing redirect loses a native entry label",
            )?;
            add(
                &p,
                &mut edits,
                p.rec(id)?.field(3)?,
                commands.to_le_bytes().to_vec(),
            )?;
        }
    }
    materialize(bytes, &edits.into_values().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moving_projectile(p: &Pack<'_>, model: &str, action: &str) -> bool {
        let root = u32at(p.b, p.table + 8 + 2 * p.ids.len()).unwrap();
        for starter in p.array(root).unwrap() {
            if p.class(starter, "beActionStarterInfo").is_err() {
                continue;
            }
            let Some(projectile) =
                graft::find_named(p, "beModelCtrlInfo", name(p, starter).unwrap()).unwrap()
            else {
                continue;
            };
            for target in p.array(p.val(starter, 7).unwrap()).unwrap() {
                if !p.rec(target).unwrap().named(p.b, model) {
                    continue;
                }
                for binding in p.array(p.val(target, 3).unwrap()).unwrap() {
                    if !p.rec(binding).unwrap().named(p.b, action)
                        || p.val(binding, 6).unwrap() != 0
                    {
                        continue;
                    }
                    let f = p.rec(binding).unwrap().field(3).unwrap();
                    let seq = std::str::from_utf8(
                        p.b[f.start + 4..f.end].split(|b| *b == 0).next().unwrap(),
                    )
                    .unwrap();
                    for data in p.array(p.val(projectile, 5).unwrap()).unwrap() {
                        if p.class(data, "beModelCtrlInfoDataSeq").is_err() {
                            continue;
                        }
                        for id in p.array(p.val(data, 4).unwrap()).unwrap() {
                            if !p.rec(id).unwrap().named(p.b, seq) {
                                continue;
                            }
                            let commands = p.array(p.val(id, 3).unwrap()).unwrap();
                            if commands.iter().any(|&c| {
                                p.class(c, "beModelCtrlMOVE").is_ok()
                                    && (p.rec(c).unwrap().named(p.b, "Move")
                                        || p.rec(c).unwrap().named(p.b, "Push"))
                            }) && commands.iter().any(|&c| {
                                p.class(c, "beModelCtrlHITBOX").is_ok() && p.val(c, 3).unwrap() != 0
                            }) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    #[test]
    fn policy_matches_river_pool_and_leaves_other_stages_untouched() {
        assert_eq!(
            POLICIES.iter().map(|p| p.model).collect::<Vec<_>>(),
            encounters::RIVER_RANGED
        );
        assert_eq!(
            apply(b"not a model pack", 614).unwrap(),
            b"not a model pack"
        );
        assert!(apply(b"not a model pack", ENTRY).is_err());
    }

    #[test]
    fn retail_river_ai_limits_choices_preserves_other_data_and_is_idempotent() {
        for variable in [
            "DW4_RETAIL_USA_ISO",
            "DW4_RETAIL_EUROPE_ISO",
            "DW4_RETAIL_JAPAN_ISO",
            "DW4_RETAIL_GC_USA_ISO",
            "DW4_RETAIL_GC_JAPAN_ISO",
        ] {
            let Some(path) = std::env::var_os(variable) else {
                continue;
            };
            let file = std::fs::File::open(path).unwrap();
            let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let release = crate::release::detect(&iso).unwrap();
            let (base, len) = if release.is_gamecube() {
                let f = gcm::find_file(&iso, "NDMW.AFS").unwrap();
                (f.offset as usize, f.size as usize)
            } else {
                let f = iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (f.lba as usize * 2048, f.size as usize)
            };
            let afs = &iso[base..base + len];
            for (policy, entry) in POLICIES.iter().zip([
                628, 187, 241, 265, 288, 312, 319, 335, 379, 387, 394, 394, 723,
            ]) {
                let o = u32at(afs, 8 + entry * 8).unwrap() as usize;
                let n = u32at(afs, 12 + entry * 8).unwrap() as usize;
                let original = &afs[o..o + n];
                let out = apply(original, ENTRY).unwrap();
                assert_eq!(out.len(), original.len());
                assert_eq!(apply(original, 614).unwrap(), original);
                assert_eq!(apply(&out, ENTRY).unwrap(), out);
                let p = Pack::parse(original).unwrap();
                let q = Pack::parse(&out).unwrap();
                let info = q.named("beModelCtrlInfo", policy.model).unwrap();
                let mut mr05_maps = BTreeSet::new();
                if let Some(mr05) = graft::find_named(&q, "beModelCtrlInfo", "e_mecha5").unwrap() {
                    for data in q.array(q.val(mr05, 5).unwrap()).unwrap() {
                        if q.class(data, "beModelCtrlInfoDataAIM").is_ok() {
                            for map in q.array(q.val(data, 4).unwrap()).unwrap() {
                                mr05_maps.insert(map);
                                assert_eq!(
                                    f32::from_bits(q.val(map, 3).unwrap()),
                                    MR05_NEAR_SQUARED
                                );
                                assert_eq!(
                                    f32::from_bits(q.val(map, 4).unwrap()),
                                    MR05_FAR_SQUARED
                                );
                                assert_eq!(q.val(map, 5).unwrap(), p.val(map, 5).unwrap());
                            }
                        }
                    }
                }

                let mut decision_count = 0;
                for data in q.array(q.val(info, 5).unwrap()).unwrap() {
                    if q.class(data, "beModelCtrlInfoDataAIM").is_ok() {
                        for id in graft::closure(&q, &[data]).unwrap().0 {
                            if q.class(id, "beModelCtrlAIData").is_ok() {
                                let chosen = name(&q, id).unwrap();
                                assert!(
                                    !policy.restricted(chosen),
                                    "{variable} {} {chosen}",
                                    policy.model
                                );
                                decision_count += 1;
                            }
                        }
                    }
                }
                assert!(decision_count > 0);
                for data in q.array(q.val(info, 5).unwrap()).unwrap() {
                    if q.class(data, "beModelCtrlInfoDataSeq").is_err() {
                        continue;
                    }
                    let seqs: BTreeMap<_, _> = q
                        .array(q.val(data, 4).unwrap())
                        .unwrap()
                        .into_iter()
                        .map(|id| (name(&q, id).unwrap().to_string(), id))
                        .collect();
                    let fire = seqs[policy.fire];
                    for (seq, id) in seqs {
                        if policy.restricted(&seq) {
                            assert_eq!(
                                q.val(id, 3).unwrap(),
                                q.val(fire, 3).unwrap(),
                                "{variable} {} callback entry {seq}",
                                policy.model
                            );
                        } else {
                            if policy.allowed.contains(&seq.as_str()) {
                                for command in q.array(q.val(id, 3).unwrap()).unwrap() {
                                    if q.class(command, "beModelCtrlACTION").is_ok() {
                                        let action = name(&q, command).unwrap();
                                        if action.starts_with("NATK")
                                            || action.starts_with("BLST")
                                            || action.starts_with("MAGC")
                                        {
                                            assert!(moving_projectile(&q,policy.model,action),"{variable} {} {seq} {action} must spawn a moving damaging projectile",policy.model);
                                        }
                                    }
                                }
                            }
                            assert_eq!(
                                p.b[p.rec(id).unwrap().range.clone()],
                                q.b[q.rec(id).unwrap().range.clone()],
                                "{variable} {} lifecycle/ranged sequence {seq}",
                                policy.model
                            );
                        }
                    }
                }
                for id in 0..p.ids.len() as u32 {
                    if let Ok(r) = p.rec(id) {
                        let after = q.rec(id).unwrap();
                        for (&field, range) in &r.fields {
                            let allowed = (p.classes[r.class].name == "beModelCtrlAIData"
                                && field == 2)
                                || (p.classes[r.class].name == "beModelCtrlData" && field == 3)
                                || (mr05_maps.contains(&id) && matches!(field, 3 | 4));
                            if !allowed {
                                assert_eq!(
                                    p.b[range.clone()],
                                    q.b[after.field(field).unwrap()],
                                    "{variable} {} object {id} field {field}",
                                    policy.model
                                );
                            }
                        }
                    } else if let Some(range) = p.memory[id as usize].clone() {
                        assert_eq!(p.b[range.clone()], q.b[range]);
                    }
                }
                eprintln!("RIVER AI {variable} {}: {decision_count} decisions checked; lifecycle, projectiles, scripts and source preserved",policy.model);
            }
        }
    }
}
