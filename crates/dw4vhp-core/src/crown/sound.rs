//! Carry an imported enemy's resident sound bank and action-to-sound bindings with it.
use super::graft;
use super::*;
use std::collections::BTreeSet;

fn info(p: &Pack, class: &str) -> Result<u32> {
    let ids: Vec<_> = (0..p.ids.len() as u32)
        .filter(|&id| p.class(id, class).is_ok())
        .collect();
    need(ids.len() == 1, "expected one sound registration")?;
    Ok(ids[0])
}

fn bank_info(p: &Pack, name: &str) -> Result<Option<u32>> {
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beCriSfpData").is_ok()
            && graft::name_of(p, p.rec(id)?)
                .is_some_and(|n| n.eq_ignore_ascii_case(name.as_bytes()))
        {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn append(p: &Pack, b: &mut Builder, list: u32, added: &[u32]) -> Result<()> {
    if added.is_empty() {
        return Ok(());
    }
    let count = p.array(list)?.len() + added.len();
    b.grow_mem(p.val(list, 4)?, words(added))?;
    b.modify(list, &[(2, count as u32), (3, count as u32)])
}

fn required(p: &Pack, model: &str) -> Result<BTreeSet<String>> {
    let binding = graft::find_named(p, "beSeInfoData", model)?
        .ok_or_else(|| bad(&format!("missing sound binding for {model}")))?;
    p.array(p.val(binding, 4)?)?
        .into_iter()
        .map(|id| {
            let f = p.rec(id)?.field(3)?;
            let name = p.b[f.start + 4..f.end]
                .split(|&c| c == 0)
                .next()
                .unwrap_or_default();
            std::str::from_utf8(name)
                .map(str::to_string)
                .map_err(|_| bad("invalid sound selector"))
        })
        .collect()
}

/// Some retail packs retain bindings to selectors supplied only in another
/// stage (MarineDevimon's death sound, for example). Find a complete source for
/// supplementation; never guess a cue number or substitute another sound.
pub(super) fn complete(src: &[u8], model: &str) -> Result<bool> {
    let p = Pack::parse(src)?;
    let names = match required(&p, model) {
        Ok(names) => names,
        Err(_) => return Ok(false),
    };
    let sounds = p.array(p.val(info(&p, "beSoundInfo")?, 5)?)?;
    for name in names {
        let Some(id) = graft::find_named(&p, "beSoundData", &name)? else {
            return Ok(false);
        };
        if !sounds.contains(&id) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn graft_resolved(
    dst: &[u8],
    src: &[u8],
    fallback: &[u8],
    model: &str,
) -> Result<Vec<u8>> {
    let sp = Pack::parse(src)?;
    let needed = required(&sp, model)?;
    let mut out = graft(dst, src, model)?;
    let fp = Pack::parse(fallback)?;
    for name in needed {
        if graft::find_named(&Pack::parse(&out)?, "beSoundData", &name)?.is_some() {
            continue;
        }
        let id = graft::find_named(&fp, "beSoundData", &name)?
            .ok_or_else(|| bad(&format!("{model}: unresolved sound {name}")))?;
        let files = fp.array(fp.val(info(&fp, "beSoundInfo")?, 7)?)?;
        let file = *files
            .get(fp.val(id, 5)? as usize)
            .ok_or_else(|| bad("fallback sound filename index outside list"))?;
        let path =
            graft::name_of(&fp, fp.rec(file)?).ok_or_else(|| bad("unnamed fallback bank"))?;
        let bank = std::str::from_utf8(path.rsplit(|&c| c == b'/').next().unwrap())
            .map_err(|_| bad("invalid fallback bank"))?;
        out = graft_bank(&out, fallback, model, bank)?;
    }
    need(
        complete(&out, model)?,
        "imported sound dependencies remain unresolved",
    )?;
    Ok(out)
}

/// Projectile/effect models can have their own action bindings even when all
/// their selectors use a common bank already present in the destination.
pub(super) fn graft_enemy(
    dst: &[u8],
    src: &[u8],
    fallback: &[u8],
    model: &str,
    companions: &[String],
) -> Result<Vec<u8>> {
    let mut out = graft_resolved(dst, src, fallback, model)?;
    let p = Pack::parse(src)?;
    for companion in companions {
        if graft::find_named(&p, "beSeInfoData", companion)?.is_some()
            && !required(&p, companion)?.is_empty()
        {
            out = graft_resolved(&out, src, fallback, companion)?;
        }
    }
    Ok(out)
}

/// Merge the model's sound banks, selectors and SE action bindings. Filename indices
/// are local to each sound pack and must be translated independently of object IDs.
pub(super) fn graft(dst: &[u8], src: &[u8], model: &str) -> Result<Vec<u8>> {
    let sp = Pack::parse(src)?;
    let Some(binding) = graft::find_named(&sp, "beSeInfoData", model)? else {
        return Ok(dst.to_vec());
    };
    let mut names = BTreeSet::new();
    for action in sp.array(sp.val(binding, 4)?)? {
        let f = sp.rec(action)?.field(3)?;
        let name = sp.b[f.start + 4..f.end]
            .split(|&c| c == 0)
            .next()
            .unwrap_or_default();
        names.insert(name);
    }
    let si = info(&sp, "beSoundInfo")?;
    let files = sp.array(sp.val(si, 7)?)?;
    let mut banks = BTreeSet::new();
    for sound in sp.array(sp.val(si, 5)?)? {
        let r = sp.rec(sound)?;
        if graft::name_of(&sp, r).is_some_and(|n| names.contains(n)) {
            let file = *files
                .get(r.val(sp.b, 5)? as usize)
                .ok_or_else(|| bad("sound filename index outside list"))?;
            let path =
                graft::name_of(&sp, sp.rec(file)?).ok_or_else(|| bad("unnamed sound bank"))?;
            let bank = path.rsplit(|&c| c == b'/').next().unwrap();
            banks.insert(
                std::str::from_utf8(bank)
                    .map_err(|_| bad("invalid bank name"))?
                    .to_string(),
            );
        }
    }
    let mut out = dst.to_vec();
    for bank in banks {
        out = graft_bank(&out, src, model, &bank)?;
    }
    Ok(out)
}

fn graft_bank(dst: &[u8], src: &[u8], model: &str, bank: &str) -> Result<Vec<u8>> {
    let dp = Pack::parse(dst)?;
    let sp = Pack::parse(src)?;
    let bank_id = bank_info(&sp, bank)?;
    let (ds, ss) = (info(&dp, "beSoundInfo")?, info(&sp, "beSoundInfo")?);
    let (de, se) = (info(&dp, "beSeInfo")?, info(&sp, "beSeInfo")?);
    let (df, sf) = (dp.val(ds, 7)?, sp.val(ss, 7)?);
    let dst_files = dp.array(df)?;
    let src_files = sp.array(sf)?;
    let file = src_files
        .iter()
        .position(|&id| {
            graft::name_of(&sp, sp.rec(id).unwrap())
                .is_some_and(|n| n.rsplit(|&c| c == b'/').next() == Some(bank.as_bytes()))
        })
        .ok_or_else(|| bad("sound bank has no filename slot"))?;
    let filename = graft::name_of(&sp, sp.rec(src_files[file])?).unwrap();
    let existing = dst_files.iter().position(|&id| {
        graft::name_of(&dp, dp.rec(id).unwrap()).is_some_and(|n| n.eq_ignore_ascii_case(filename))
    });
    let target_file = existing.unwrap_or(dst_files.len());
    let mut roots = Vec::new();
    let dst_bank = bank_info(&dp, bank)?;
    let copy_bank = bank_id.is_some() && dst_bank.is_none();
    if let (Some(source), Some(target)) = (bank_id, dst_bank) {
        let payload = |p: &Pack, id| -> Result<Range<usize>> {
            p.memory
                .get(p.val(id, 4)? as usize)
                .and_then(Clone::clone)
                .ok_or_else(|| bad("missing embedded sound bank"))
        };
        need(
            sp.b[payload(&sp, source)?] == dp.b[payload(&dp, target)?],
            "conflicting embedded sound banks",
        )?;
    }
    if existing.is_none() {
        roots.push(src_files[file]);
    }
    if copy_bank {
        roots.extend(bank_id);
    }
    let mut sounds = Vec::new();
    let mut bank_sounds = BTreeSet::new();
    let mut patched_src = src.to_vec();
    for id in sp.array(sp.val(ss, 5)?)? {
        let r = sp.rec(id)?;
        if r.val(sp.b, 5)? as usize == file {
            let name = graft::name_of(&sp, r).ok_or_else(|| bad("unnamed sound"))?;
            bank_sounds.insert(name);
            if graft::find_named(
                &dp,
                "beSoundData",
                std::str::from_utf8(name).map_err(|_| bad("invalid sound name"))?,
            )?
            .is_none()
            {
                let f = r.field(5)?;
                need(f.len() == 4, "unexpected sound filename index width")?;
                put(&mut patched_src, f.start, target_file as u32);
                sounds.push(id);
            }
        }
    }
    roots.extend(&sounds);
    let mut bindings = Vec::new();
    for id in sp.array(sp.val(se, 5)?)? {
        let r = sp.rec(id)?;
        let Some(name) = graft::name_of(&sp, r) else {
            continue;
        };
        let uses_bank = sp.array(sp.val(id, 4)?)?.iter().any(|&action| {
            sp.rec(action)
                .and_then(|r| r.field(3))
                .ok()
                .and_then(|f| sp.b.get(f.start + 4..f.end))
                .and_then(|s| s.split(|&c| c == 0).next())
                .is_some_and(|s| bank_sounds.contains(s))
        });
        if (name == model.as_bytes()
            || (uses_bank && !["allse.sfp", "COMMON_BLST.sfp", "common_blst.sfp"].contains(&bank)))
            && graft::find_named(
                &dp,
                "beSeInfoData",
                std::str::from_utf8(name).map_err(|_| bad("invalid SE model name"))?,
            )?
            .is_none()
        {
            bindings.push(id);
        }
    }
    roots.extend(&bindings);
    if roots.is_empty() {
        return Ok(dst.to_vec());
    }
    let sp = Pack::parse(&patched_src)?;
    let (ids, owners) = graft::closure(&sp, &roots)?;
    let used: BTreeSet<_> = ids
        .iter()
        .filter_map(|&id| {
            sp.ids[id as usize].map(|i| sp.classes[sp.objects[i].class].name.as_str())
        })
        .collect();
    let defs = class_defs(src)?
        .into_iter()
        .filter(|c| {
            used.contains(c.class.name.as_str())
                && !dp.classes.iter().any(|d| d.name == c.class.name)
        })
        .collect::<Vec<_>>();
    let dst = if defs.is_empty() {
        dst.to_vec()
    } else {
        add_classes(dst, &defs)?
    };
    let dp = Pack::parse(&dst)?;
    let mut builder = Builder::new(&dp);
    let map = graft::copy_graph(
        &dp,
        &sp,
        &mut builder,
        &ids,
        &owners,
        &mut graft::Report::default(),
    )?;
    if existing.is_none() {
        append(&dp, &mut builder, df, &[map[&src_files[file]]])?;
    }
    append(
        &dp,
        &mut builder,
        dp.val(ds, 5)?,
        &sounds.iter().map(|i| map[i]).collect::<Vec<_>>(),
    )?;
    append(
        &dp,
        &mut builder,
        dp.val(de, 5)?,
        &bindings.iter().map(|i| map[i]).collect::<Vec<_>>(),
    )?;
    let root = u32at(dp.b, dp.table + 8 + 2 * dp.ids.len())?;
    if let Some(bank_id) = bank_id.filter(|_| copy_bank) {
        // The loader discovers bank data through the registered static info.
        // A bank in the root igInfoList is absent from that static-data lookup.
        let static_info = info(&dp, "beStaticInfo")?;
        need(
            dp.array(root)?.contains(&static_info),
            "sound static info is not registered",
        )?;
        append(&dp, &mut builder, dp.val(static_info, 5)?, &[map[&bank_id]])?;
    }
    let grows = builder.grown();
    let edits = builder.finish(dp.val(root, 4)?)?;
    let out = materialize(&dst, &edits)?;
    graft::verify(&dp, &sp, &out, &map, &owners, &grows)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_gekomon_has_song_binding_selector_and_bank() {
        for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
            let Some(path) = std::env::var_os(variable) else {
                continue;
            };
            let iso = std::fs::read(path).unwrap();
            let (base, size) = if crate::gcm::game_id(&iso).is_ok() {
                let file = crate::gcm::find_file(&iso, "NDMW.AFS").unwrap();
                (file.offset as usize, file.size as usize)
            } else {
                let file = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (file.lba as usize * 2048, file.size as usize)
            };
            let afs = &iso[base..base + size];
            let entry = |e: usize| {
                let off = u32at(afs, 8 + e * 8).unwrap() as usize;
                let len = u32at(afs, 12 + e * 8).unwrap() as usize;
                &afs[off..off + len]
            };
            let dst = entry(620);
            let src = entry(678);
            let out = graft(dst, src, "e_geko").unwrap();
            let p = Pack::parse(&out).unwrap();
            let original = Pack::parse(dst).unwrap();
            assert_eq!(&p.ids[..original.ids.len()], &original.ids);
            let si = info(&p, "beSoundInfo").unwrap();
            let sounds = p.array(p.val(si, 5).unwrap()).unwrap();
            let song = graft::find_named(&p, "beSoundData", "geNATK01")
                .unwrap()
                .unwrap();
            assert!(sounds.contains(&song));
            let files = p.array(p.val(si, 7).unwrap()).unwrap();
            let filename = files[p.val(song, 5).unwrap() as usize];
            assert!(graft::name_of(&p, p.rec(filename).unwrap())
                .unwrap()
                .ends_with(b"/e_geko.sfp"));
            let se = info(&p, "beSeInfo").unwrap();
            let binding = graft::find_named(&p, "beSeInfoData", "e_geko")
                .unwrap()
                .unwrap();
            assert!(p.array(p.val(se, 5).unwrap()).unwrap().contains(&binding));
            let actions = p.array(p.val(binding, 4).unwrap()).unwrap();
            assert!(actions.iter().any(|&id| {
                let f = p.rec(id).unwrap().field(3).unwrap();
                p.b[f.start + 4..f.end].starts_with(b"geNATK01\0")
            }));
            for (label, selector) in [
                ("NATK01", "geNATK01"),
                ("DEAD01", "geDEAD01"),
                ("LDMG01", "geLDMG01"),
                ("SDMG01", "geSDMG01"),
                ("MDMG01", "geSDMG01"),
                ("MAGC01", "tgVOICE"),
            ] {
                assert!(
                    actions.iter().any(|&id| {
                        let r = p.rec(id).unwrap();
                        let f = r.field(3).unwrap();
                        graft::name_of(&p, r) == Some(label.as_bytes())
                            && p.b[f.start + 4..f.end]
                                .starts_with(format!("{selector}\0").as_bytes())
                    }),
                    "Gekomon {label} must play {selector}"
                );
                let sound = graft::find_named(&p, "beSoundData", selector)
                    .unwrap()
                    .unwrap();
                assert!(sounds.contains(&sound));
                assert!((p.val(sound, 5).unwrap() as usize) < files.len());
            }
            let bank = graft::find_named(&p, "beCriSfpData", "e_geko.sfp")
                .unwrap()
                .unwrap();
            let root = u32at(p.b, p.table + 8 + 2 * p.ids.len()).unwrap();
            let static_info = info(&p, "beStaticInfo").unwrap();
            assert!(p.array(root).unwrap().contains(&static_info));
            assert!(
                p.array(p.val(static_info, 5).unwrap())
                    .unwrap()
                    .contains(&bank),
                "embedded banks must be registered in beStaticInfo's data list"
            );
            assert!(!p.array(root).unwrap().contains(&bank));
            let source = Pack::parse(src).unwrap();
            let source_bank = graft::find_named(&source, "beCriSfpData", "e_geko.sfp")
                .unwrap()
                .unwrap();
            let data = |p: &Pack, id| p.memory[p.val(id, 4).unwrap() as usize].clone().unwrap();
            assert_eq!(&p.b[data(&p, bank)], &source.b[data(&source, source_bank)]);
            assert_eq!(
                graft(&out, src, "e_geko").unwrap(),
                out,
                "do not duplicate a bank or its bindings"
            );
            // Existing sounds retain their bank indices and payloads.
            for id in original
                .array(
                    original
                        .val(info(&original, "beSoundInfo").unwrap(), 5)
                        .unwrap(),
                )
                .unwrap()
            {
                let before = original.rec(id).unwrap();
                let after = p.rec(id).unwrap();
                for (slot, range) in &before.fields {
                    assert_eq!(
                        &original.b[range.clone()],
                        &p.b[after.field(*slot).unwrap()]
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod audit {
    use super::*;

    #[test]
    fn audit_every_enemy_audio_dependency() {
        for variable in ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_GC_USA_ISO"] {
            let Some(path) = std::env::var_os(variable) else {
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
            let entry = |e: usize| {
                let o = u32at(afs, 8 + 8 * e).unwrap() as usize;
                let l = u32at(afs, 12 + 8 * e).unwrap() as usize;
                &afs[o..o + l]
            };
            let mut sources = BTreeMap::new();
            for e in 1..u32at(afs, 4).unwrap() as usize {
                let b = entry(e);
                if memchr::memmem::find(b, b"KIND_ENEMY\0").is_none() {
                    continue;
                }
                let Ok(p) = Pack::parse(b) else { continue };
                for &model in super::super::random::REGULAR {
                    if !sources.contains_key(model)
                        && graft::find_named(&p, "beModelCtrlInfo", model)
                            .unwrap()
                            .is_some()
                        && complete(entry(e - 1), model).unwrap_or(false)
                    {
                        sources.insert(model, e - 1);
                    }
                }
            }
            assert_eq!(sources.len(), super::super::random::REGULAR.len());
            let mut checked = 0;
            let mut missing = Vec::new();
            for e in 0..u32at(afs, 4).unwrap() as usize {
                let b = entry(e);
                if memchr::memmem::find(b, b"KIND_ENEMY\0").is_none() {
                    continue;
                }
                let Ok(models) = Pack::parse(b) else { continue };
                if e == 0 {
                    continue;
                }
                let Ok(src) = Pack::parse(entry(e - 1)) else {
                    continue;
                };
                if info(&src, "beSoundInfo").is_err() {
                    continue;
                }
                for model in super::super::random::REGULAR {
                    if graft::find_named(&models, "beModelCtrlInfo", model)
                        .unwrap()
                        .is_none()
                    {
                        continue;
                    }
                    let binding = graft::find_named(&src, "beSeInfoData", model).unwrap();
                    let Some(binding) = binding else {
                        missing.push(format!("{e} {model}: no model sound binding"));
                        continue;
                    };
                    let dst_entry = if ["e_goburi", "e_ogre"].contains(model) {
                        678
                    } else {
                        620
                    };
                    let companions =
                        graft::companion_models(&models, None, &[model.to_string()]).unwrap();
                    let out = graft_enemy(
                        entry(dst_entry),
                        entry(e - 1),
                        entry(sources[model]),
                        model,
                        &companions,
                    )
                    .unwrap();
                    let fallback = Pack::parse(entry(sources[model])).unwrap();
                    let original = Pack::parse(entry(dst_entry)).unwrap();
                    assert_eq!(
                        graft_enemy(
                            &out,
                            entry(e - 1),
                            entry(sources[model]),
                            model,
                            &companions
                        )
                        .unwrap(),
                        out,
                        "repeat import must be a no-op"
                    );
                    let dest = Pack::parse(&out).unwrap();
                    let root = u32at(dest.b, dest.table + 8 + 2 * dest.ids.len()).unwrap();
                    let original_root =
                        u32at(original.b, original.table + 8 + 2 * original.ids.len()).unwrap();
                    assert_eq!(
                        dest.array(root).unwrap(),
                        original.array(original_root).unwrap()
                    );
                    let original_static = info(&original, "beStaticInfo").unwrap();
                    let original_banks = original
                        .array(original.val(original_static, 5).unwrap())
                        .unwrap();
                    let static_info = info(&dest, "beStaticInfo").unwrap();
                    let registered_banks = dest.array(dest.val(static_info, 5).unwrap()).unwrap();
                    assert!(registered_banks.starts_with(&original_banks));
                    let si = info(&dest, "beSoundInfo").unwrap();
                    let files = dest.array(dest.val(si, 7).unwrap()).unwrap();
                    for companion in
                        graft::companion_models(&models, None, &[model.to_string()]).unwrap()
                    {
                        if let Some(binding) =
                            graft::find_named(&src, "beSeInfoData", &companion).unwrap()
                        {
                            if !src.array(src.val(binding, 4).unwrap()).unwrap().is_empty()
                                && graft::find_named(&dest, "beSeInfoData", &companion)
                                    .unwrap()
                                    .is_none()
                            {
                                missing.push(format!(
                                    "{e} {model}: missing companion audio {companion}"
                                ));
                            }
                        }
                    }
                    for actor in
                        std::iter::once(model.to_string()).chain(companions.iter().cloned())
                    {
                        let Some(binding) =
                            graft::find_named(&src, "beSeInfoData", &actor).unwrap()
                        else {
                            continue;
                        };
                        let actions = src.array(src.val(binding, 4).unwrap()).unwrap();
                        if actions.is_empty() {
                            continue;
                        }
                        let target = graft::find_named(&dest, "beSeInfoData", &actor)
                            .unwrap()
                            .unwrap();
                        let copied = dest.array(dest.val(target, 4).unwrap()).unwrap();
                        assert_eq!(actions.len(), copied.len(), "{actor} action count");
                        for (&before, &after) in actions.iter().zip(&copied) {
                            let (a, b) = (src.rec(before).unwrap(), dest.rec(after).unwrap());
                            for (&slot, range) in &a.fields {
                                if !graft::ref_slots(&src, a).any(|(n, _)| n == slot) {
                                    assert_eq!(
                                        src.b[range.clone()],
                                        dest.b[b.field(slot).unwrap()],
                                        "{actor} action field {slot}"
                                    );
                                }
                            }
                        }
                        for name in required(&src, &actor).unwrap() {
                            let (source, sid) = [&original, &src, &fallback]
                                .into_iter()
                                .find_map(|p| {
                                    graft::find_named(p, "beSoundData", &name)
                                        .unwrap()
                                        .map(|id| (p, id))
                                })
                                .unwrap();
                            let did = graft::find_named(&dest, "beSoundData", &name)
                                .unwrap()
                                .unwrap();
                            let (a, b) = (source.rec(sid).unwrap(), dest.rec(did).unwrap());
                            for (&slot, range) in &a.fields {
                                if slot != 5 && !graft::ref_slots(source, a).any(|(n, _)| n == slot)
                                {
                                    assert_eq!(
                                        source.b[range.clone()],
                                        dest.b[b.field(slot).unwrap()],
                                        "{actor} {name} cue/parameter {slot}"
                                    );
                                }
                            }
                            let filenames = source
                                .array(source.val(info(source, "beSoundInfo").unwrap(), 7).unwrap())
                                .unwrap();
                            let file = filenames[source.val(sid, 5).unwrap() as usize];
                            let path = graft::name_of(source, source.rec(file).unwrap()).unwrap();
                            let target_files = dest
                                .array(dest.val(info(&dest, "beSoundInfo").unwrap(), 7).unwrap())
                                .unwrap();
                            let target_file = target_files[dest.val(did, 5).unwrap() as usize];
                            assert!(graft::name_of(&dest, dest.rec(target_file).unwrap())
                                .unwrap()
                                .eq_ignore_ascii_case(path));
                            let bank =
                                std::str::from_utf8(path.rsplit(|&c| c == b'/').next().unwrap())
                                    .unwrap();
                            if let Some(srcbank) = bank_info(source, bank).unwrap() {
                                let target_bank = bank_info(&dest, bank).unwrap().unwrap();
                                let root =
                                    u32at(dest.b, dest.table + 8 + 2 * dest.ids.len()).unwrap();
                                let static_info = info(&dest, "beStaticInfo").unwrap();
                                assert!(dest.array(root).unwrap().contains(&static_info));
                                assert!(
                                    dest.array(dest.val(static_info, 5).unwrap())
                                        .unwrap()
                                        .contains(&target_bank),
                                    "{e} {actor} {name} bank {bank} must be registered as static data"
                                );
                                assert!(!dest.array(root).unwrap().contains(&target_bank));
                                assert_eq!(
                                    source.b[source.memory
                                        [source.val(srcbank, 4).unwrap() as usize]
                                        .clone()
                                        .unwrap()],
                                    dest.b[dest.memory[dest.val(target_bank, 4).unwrap() as usize]
                                        .clone()
                                        .unwrap()],
                                    "{actor} bank bytes"
                                );
                            }
                        }
                    }
                    let mut labels = Vec::new();
                    for action in src.array(src.val(binding, 4).unwrap()).unwrap() {
                        let r = src.rec(action).unwrap();
                        let f = r.field(3).unwrap();
                        let sound = src.b[f.start + 4..f.end].split(|&c| c == 0).next().unwrap();
                        let sound = std::str::from_utf8(sound).unwrap();
                        labels.push(format!(
                            "{}={sound}",
                            String::from_utf8_lossy(graft::name_of(&src, r).unwrap_or_default())
                        ));
                        match graft::find_named(&dest, "beSoundData", sound).unwrap() {
                            Some(id) => {
                                let index = dest.val(id, 5).unwrap() as usize;
                                if index >= files.len() {
                                    missing
                                        .push(format!("{e} {model}: {sound} invalid bank index"));
                                }
                            }
                            None => missing.push(format!("{e} {model}: unresolved {sound}")),
                        }
                    }
                    if !graft::find_named(&dest, "beSeInfoData", model)
                        .unwrap()
                        .is_some()
                    {
                        missing.push(format!("{e} {model}: lost binding"));
                    }
                    println!("AUDIO {variable} {e} {model} {}", labels.join(" "));
                    checked += 1;
                }
            }
            println!("AUDIO SUMMARY {variable}: {checked} checked; missing {missing:?}");
            assert!(checked >= 100);
            assert!(missing.is_empty(), "{missing:?}");
        }
    }
}
