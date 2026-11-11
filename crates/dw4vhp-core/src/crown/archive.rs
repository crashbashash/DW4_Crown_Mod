//! Compose resource edits, rebuild AFS and retain verified disc/recovery recipes.
use super::*;
use crate::writer::Phase;

#[derive(Debug)]
pub(crate) enum Piece {
    Source(Range<usize>),
    Bytes(Vec<u8>),
}
/// A small, self-contained archive rebuild recipe; original bulk data stays mmap-backed.
pub struct CrownPlan {
    pieces: Vec<Piece>,
    /// What the enemy randomiser did, when it ran: recorded in the image signature.
    random: Option<serde_json::Value>,
    pub(crate) rewards: rewards::Plan,
    pub regions: Vec<WriteRegion>,
    pub output_size: u64,
    append_offset: usize,
}
pub(crate) struct Rebuilt {
    pieces: Vec<Piece>,
    length: usize,
    random: Option<serde_json::Value>,
    rewards: rewards::Plan,
    regions: Vec<WriteRegion>,
    difficulty: bool,
    area: Option<Vec<u8>>,
}

pub(in crate::crown) fn filename_table(
    afs: &[u8],
    count: usize,
    first: usize,
) -> Result<Option<Range<usize>>> {
    let mut table = None;
    // AFS writers use either the end of the entry table or the end of the
    // sector-aligned prefix. Retail DW4 uses the former.
    for pos in [8 + 8 * count, first - 8] {
        let offset = u32at(afs, pos)? as usize;
        let length = u32at(afs, pos + 4)? as usize;
        if offset == 0 && length == 0 {
            continue;
        }
        need(
            offset > 0
                && length == count * 48
                && offset <= afs.len()
                && length <= afs.len() - offset,
            "unexpected archive filename table",
        )?;
        let range = offset..offset + length;
        need(
            table.as_ref().is_none_or(|prior| *prior == range),
            "conflicting archive filename tables",
        )?;
        table = Some(range);
    }
    Ok(table)
}

// Estimated rebuild work out of REBUILD_TOTAL; boss shuffling takes the gap
// between the two entry marks when it runs.
const REBUILD_TOTAL: u64 = 1000;
const REBUILD_SCANNED: u64 = 20;
const REBUILD_ENTRIES_BEFORE_BOSSES: u64 = 680;
const REBUILD_ENTRIES: u64 = 980;

