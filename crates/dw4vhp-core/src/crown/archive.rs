//! Compose resource edits, rebuild AFS and retain verified disc/recovery recipes.
use super::*;

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
}
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
) -> Result<Rebuilt> {
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
            Some(random::Randomiser::scan(
                afs,
                &entries,
                crate::table::EnemyTable::read(iso, &layout)?,
                options,
            )?)
        }
        None => None,
    };
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
                    r.stage(i, current, afs, &entries)
                        .map_err(|e| bad(&format!("randomise entry {i}: {e}")))?
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
    }
    if let Some(r) = random.as_mut() {
        for (i, bytes) in std::mem::take(&mut r.sounds) {
            entry_edits[i] = vec![Edit {
                range: 0..entries[i].1,
                bytes,
            }];
        }
    }
    for (i, ((off, len), edits)) in entries.iter().copied().zip(entry_edits).enumerate() {
        let next = entries.get(i + 1).map_or(size, |e| e.0);
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
    pieces.insert(0, Piece::Bytes(prefix));
    if let Some(r) = &random {
        if std::env::var_os("DW4_RANDOM_LOG").is_some() {
            for line in &r.log {
                eprintln!("{line}");
            }
        }
    }
    let rewards = random
        .as_ref()
        .map(|r| r.reward_plan(iso))
        .transpose()?
        .unwrap_or_default();
    Ok(Rebuilt {
        pieces,
        length: cursor,
        random: random.as_ref().map(random::Randomiser::summary),
        rewards,
    })
}

impl CrownPlan {
    pub fn build(iso: &[u8]) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, None, None)
    }
    /// Experimental: the usual crown expansion plus an enemy model graft.
    pub fn build_graft(iso: &[u8], spec: &graft::Spec) -> Result<Self> {
        Self::build_with_options(iso, false, false, false, 0, Some(spec), None)
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
    ) -> Result<Self> {
        let file = iso9660::find_file(iso, "NDMW.AFS")?;
        let base = file.lba as usize * 2048;
        let size = file.size as usize;
        let Rebuilt {
            pieces,
            length: cursor,
            random,
            rewards,
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
        )?;
        let sectors = u32at(iso, 16 * 2048 + 80)? as usize;
        let append_offset = sectors * 2048;
        need(append_offset == iso.len(), "unexpected trailing ISO data")?;
        let output_size = a2048(append_offset + cursor);
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
    ) -> Result<Self> {
        let file = gcm::find_file(disc, "ndmw.afs")?;
        let (base, size) = (file.offset as usize, file.size as usize);
        let Rebuilt {
            pieces,
            length: cursor,
            random,
            rewards,
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
        )?;
        let append_offset = disc.len();
        need(
            append_offset.is_multiple_of(4),
            "unaligned GameCube image length",
        )?;
        need(
            (append_offset + cursor) as u64 <= u32::MAX as u64,
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
        regions.extend(rewards.models.clone());
        Ok(Self {
            pieces,
            rewards,
            regions,
            output_size: (append_offset + cursor) as u64,
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