/// Rebuilds the archive at `iso[base..base + size]` with every crown model pack
/// expanded and the river turret scripts updated. The shared rebuild also
/// resolves area-local enemy reward tables through the disc's resource manifest.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_afs(
    iso: &[u8],
    base: usize,
    size: usize,
    four_player_spawns: bool,
    crown_drops: bool,
    random_crown_colours: bool,
    colour_seed: u32,
    graft: Option<&graft::Spec>,
    random: Option<RandomOptions>,
    crown_mod: bool,
    final_table: Option<&crate::table::EnemyTable>,
    solo_buttons: bool,
    death_softlock: bool,
    progress: &mut dyn FnMut(Phase, u64, u64),
) -> Result<Rebuilt> {
    let _timing = crate::profiling::measure("archive.rebuild");
    progress(Phase::Building, 0, REBUILD_TOTAL);
    let afs = iso
        .get(base..base + size)
        .ok_or_else(|| bad("NDMW.AFS outside image"))?;
    need(afs.get(..4) == Some(b"AFS\0"), "invalid archive signature")?;
    let count = u32at(afs, 4)? as usize;
    need(
        count > 0 && count < 65536 && 8 + 8 * count <= size,
        "invalid archive count",
    )?;
    let entries = (0..count)
        .map(|i| {
            Ok((
                u32at(afs, 8 + 8 * i)? as usize,
                u32at(afs, 12 + 8 * i)? as usize,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let first = entries[0].0;
    need(
        first >= 16 + 8 * count && first <= size,
        "invalid archive prefix",
    )?;
    let mut random = match random {
        Some(options) => {
            let layout = crate::release::detect(iso)?.layout_in(iso)?;
            let mut random = random::Randomiser::scan(
                afs,
                &entries,
                crate::table::EnemyTable::read(iso, &layout)?,
                options,
            )?;
            random.validate_tiers(iso)?;
            random.card_bases = random::boss::intro::message_bases(iso, afs, &entries)?;
            Some(random)
        }
        None => None,
    };
    progress(Phase::Building, REBUILD_SCANNED, REBUILD_TOTAL);
    // Stage planning dominates randomised builds and costs roughly the same
    // per stage, so it drives progress. Without it, archive entries do.
    let stage_count = random
        .as_ref()
        .map_or(0, |r| (0..count).filter(|&i| r.is_stage(i)).count());
    let entries_end = if random.as_ref().is_some_and(|r| r.opts.random_bosses) {
        REBUILD_ENTRIES_BEFORE_BOSSES
    } else {
        REBUILD_ENTRIES
    };
    let mut stages_done = 0;
    let mut reported = None;
    let mut report_entry = |entries_done: usize, stages_done: usize| {
        let (phase, done, total) = if stage_count > 0 {
            let phase = Phase::RandomisingEnemies {
                area: (stages_done + 1).min(stage_count) as u32,
                areas: stage_count as u32,
            };
            (phase, stages_done, stage_count)
        } else {
            (Phase::Building, entries_done, count)
        };
        let span = entries_end - REBUILD_SCANNED;
        let done = REBUILD_SCANNED + span * done as u64 / total as u64;
        if reported != Some((phase, done)) {
            reported = Some((phase, done));
            progress(phase, done, REBUILD_TOTAL);
        }
    };
    report_entry(0, 0);
    let mut prefix = afs[..first].to_vec();
    let mut pieces = Vec::new();
    let mut cursor = first;
    let mut newentries = Vec::new();
    let mut patched = Vec::new();
    let mut crown_packs = 0;
    let mut spawn_packs = 0;
    let mut drop_packs = 0;
    let mut river_packs = 0;
    let mut practice_packs = 0;
    let mut exp_packs = 0;
    let mut library = drops::Library::default();
    if crown_drops {
        for &(off, len) in &entries {
            let data = afs
                .get(off..off + len)
                .ok_or_else(|| bad("loot archive entry outside archive"))?;
            if memchr::memmem::find(data, b"beGeneraterData\0").is_some() {
                library.collect(data)?;
            }
        }
    }
    // A stage model is planned after its sound pack in archive order. Keep edits until
    // planning finishes so imported enemy audio can update that earlier entry.
    let mut entry_edits = Vec::with_capacity(count);
    for (i, &(off, len)) in entries.iter().enumerate() {
        let next = entries.get(i + 1).map_or(size, |e| e.0);
        need(
            off <= next && len <= next - off && next <= size && off % 2048 == 0,
            "invalid archive entry extent/order",
        )?;
        let data = &afs[off..off + len];
        // Picks need only the stage packs as the planner receives them.
        if random
            .as_ref()
            .is_some_and(|r| r.opts.picks_only && !r.is_stage(i))
        {
            entry_edits.push(Vec::new());
            continue;
        }
        let edits = if memchr::memmem::find(data, b"i_rareicon").is_some() {
            crown_packs += 1;
            let edits = expand(data)?;
            if (crown_mod || random_crown_colours)
                && memchr::memmem::find(data, b"g_numepodz01\0").is_some()
            {
                river_packs += 1;
                let expanded = materialize(data, &edits)?;
                let edits = river::expand_with_balance(
                    &expanded,
                    random_crown_colours,
                    colour_seed,
                    crown_mod,
                )
                .map_err(|e| bad(&format!("river archive entry {i}: {e}")))?;
                vec![Edit {
                    range: 0..data.len(),
                    bytes: materialize(&expanded, &edits)?,
                }]
            } else if four_player_spawns && practice::candidate(data) {
                let expanded = materialize(data, &edits)?;
                let more = practice::expand(&expanded)
                    .map_err(|e| bad(&format!("practice archive entry {i}: {e}")))?;
                if more.is_empty() {
                    edits
                } else {
                    practice_packs += 1;
                    vec![Edit {
                        range: 0..data.len(),
                        bytes: materialize(&expanded, &more)?,
                    }]
                }
            } else {
                edits
            }
        } else if (four_player_spawns || crown_drops)
            && memchr::memmem::find(data, b"beGeneraterData\0").is_some()
        {
            let mut bytes = data.to_vec();
            if four_player_spawns {
                let edits = spawns::expand(&bytes)
                    .map_err(|e| bad(&format!("spawn archive entry {i}: {e}")))?;
                if !edits.is_empty() {
                    spawn_packs += 1;
                    bytes = materialize(&bytes, &edits)?;
                }
            }
            if crown_drops {
                let edits = drops::expand(&bytes, &library)
                    .map_err(|e| bad(&format!("loot archive entry {i}: {e}")))?;
                if !edits.is_empty() {
                    drop_packs += 1;
                    bytes = materialize(&bytes, &edits)?;
                }
            }
            if bytes == data {
                Vec::new()
            } else {
                vec![Edit {
                    range: 0..data.len(),
                    bytes,
                }]
            }
        } else {
            Vec::new()
        };
        let edits = if crown_mod && exp::candidate(data) {
            exp_packs += 1;
            let current = materialize(data, &edits)?;
            let more =
                exp::expand(&current).map_err(|e| bad(&format!("EXP archive entry {i}: {e}")))?;
            vec![Edit {
                range: 0..data.len(),
                bytes: materialize(&current, &more)?,
            }]
        } else {
            edits
        };
        let edits = match graft {
            Some(g)
                if i == g.model_entry
                    || g.generator_entries.contains(&i)
                    || g.lua_edits.iter().any(|e| e.entry == i) =>
            {
                let current = if i == g.model_entry && !g.expand_model_pack {
                    data.to_vec()
                } else {
                    materialize(data, &edits)?
                };
                let mut bytes = current;
                if i == g.model_entry && !g.models.is_empty() {
                    let sources = std::iter::once((g.source_entry, &g.models))
                        .chain(g.extra_sources.iter().map(|(e, m)| (*e, m)));
                    for (src_entry, models) in sources {
                        // Models the stage already carries need no copy.
                        let mut missing = Vec::new();
                        for m in models {
                            if !graft::resident(&bytes, m)? {
                                missing.push(m.clone());
                            }
                        }
                        if missing.is_empty() {
                            continue;
                        }
                        let (soff, slen) = *entries
                            .get(src_entry)
                            .ok_or_else(|| bad("graft source entry outside archive"))?;
                        let source = afs
                            .get(soff..soff + slen)
                            .ok_or_else(|| bad("graft source outside archive"))?;
                        let (more, report) =
                            graft::graft(&bytes, source, &missing).map_err(|e| {
                                bad(&format!(
                                    "graft {missing:?} into entry {i} from {src_entry}: {e}"
                                ))
                            })?;
                        eprintln!("graft entry {i} <- {src_entry}: {report:?}");
                        bytes = materialize(&bytes, &more)?;
                    }
                } else if g.generator_entries.contains(&i) {
                    let (out, n) = graft::retarget(
                        &bytes,
                        &g.rows,
                        &g.spread,
                        &g.only_generators,
                        &g.generator_models,
                    )
                    .map_err(|e| bad(&format!("retarget entry {i}: {e}")))?;
                    need(n > 0, "no generator rows were retargeted")?;
                    eprintln!("retarget entry {i}: {n} generators");
                    bytes = out;
                }
                for e in g.lua_edits.iter().filter(|e| e.entry == i) {
                    let more = lua_names::rename_in_pack(&bytes, &e.scripts, &e.names)
                        .map_err(|err| bad(&format!("Lua rename in entry {i}: {err}")))?;
                    eprintln!("lua rename entry {i}: {:?}", e.names);
                    bytes = materialize(&bytes, &more)?;
                }
                vec![Edit {
                    range: 0..data.len(),
                    bytes,
                }]
            }
            _ => edits,
        };
        let edits = match random.as_mut() {
            Some(r) if r.is_stage(i) || r.plans_for(i) => {
                let current = match edits.as_slice() {
                    [e] if e.range == (0..data.len()) => e.bytes.clone(),
                    _ => materialize(data, &edits)?,
                };
                let bytes = if r.is_stage(i) {
                    let bytes = r
                        .stage(i, current, afs, &entries)
                        .map_err(|e| bad(&format!("randomise entry {i}: {e}")))?;
                    stages_done += 1;
                    bytes
                } else {
                    r.retarget(i, &current)
                        .map_err(|e| bad(&format!("randomise entry {i}: {e}")))?
                        .unwrap_or(current)
                };
                vec![Edit {
                    range: 0..data.len(),
                    bytes,
                }]
            }
            _ => edits,
        };
        entry_edits.push(edits);
        report_entry(i + 1, stages_done);
    }
    if let Some(r) = random.as_mut().filter(|r| r.opts.random_bosses) {
        r.randomise_bosses(
            iso,
            afs,
            &entries,
            &mut entry_edits,
            crown_mod,
            &mut |phase, d, t| {
                let span = REBUILD_ENTRIES - entries_end;
                progress(phase, entries_end + span * d / t.max(1), REBUILD_TOTAL)
            },
        )?;
        #[cfg(feature = "devtools")]
        if let Some(roster) = std::env::var_os("DW4_BOSS_TEST_ROSTER") {
            std::fs::write(
                roster,
                serde_json::to_vec_pretty(&r.boss_report)
                    .map_err(|_| bad("cannot encode boss roster"))?,
            )?;
        }
    }
    #[cfg(feature = "devtools")]
    if let (Some(r), Some(out)) = (random.as_ref(), std::env::var_os("DW4_PICKS_OUT")) {
        // Validation: the choices a full image build made, for comparison with picks mode.
        std::fs::write(
            out,
            serde_json::to_vec(&r.picks()).map_err(|_| bad("cannot encode picks"))?,
        )?;
    }
    if let Some(r) = random.as_ref().filter(|r| r.opts.picks_only) {
        return Ok(Rebuilt {
            pieces: Vec::new(),
            length: 0,
            random: Some(r.picks()),
            rewards: rewards::Plan::default(),
            regions: Vec::new(),
            difficulty: false,
            area: None,
        });
    }
    if let Some(r) = random.as_mut() {
        for (i, bytes) in std::mem::take(&mut r.sounds) {
            entry_edits[i] = vec![Edit {
                range: 0..entries[i].1,
                bytes,
            }];
        }
    }
    // Final script edits, after every stage, boss and sound edit has settled:
    // scripted spawns' literal HP gets the final table's HP factor, solo
    // button pairs latch and the first-death tutorial needs its window.
    let authored = match final_table {
        Some(_) => Some(crate::table::EnemyTable::read(
            iso,
            &crate::release::detect(iso)?.layout_in(iso)?,
        )?),
        None => None,
    };
    let finish = |pack: &[u8],
                  variants: Option<&BTreeMap<usize, rewards::Variant>>|
     -> Result<Option<Vec<u8>>> {
        let mut out = None;
        if let (Some(balanced), Some(authored)) = (final_table, &authored) {
            out = script_hp::rescale(pack, authored, balanced, variants)
                .map_err(|e| bad(&format!("script HP: {e}")))?;
        }
        if solo_buttons {
            let current = out.as_deref().unwrap_or(pack);
            if let Some(bytes) =
                buttons::latch(current).map_err(|e| bad(&format!("solo buttons: {e}")))?
            {
                out = Some(bytes);
            }
        }
        if death_softlock {
            let current = out.as_deref().unwrap_or(pack);
            if let Some(bytes) =
                death::fix(current).map_err(|e| bad(&format!("death softlock: {e}")))?
            {
                out = Some(bytes);
            }
        }
        Ok(out)
    };
    let candidate = |b: &[u8]| {
        (final_table.is_some() && script_hp::candidate(b))
            || (solo_buttons && buttons::candidate(b))
            || (death_softlock && memchr::memmem::find(b, b"SQ_GAMEOVER\0").is_some())
    };
    {
        let rewards = random.as_ref().map(|r| &r.rewards);
        let variants = |entry: usize| rewards.and_then(|r| r.get(&entry)).map(|s| &s.rows);
        for (i, edits) in entry_edits.iter_mut().enumerate() {
            let (off, len) = entries[i];
            let data = &afs[off..off + len];
            if !candidate(data) && !edits.iter().any(|e| candidate(&e.bytes)) {
                continue;
            }
            let finished = match edits.as_slice() {
                [] => finish(data, variants(i)),
                [e] if e.range == (0..len) => finish(&e.bytes, variants(i)),
                _ => finish(&materialize(data, edits)?, variants(i)),
            }
            .map_err(|e| bad(&format!("entry {i}: {e}")))?;
            if let Some(bytes) = finished {
                *edits = vec![Edit {
                    range: 0..len,
                    bytes,
                }];
            }
        }
    }
    if let Some(r) = random.as_mut() {
        for (&entry, packs) in &mut r.extra {
            for pack in packs {
                let variants = r.rewards.get(&entry).map(|s| &s.rows);
                if let Some(bytes) = finish(pack, variants)
                    .map_err(|e| bad(&format!("entry {entry} variant: {e}")))?
                {
                    *pack = bytes;
                }
            }
        }
    }
    let rewards = random
        .as_ref()
        .map(|r| r.reward_plan(iso))
        .transpose()?
        .unwrap_or_default();
    let extra = random
        .as_mut()
        .map(|r| std::mem::take(&mut r.extra))
        .unwrap_or_default();
    let area = match random.as_ref() {
        Some(r) if r.opts.random_bosses || !r.card_stages.is_empty() => {
            let mut stages = r.card_stages.clone();
            if r.opts.random_bosses {
                stages.extend(random::boss::ARENAS.iter().map(|a| a.entry));
            }
            Some(random::boss::intro::build(
                iso,
                afs,
                &entries,
                &rewards,
                final_table.unwrap_or(&r.table),
                &stages,
            )?)
        }
        _ => None,
    };
    let expanded_count = count + 2 * extra.len();
    let difficulty = !extra.is_empty();
    need(
        expanded_count < 65536,
        "difficulty archive has too many entries",
    )?;
    let targets: BTreeMap<_, _> = extra
        .keys()
        .enumerate()
        .map(|(index, &entry)| (entry, [count + 2 * index, count + 2 * index + 1]))
        .collect();
    if difficulty {
        let expanded_prefix = a2048(first.max(16 + 8 * expanded_count));
        prefix = vec![0; expanded_prefix];
        prefix[..8].copy_from_slice(&afs[..8]);
        put(&mut prefix, 4, expanded_count as u32);
        cursor = expanded_prefix;
        for (index, &(off, len)) in entries.iter().enumerate() {
            let current = materialize(&afs[off..off + len], &entry_edits[index])?;
            if let Some(bytes) = difficulty::retarget_manifest(&current, &targets)? {
                entry_edits[index] = vec![Edit {
                    range: 0..len,
                    bytes,
                }];
            }
        }
    }
    for (i, ((off, len), edits)) in entries.iter().copied().zip(entry_edits).enumerate() {
        // CRI caches sector counts and sums them to locate each file. The old
        // filename table must move after the appended variants, rather than
        // surviving as an unindexed gap between the last old and first new file.
        let next = entries
            .get(i + 1)
            .map_or(if difficulty { a2048(off + len) } else { size }, |e| e.0);
        newentries.push((cursor, len));
        if !edits.is_empty() {
            let mut q = 0;
            let mut newlen = len;
            for e in edits {
                need(e.range.start >= q, "overlapping archive edits")?;
                if e.range.start > q {
                    pieces.push(Piece::Source(base + off + q..base + off + e.range.start));
                }
                newlen = newlen + e.bytes.len() - e.range.len();
                pieces.push(Piece::Bytes(e.bytes));
                q = e.range.end;
            }
            if q < len {
                pieces.push(Piece::Source(base + off + q..base + off + len));
            }
            newentries[i].1 = newlen;
            cursor += newlen;
            let aligned = a2048(cursor);
            if aligned > cursor {
                pieces.push(Piece::Bytes(vec![0; aligned - cursor]));
            }
            cursor = aligned;
            let oldend = a2048(off + len);
            if next > oldend {
                pieces.push(Piece::Source(base + oldend..base + next));
                cursor += next - oldend;
            }
            patched.push(i);
        } else {
            pieces.push(Piece::Source(base + off..base + next));
            cursor += next - off;
        }
    }
    for variants in extra.into_values() {
        for bytes in variants {
            newentries.push((cursor, bytes.len()));
            cursor += bytes.len();
            pieces.push(Piece::Bytes(bytes));
            let aligned = a2048(cursor);
            if aligned > cursor {
                pieces.push(Piece::Bytes(vec![0; aligned - cursor]));
            }
            cursor = aligned;
        }
    }
    need(crown_packs == 85, "expected 85 crown model packs")?;
    need(
        river_packs == usize::from(crown_mod || random_crown_colours),
        "unexpected Numenume River patch count",
    )?;
    need(
        exp_packs == if crown_mod { 85 } else { 0 },
        "unexpected EXP popup patch count",
    )?;
    need(
        !four_player_spawns || spawn_packs > 0,
        "no four-player spawn profiles found",
    )?;
    need(
        !four_player_spawns || practice_packs == 1,
        "expected one practice graduation script",
    )?;
    need(
        !crown_drops || drop_packs > 0,
        "no custom loot profiles found",
    )?;
    for (i, &(off, len)) in newentries.iter().enumerate() {
        need(
            off <= u32::MAX as usize && len <= u32::MAX as usize,
            "archive exceeds format limit",
        )?;
        put(&mut prefix, 8 + 8 * i, off as u32);
        put(&mut prefix, 12 + 8 * i, len as u32);
    }
    if difficulty {
        let mut sector_offset = prefix.len();
        for &(offset, length) in &newentries {
            need(
                offset == sector_offset,
                "archive gap differs from native cached sector offsets",
            )?;
            sector_offset = a2048(offset + length);
        }
    }
    if !difficulty {
        for pos in [8 + 8 * count, first - 8] {
            let footer = u32at(afs, pos)? as usize;
            let len = u32at(afs, pos + 4)? as usize;
            if footer == 0 && len == 0 {
                continue;
            }
            need(
                footer > 0 && len > 0 && footer <= size && len <= size - footer,
                "invalid archive footer",
            )?;
            let owner = entries
                .iter()
                .rposition(|e| e.0 <= footer)
                .ok_or_else(|| bad("footer has no owner"))?;
            let (oldoff, oldlen) = entries[owner];
            let (newoff, newlen) = newentries[owner];
            let relocated = if patched.contains(&owner) {
                let end = a2048(oldoff + oldlen);
                need(footer >= end, "footer overlaps modified pack")?;
                a2048(newoff + newlen) + footer - end
            } else {
                newoff + footer - oldoff
            };
            put(&mut prefix, pos, relocated as u32);
        }
    } else {
        if let Some(table) = filename_table(afs, count, first)? {
            let mut names = afs[table].to_vec();
            names.resize(expanded_count * 48, 0);
            for (entry, &(_, length)) in newentries.iter().enumerate() {
                if entry >= count {
                    let name = format!("crown{entry}.igb");
                    names[entry * 48..entry * 48 + name.len()].copy_from_slice(name.as_bytes());
                }
                put(&mut names, entry * 48 + 44, length as u32);
            }
            for pos in [8 + 8 * expanded_count, prefix.len() - 8] {
                put(&mut prefix, pos, cursor as u32);
                put(&mut prefix, pos + 4, names.len() as u32);
            }
            cursor += names.len();
            pieces.push(Piece::Bytes(names));
            let aligned = a2048(cursor);
            if aligned > cursor {
                pieces.push(Piece::Bytes(vec![0; aligned - cursor]));
            }
            cursor = aligned;
        }
    }
    pieces.insert(0, Piece::Bytes(prefix));
    if let Some(r) = &random {
        if std::env::var_os("DW4_RANDOM_LOG").is_some() {
            for line in &r.log {
                eprintln!("{line}");
            }
        }
    }
    progress(Phase::Building, REBUILD_TOTAL, REBUILD_TOTAL);
    Ok(Rebuilt {
        pieces,
        length: cursor,
        random: random.as_ref().map(random::Randomiser::summary),
        rewards,
        regions: if difficulty {
            vec![difficulty::file_count(iso, count, expanded_count)?]
        } else {
            Vec::new()
        },
        difficulty,
        area,
    })
}

/// Keep the original AREA bytes for recovery and original-table verification.
/// Its rebuilt localized packs load through the unchanged virtual filesystem.
fn append_area(
    iso: &[u8],
    start: usize,
    ndmw_size: usize,
    area: Option<Vec<u8>>,
    pieces: &mut Vec<Piece>,
) -> Result<(usize, Vec<WriteRegion>)> {
    let Some(area) = area else {
        return Ok((start + ndmw_size, Vec::new()));
    };
    let offset = a2048(start + ndmw_size);
    pieces.push(Piece::Bytes(vec![0; offset - start - ndmw_size]));
    let end = offset + area.len();
    let regions = if crate::release::detect(iso)?.is_gamecube() {
        let file = gcm::find_file(iso, "AREA.AFS")?;
        vec![
            WriteRegion {
                offset: file.offset_field,
                bytes: u32::try_from(offset)
                    .map_err(|_| bad("localized archive exceeds format limit"))?
                    .to_be_bytes()
                    .to_vec(),
            },
            WriteRegion {
                offset: file.size_field,
                bytes: u32::try_from(area.len())
                    .map_err(|_| bad("localized archive exceeds format limit"))?
                    .to_be_bytes()
                    .to_vec(),
            },
        ]
    } else {
        let file = iso9660::find_file(iso, "AREA.AFS")?;
        let sector = u32::try_from(offset / 2048)
            .map_err(|_| bad("localized archive exceeds format limit"))?;
        let size =
            u32::try_from(area.len()).map_err(|_| bad("localized archive exceeds format limit"))?;
        vec![
            WriteRegion {
                offset: file.record_offset + 2,
                bytes: [sector.to_le_bytes(), sector.to_be_bytes()].concat(),
            },
            WriteRegion {
                offset: file.record_offset + 10,
                bytes: [size.to_le_bytes(), size.to_be_bytes()].concat(),
            },
        ]
    };
    pieces.push(Piece::Bytes(area));
    Ok((end, regions))
}

fn ps2_memory_module(iso: &[u8]) -> Result<Option<iso9660::IsoFile>> {
    crate::ps2_memory::relocated_module(iso, crate::release::detect(iso)?)
}

/// Appends a copy of a PS2 file (sector-aligned) and points its directory
/// record at the copy, freeing its original sectors.
fn append_ps2_file(
    start: usize,
    file: &iso9660::IsoFile,
    pieces: &mut Vec<Piece>,
) -> Result<(usize, Vec<WriteRegion>)> {
    let offset = a2048(start);
    pieces.push(Piece::Bytes(vec![0; offset - start]));
    let source = file.lba as usize * 2048;
    pieces.push(Piece::Source(source..source + file.size as usize));
    let sector =
        u32::try_from(offset / 2048).map_err(|_| bad("relocated file exceeds format limit"))?;
    Ok((
        offset + file.size as usize,
        vec![WriteRegion {
            offset: file.record_offset + 2,
            bytes: [sector.to_le_bytes(), sector.to_be_bytes()].concat(),
        }],
    ))
}

impl CrownPlan {
    pub fn build(iso: &[u8]) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, None, None)
    }
    /// Experimental: the usual crown expansion plus an enemy model graft.
    pub fn build_graft(iso: &[u8], spec: &graft::Spec) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, Some(spec), None)
    }
    /// Analysis: the enemy randomiser's choices for a PS2 or GameCube image built with the
    /// same settings, without building the image. Requires `options.picks_only`.
    #[allow(clippy::too_many_arguments)]
    pub fn random_picks(
        iso: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        options: RandomOptions,
        crown_mod: bool,
    ) -> Result<serde_json::Value> {
        need(options.picks_only, "picks require the analysis option")?;
        let (base, size) = if crate::release::detect(iso)?.is_gamecube() {
            let file = gcm::find_file(iso, "ndmw.afs")?;
            (file.offset as usize, file.size as usize)
        } else {
            let file = iso9660::find_file(iso, "NDMW.AFS")?;
            (file.lba as usize * 2048, file.size as usize)
        };
        rebuild_afs(
            iso,
            base,
            size,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            None,
            Some(options),
            crown_mod,
            None,
            false,
            false,
            &mut |_, _, _| {},
        )?
        .random
        .ok_or_else(|| bad("no randomiser picks"))
    }
    /// Experimental: the usual crown expansion plus the enemy randomiser.
    pub fn build_random(iso: &[u8], options: RandomOptions) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, None, Some(options))
    }
    pub(crate) fn build_with_options(
        iso: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        graft: Option<&graft::Spec>,
        random: Option<RandomOptions>,
    ) -> Result<Self> {
        Self::build_with_balance(
            iso,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            graft,
            random,
            true,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_with_balance(
        iso: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        graft: Option<&graft::Spec>,
        random: Option<RandomOptions>,
        crown_mod: bool,
        final_table: Option<&crate::table::EnemyTable>,
    ) -> Result<Self> {
        Self::build_with_balance_and_progress(
            iso,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            graft,
            random,
            crown_mod,
            final_table,
            false,
            false,
            &mut |_, _, _| {},
        )
    }
    /// [`CrownPlan::build_with_balance`], reporting archive rebuild progress.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_with_balance_and_progress(
        iso: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        graft: Option<&graft::Spec>,
        random: Option<RandomOptions>,
        crown_mod: bool,
        final_table: Option<&crate::table::EnemyTable>,
        solo_buttons: bool,
        death_softlock: bool,
        progress: &mut dyn FnMut(Phase, u64, u64),
    ) -> Result<Self> {
        need(
            random.as_ref().is_none_or(|r| !r.picks_only),
            "picks-only randomiser analysis cannot build an image",
        )?;
        let file = iso9660::find_file(iso, "NDMW.AFS")?;
        let base = file.lba as usize * 2048;
        let size = file.size as usize;
        let Rebuilt {
            mut pieces,
            length: cursor,
            random,
            rewards,
            regions: resource_regions,
            difficulty,
            area,
        } = rebuild_afs(
            iso,
            base,
            size,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            graft,
            random,
            crown_mod,
            final_table,
            solo_buttons,
            death_softlock,
            progress,
        )?;
        let sectors = u32at(iso, 16 * 2048 + 80)? as usize;
        let append_offset = sectors * 2048;
        need(append_offset == iso.len(), "unexpected trailing ISO data")?;
        let (extended_size, area_regions) =
            append_area(iso, append_offset, cursor, area, &mut pieces)?;
        // The PS2 allocator hooks need more code than the boot ELF's last
        // sector holds: move the module stored after it to the end.
        let module = match random {
            Some(_) => ps2_memory_module(iso)?,
            None => None,
        };
        let (extended_size, module_regions) = match &module {
            Some(m) => append_ps2_file(extended_size, m, &mut pieces)?,
            None => (extended_size, Vec::new()),
        };
        let output_size = a2048(extended_size);
        need(
            output_size / 2048 <= u32::MAX as usize && cursor <= u32::MAX as usize,
            "expanded image exceeds format limit",
        )?;
        let mut extent = Vec::new();
        extent.extend((sectors as u32).to_le_bytes());
        extent.extend((sectors as u32).to_be_bytes());
        let mut length = Vec::new();
        length.extend((cursor as u32).to_le_bytes());
        length.extend((cursor as u32).to_be_bytes());
        let mut regions = vec![
            WriteRegion {
                offset: file.record_offset + 2,
                bytes: extent,
            },
            WriteRegion {
                offset: file.record_offset + 10,
                bytes: length,
            },
        ];
        regions.extend(resource_regions);
        regions.extend(area_regions);
        regions.extend(module_regions);
        if random.is_some() {
            let release = crate::release::detect(iso)?;
            regions.extend(crate::ps2_memory::regions_for_release(
                iso,
                release,
                difficulty,
                module.as_ref(),
            )?);
        }
        for sector in 16..(iso.len() / 2048).min(64) {
            let p = sector * 2048;
            need(&iso[p + 1..p + 6] == b"CD001", "invalid volume descriptor")?;
            if iso[p] == 255 {
                break;
            }
            if iso[p] == 1 || iso[p] == 2 {
                let n = (output_size / 2048) as u32;
                let mut bytes = n.to_le_bytes().to_vec();
                bytes.extend(n.to_be_bytes());
                regions.push(WriteRegion {
                    offset: (p + 80) as u64,
                    bytes,
                });
            }
        }
        regions.extend(rewards.models.clone());
        Ok(Self {
            pieces,
            rewards,
            regions,
            output_size: output_size as u64,
            append_offset,
            random,
        })
    }
    /// GameCube twin of [`CrownPlan::build`]: the rebuilt archive is appended
    /// after the last byte of the image and the FST entry repointed at it.
    pub fn build_gcm(disc: &[u8]) -> Result<Self> {
        Self::build_gcm_with_options(disc, false, false, false, 0, None)
    }
    pub(crate) fn build_gcm_with_options(
        disc: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        random: Option<RandomOptions>,
    ) -> Result<Self> {
        Self::build_gcm_with_balance(
            disc,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            random,
            true,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_gcm_with_balance(
        disc: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        random: Option<RandomOptions>,
        crown_mod: bool,
        final_table: Option<&crate::table::EnemyTable>,
    ) -> Result<Self> {
        Self::build_gcm_with_balance_and_progress(
            disc,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            random,
            crown_mod,
            final_table,
            false,
            false,
            &mut |_, _, _| {},
        )
    }
    /// [`CrownPlan::build_gcm_with_balance`], reporting archive rebuild progress.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_gcm_with_balance_and_progress(
        disc: &[u8],
        four_player_spawns: bool,
        crown_drops: bool,
        random_crown_colours: bool,
        colour_seed: u32,
        random: Option<RandomOptions>,
        crown_mod: bool,
        final_table: Option<&crate::table::EnemyTable>,
        solo_buttons: bool,
        death_softlock: bool,
        progress: &mut dyn FnMut(Phase, u64, u64),
    ) -> Result<Self> {
        need(
            random.as_ref().is_none_or(|r| !r.picks_only),
            "picks-only randomiser analysis cannot build an image",
        )?;
        let file = gcm::find_file(disc, "ndmw.afs")?;
        let (base, size) = (file.offset as usize, file.size as usize);
        let Rebuilt {
            mut pieces,
            length: cursor,
            random,
            rewards,
            regions: resource_regions,
            difficulty,
            area,
        } = rebuild_afs(
            disc,
            base,
            size,
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            colour_seed,
            None,
            random,
            crown_mod,
            final_table,
            solo_buttons,
            death_softlock,
            progress,
        )?;
        let append_offset = disc.len();
        need(
            append_offset.is_multiple_of(4),
            "unaligned GameCube image length",
        )?;
        let (extended_size, area_regions) =
            append_area(disc, append_offset, cursor, area, &mut pieces)?;
        need(
            extended_size as u64 <= u32::MAX as u64,
            "expanded image exceeds format limit",
        )?;
        let mut regions = vec![
            WriteRegion {
                offset: file.offset_field,
                bytes: (append_offset as u32).to_be_bytes().to_vec(),
            },
            WriteRegion {
                offset: file.size_field,
                bytes: (cursor as u32).to_be_bytes().to_vec(),
            },
        ];
        regions.extend(resource_regions);
        regions.extend(area_regions);
        if random.is_some() {
            let release = crate::release::detect(disc)?;
            regions.extend(crate::gc_memory::regions_for_release(disc, release)?);
            if difficulty {
                regions.extend(crate::difficulty_io::gamecube_regions(disc, release)?);
            }
        }
        regions.extend(rewards.models.clone());
        Ok(Self {
            pieces,
            rewards,
            regions,
            output_size: extended_size as u64,
            append_offset,
            random,
        })
    }
    /// Append identification outside NDMW.AFS; preserve every resource offset and length.
    pub fn with_signature(
        self,
        release: crate::release::Release,
        plan: &crate::plan::PatchPlan,
    ) -> Result<Self> {
        let mut metadata = crate::signature::metadata(release, plan);
        if let Some(random) = &self.random {
            metadata["randomEnemies"] = random.clone();
        }
        self.finish_signature(release, metadata)
    }

    /// Store the original bytes of every overwritten region outside all game files.
    pub(crate) fn with_recovery(
        mut self,
        iso: &[u8],
        release: crate::release::Release,
        plan: &crate::plan::PatchPlan,
        regions: &[WriteRegion],
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Self> {
        let recovery = crate::update::capture(iso, regions, progress)?;
        let used: usize = self
            .pieces
            .iter()
            .map(|p| match p {
                Piece::Source(r) => r.len(),
                Piece::Bytes(b) => b.len(),
            })
            .sum();
        let offset = self.output_size as usize;
        let padding = offset
            .checked_sub(self.append_offset + used)
            .ok_or_else(|| bad("recovery overlaps archive"))?;
        self.pieces.push(Piece::Bytes(vec![0; padding]));
        let mut metadata = crate::signature::metadata(release, plan);
        if let Some(random) = &self.random {
            metadata["randomEnemies"] = random.clone();
        }
        metadata["recovery"] = serde_json::json!({
            "formatVersion": 1, "offset": offset, "length": recovery.len(),
            "sha1": crate::update::digest(&recovery), "originalSize": iso.len(),
            "regionCount": regions.len(), "archiveEnd": self.output_size,
        });
        self.pieces.push(Piece::Bytes(recovery));
        self.output_size = offset as u64
            + match self.pieces.last().unwrap() {
                Piece::Bytes(b) => b.len() as u64,
                _ => 0,
            };
        self.finish_signature(release, metadata)
    }

    fn finish_signature(
        mut self,
        release: crate::release::Release,
        metadata: serde_json::Value,
    ) -> Result<Self> {
        let offset = a2048(self.output_size as usize);
        let used: usize = self
            .pieces
            .iter()
            .map(|p| match p {
                Piece::Source(r) => r.len(),
                Piece::Bytes(b) => b.len(),
            })
            .sum();
        let padding = offset
            .checked_sub(self.append_offset + used)
            .ok_or_else(|| bad("signature overlaps archive"))?;
        self.pieces.push(Piece::Bytes(vec![0; padding]));
        self.pieces
            .push(Piece::Bytes(crate::signature::encode(&metadata)?));
        self.output_size = offset as u64 + crate::signature::SIZE as u64;
        if release.is_gamecube() {
            need(
                self.output_size <= u32::MAX as u64,
                "signed image exceeds GameCube format limit",
            )?;
        } else {
            let sectors = u32::try_from(self.output_size / 2048)
                .map_err(|_| bad("signed image exceeds ISO format limit"))?;
            for r in &mut self.regions {
                if r.offset >= 16 * 2048 && r.offset < 64 * 2048 && r.offset % 2048 == 80 {
                    r.bytes = [sectors.to_le_bytes(), sectors.to_be_bytes()].concat();
                }
            }
        }
        Ok(self)
    }
    /// Experimental: write the complete patched image (original bytes, appended archive, in-place regions).
    pub fn write_image(&self, iso: &[u8], out: &mut std::fs::File) -> Result<()> {
        use std::io::{Seek, SeekFrom};
        out.write_all(iso)?;
        self.append(iso, out, &mut |_, _| {})?;
        for r in self
            .regions
            .iter()
            .chain(self.rewards.unscaled(iso)?.iter())
        {
            out.seek(SeekFrom::Start(r.offset))?;
            out.write_all(&r.bytes)?;
        }
        Ok(())
    }
    pub fn append(
        &self,
        iso: &[u8],
        out: &mut impl Write,
        progress: &mut dyn FnMut(u64, u64),
    ) -> Result<()> {
        let total = self.output_size - self.append_offset as u64;
        let mut done = 0;
        for piece in &self.pieces {
            let bytes = match piece {
                Piece::Source(r) => &iso[r.clone()],
                Piece::Bytes(b) => b,
            };
            for chunk in bytes.chunks(8 << 20) {
                out.write_all(chunk)?;
                done += chunk.len() as u64;
                progress(done, total);
            }
        }
        need(done <= total, "rebuilt archive exceeds image")?;
        out.write_all(&vec![0; (total - done) as usize])?;
        Ok(())
    }
    pub fn verify(&self, iso: &[u8], out: &[u8]) -> Result<()> {
        need(
            out.len() as u64 == self.output_size,
            "expanded image length mismatch",
        )?;
        let mut pos = self.append_offset;
        for piece in &self.pieces {
            let bytes = match piece {
                Piece::Source(r) => &iso[r.clone()],
                Piece::Bytes(b) => b,
            };
            need(
                out.get(pos..pos + bytes.len()) == Some(bytes),
                "rebuilt archive differs from plan",
            )?;
            pos += bytes.len();
        }
        need(out[pos..].iter().all(|b| *b == 0), "nonzero image padding")?;
        for r in &self.regions {
            need(
                out.get(r.offset as usize..r.offset as usize + r.bytes.len()) == Some(&r.bytes),
                "ISO extent metadata mismatch",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod filename_tests {
    use super::*;

    #[test]
    fn accepts_both_afs_filename_pointer_locations_and_rejects_conflicts() {
        let count = 2;
        let first = 2048;
        let table = 4096;
        let length = count * 48;
        let mut afs = vec![0; table + length];
        assert_eq!(filename_table(&afs, count, first).unwrap(), None);
        for pos in [8 + 8 * count, first - 8] {
            let mut bytes = afs.clone();
            put(&mut bytes, pos, table as u32);
            put(&mut bytes, pos + 4, length as u32);
            assert_eq!(
                filename_table(&bytes, count, first).unwrap(),
                Some(table..table + length)
            );
        }
        for pos in [8 + 8 * count, first - 8] {
            put(&mut afs, pos, table as u32);
            put(&mut afs, pos + 4, length as u32);
        }
        assert!(filename_table(&afs, count, first).is_ok());
        put(&mut afs, first - 8, table as u32 - 4);
        assert!(filename_table(&afs, count, first).is_err());
        put(&mut afs, first - 8, table as u32 + 4);
        assert!(filename_table(&afs, count, first).is_err());
    }
}
