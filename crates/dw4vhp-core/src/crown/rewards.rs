//! Per-spawn balance without global changes to the replacement enemy's native rows.
//! Each localized AREA pack owns its enemy database. Its unfilled rows can
//! carry the replacement model with the original encounter's stats and rewards.
use super::*;
use crate::{region::WriteRegion, table::EnemyTable};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Variant {
    /// Supplies the model and bounded elemental resistance differences only.
    pub combat: usize,
    /// Original encounter: supplies all other stats, including speed and EXP.
    pub reward: usize,
}

impl Variant {
    /// Both callers supply final table bytes: authored for the archive-only
    /// writer, transformed once for the application. Never multiply here.
    fn write(self, index: usize, base: &[u8], row: usize, bytes: &mut [u8]) {
        let stride = [4, 24, 3][index];
        bytes[row * stride..(row + 1) * stride]
            .copy_from_slice(&base[self.reward * stride..(self.reward + 1) * stride]);
        if index == 1 {
            // Fire, ice, thunder and dark only. Stun and poison stay native.
            for column in 5..9 {
                let read = |source| {
                    let at = source * 24 + column * 2;
                    i16::from_le_bytes(base[at..at + 2].try_into().unwrap()) as i32
                };
                let original = read(self.reward);
                let resistance = read(self.combat).clamp(original - 100, original + 100) as i16;
                let at = row * 24 + column * 2;
                bytes[at..at + 2].copy_from_slice(&resistance.to_le_bytes());
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct StageRewards {
    pub generators: Vec<usize>,
    pub rows: BTreeMap<usize, Variant>,
    pub targets: BTreeMap<usize, BTreeMap<String, u32>>,
    free: Vec<usize>,
}
impl StageRewards {
    pub fn new(generators: Vec<usize>, mut free: Vec<usize>) -> Self {
        free.reverse();
        Self {
            generators,
            rows: BTreeMap::new(),
            targets: BTreeMap::new(),
            free,
        }
    }
    pub fn assign(&mut self, combat: usize, reward: usize) -> Option<usize> {
        if combat == reward {
            return Some(combat);
        }
        let variant = Variant { combat, reward };
        if let Some((&row, _)) = self.rows.iter().find(|(_, v)| **v == variant) {
            return Some(row);
        }
        let row = self.free.pop()?;
        self.rows.insert(row, variant);
        Some(row)
    }
}

struct Block {
    index: usize,
    rows: BTreeMap<usize, Variant>,
}
#[derive(Default)]
pub(crate) struct Plan {
    blocks: BTreeMap<u64, Block>,
    /// Serialized MODEL object-ID lists, kept with the ordinary archive writes
    /// so recovery and byte-for-byte verification cover these references too.
    pub(super) models: Vec<WriteRegion>,
}
struct VirtualFile {
    part: u32,
    entry: usize,
    path: Vec<Vec<u8>>,
}
fn location(iso: &[u8], name: &str) -> Result<(usize, usize)> {
    if crate::release::detect(iso)?.is_gamecube() {
        let file = gcm::find_file(iso, name)?;
        Ok((file.offset as usize, file.size as usize))
    } else {
        let file = iso9660::find_file(iso, name)?;
        Ok((file.lba as usize * 2048, file.size as usize))
    }
}

/// Locate MODEL by following the ENEMY database containing the known stat
/// matrix, rather than searching for a platform-specific pointer byte pattern.
fn model_list(p: &Pack, stat: usize, rows: usize) -> Result<Range<usize>> {
    let mut found = Vec::new();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beDataBase").is_err() {
            continue;
        }
        let short = p.val(id, 5)?;
        if p.val(short, 2)? as usize != rows * 12 {
            continue;
        }
        let memory = p.val(short, 4)? as usize;
        if p.memory
            .get(memory)
            .and_then(Option::as_ref)
            .map(|r| r.start)
            != Some(stat)
        {
            continue;
        }
        let columns = p.array(p.val(id, 12)?)?;
        need(
            columns.len() == 1 && graft::name_of(p, p.rec(columns[0])?) == Some(b"MODEL"),
            "unexpected enemy model columns",
        )?;
        let list = p.val(id, 13)?;
        need(
            p.array(list)?.len() == rows,
            "unexpected enemy model row count",
        )?;
        let range = p
            .memory
            .get(p.val(list, 4)? as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing enemy model memory"))?;
        need(
            range.len() == rows * 4,
            "unexpected enemy model memory size",
        )?;
        found.push(range.clone());
    }
    need(found.len() == 1, "ambiguous enemy model database")?;
    Ok(found.remove(0))
}

impl Plan {
    pub(super) fn build(
        iso: &[u8],
        table: &EnemyTable,
        stages: &BTreeMap<usize, StageRewards>,
    ) -> Result<Self> {
        let mut plan = Self::default();
        if stages.is_empty() {
            return Ok(plan);
        }
        let (offset, size) = location(iso, "NDMW.IGB")?;
        let manifest = Pack::parse(
            iso.get(offset..offset + size)
                .ok_or_else(|| bad("manifest outside image"))?,
        )?;
        let mut files = Vec::new();
        for id in 0..manifest.ids.len() as u32 {
            if manifest.class(id, "igAdxAfsFile").is_err() {
                continue;
            }
            let list = manifest.val(id, 3)?;
            if list == NULL {
                continue;
            }
            let path = manifest
                .array(list)?
                .into_iter()
                .map(|node| {
                    graft::name_of(&manifest, manifest.rec(node)?)
                        .map(|s| s.to_vec())
                        .ok_or_else(|| bad("unnamed reward pack directory"))
                })
                .collect::<Result<Vec<_>>>()?;
            files.push(VirtualFile {
                part: manifest.val(id, 4)?,
                entry: manifest.val(id, 5)? as usize,
                path,
            });
        }
        let (area, size) = location(iso, "AREA.AFS")?;
        let afs = iso
            .get(area..area + size)
            .ok_or_else(|| bad("reward archive outside image"))?;
        need(afs.get(..4) == Some(b"AFS\0"), "invalid reward archive")?;
        let count = u32at(afs, 4)? as usize;
        need(
            count > 0 && count <= afs.len().saturating_sub(8) / 8,
            "invalid reward archive count",
        )?;
        let blocks = table.blocks();
        let mut model_writes = BTreeMap::new();
        for (stage, request) in stages {
            let paths: BTreeSet<_> = files
                .iter()
                .filter(|f| f.part == 0 && request.generators.contains(&f.entry))
                .map(|f| f.path.clone())
                .collect();
            need(
                paths.len() == 1,
                &format!("stage {stage}: ambiguous reward pack directory"),
            )?;
            let path = paths.first().unwrap();
            let mut found = BTreeSet::new();
            for file in files.iter().filter(|f| f.part == 1 && &f.path == path) {
                need(file.entry < count, "reward pack index outside archive")?;
                let offset = u32at(afs, 8 + 8 * file.entry)? as usize;
                let size = u32at(afs, 12 + 8 * file.entry)? as usize;
                let bytes = afs
                    .get(offset..offset + size)
                    .ok_or_else(|| bad("reward pack outside archive"))?;
                let positions: Vec<_> = memchr::memmem::find_iter(bytes, &blocks[1]).collect();
                if positions.is_empty() {
                    continue;
                }
                need(positions.len() == 1, "ambiguous reward stat block")?;
                let p = Pack::parse(bytes)?;
                let models = model_list(&p, positions[0], table.len())?;
                let mut model_bytes = bytes[models.clone()].to_vec();
                for (&row, variant) in &request.rows {
                    need(
                        row < table.len()
                            && table.hp[row] == 0
                            && variant.combat < table.len()
                            && variant.reward < table.len(),
                        "invalid local reward variant",
                    )?;
                    let source = u32at(bytes, models.start + variant.combat * 4)?;
                    need(
                        graft::name_of(&p, p.rec(source)?)
                            == random::family(variant.combat as u32).map(str::as_bytes),
                        "reward model differs from combat family",
                    )?;
                    put(&mut model_bytes, row * 4, source);
                }
                let at = (area + offset + models.start) as u64;
                if let Some(old) = model_writes.insert(at, model_bytes.clone()) {
                    need(old == model_bytes, "conflicting local reward models")?;
                }
                for (index, block) in blocks.iter().enumerate() {
                    let positions: Vec<_> = memchr::memmem::find_iter(bytes, block).collect();
                    need(positions.len() == 1, "ambiguous local enemy data block")?;
                    let at = (area + offset + positions[0]) as u64;
                    found.insert(at);
                    let target = plan.blocks.entry(at).or_insert_with(|| Block {
                        index,
                        rows: BTreeMap::new(),
                    });
                    need(
                        target.index == index,
                        "overlapping local reward data blocks",
                    )?;
                    for (&row, &variant) in &request.rows {
                        if let Some(old) = target.rows.insert(row, variant) {
                            need(old == variant, "conflicting local reward variants")?;
                        }
                    }
                }
            }
            need(
                !found.is_empty(),
                &format!("stage {stage}: no local reward tables"),
            )?;
        }
        plan.models = model_writes
            .into_iter()
            .map(|(offset, bytes)| WriteRegion { offset, bytes })
            .collect();
        Ok(plan)
    }

    /// The experimental archive-only writer has no stat transform. Give its
    /// generators balanced variants from each original local block.
    pub(super) fn unscaled(&self, iso: &[u8]) -> Result<Vec<WriteRegion>> {
        let mut regions = Vec::new();
        if self.blocks.is_empty() {
            return Ok(regions);
        }
        let row_count = crate::release::detect(iso)?.layout_in(iso)?.rows;
        for (&offset, block) in &self.blocks {
            let stride = [4, 24, 3][block.index];
            let start = offset as usize;
            let base = iso
                .get(start..start + row_count * stride)
                .ok_or_else(|| bad("local reward block outside image"))?;
            let mut bytes = base.to_vec();
            for (&row, variant) in &block.rows {
                variant.write(block.index, base, row, &mut bytes);
            }
            regions.push(WriteRegion { offset, bytes });
        }
        Ok(regions)
    }

    /// Merge variants into full-block writes. Read encounter stats and donor
    /// resistances from the final table, so multipliers, exclusions and caps apply
    /// exactly once, in the same way as the unrandomized original encounter.
    pub(crate) fn apply(
        &self,
        regions: &mut [WriteRegion],
        table: &EnemyTable,
    ) -> Result<Vec<WriteRegion>> {
        let base = table.blocks();
        let mut seen = BTreeSet::new();
        let mut changed = Vec::new();
        for region in regions {
            let Some(block) = self.blocks.get(&region.offset) else {
                continue;
            };
            need(seen.insert(region.offset), "duplicate reward table write")?;
            need(
                region.bytes == base[block.index],
                "reward write differs from computed table",
            )?;
            for (&row, variant) in &block.rows {
                need(
                    row < table.len()
                        && variant.combat < table.len()
                        && variant.reward < table.len(),
                    "invalid reward table row",
                )?;
                variant.write(block.index, &base[block.index], row, &mut region.bytes);
            }
            if region.bytes != base[block.index] {
                changed.push(region.clone());
            }
        }
        need(
            seen.len() == self.blocks.len(),
            "a local reward table has no planned write",
        )?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::EXP_COLUMN;

    #[test]
    fn same_combat_variant_can_preserve_two_different_original_rewards() {
        let mut stage = StageRewards::new(vec![615], vec![2, 3]);
        assert_eq!(stage.assign(0, 0), Some(0));
        assert_eq!(stage.assign(0, 1), Some(2));
        assert_eq!(stage.assign(0, 1), Some(2));
        assert_eq!(stage.assign(0, 4), Some(3));
        assert_eq!(stage.assign(1, 4), None);
        let mut table = EnemyTable {
            hp: vec![100, 500, 0, 0, 900],
            stat: vec![[10; 12]; 5],
            crit: vec![1; 5],
            para: vec![2; 5],
            rarity: vec![5; 5],
        };
        table.crit[2] = 0;
        table.crit[3] = 0;
        table.stat[0][EXP_COLUMN] = 58;
        table.stat[1][EXP_COLUMN] = 496;
        table.stat[4][EXP_COLUMN] = 20; // Lower rewards are preserved too.
        let mut plan = Plan::default();
        let mut regions = Vec::new();
        for (index, bytes) in table.blocks().into_iter().enumerate() {
            let offset = 100 + index as u64 * 1000;
            plan.blocks.insert(
                offset,
                Block {
                    index,
                    rows: stage.rows.clone(),
                },
            );
            regions.push(WriteRegion { offset, bytes });
        }
        let unrelated = WriteRegion {
            offset: 10000,
            bytes: table.blocks()[1].clone(),
        };
        regions.push(unrelated.clone());
        assert_eq!(plan.apply(&mut regions, &table).unwrap().len(), 3);
        assert_eq!(regions[3], unrelated);
        for clone in [2, 3] {
            let original = if clone == 2 { 1 } else { 4 };
            assert_eq!(
                &regions[0].bytes[clone * 4..clone * 4 + 4],
                &table.hp[original].to_le_bytes()
            );
            assert_eq!(
                &regions[1].bytes[clone * 24..clone * 24 + 22],
                &table.blocks()[1][..22]
            );
            let reward = if clone == 2 { 496i16 } else { 20i16 };
            assert_eq!(
                &regions[1].bytes[clone * 24 + 22..clone * 24 + 24],
                &reward.to_le_bytes()
            );
        }
        assert!(plan.apply(&mut [], &table).is_err());
    }

    #[test]
    fn original_encounter_controls_combat_with_only_bounded_elemental_differences() {
        let table = EnemyTable {
            hp: vec![805, 58, 0],
            stat: vec![
                [
                    4461,
                    144,
                    3122,
                    227,
                    492,
                    650,
                    0,
                    320,
                    i16::MAX,
                    22,
                    400,
                    404,
                ],
                [
                    18,
                    90,
                    13,
                    74,
                    108,
                    450,
                    250,
                    300,
                    i16::MAX - 20,
                    10,
                    150,
                    18,
                ],
                [0; 12],
            ],
            crit: vec![18, 4, 0],
            para: vec![18, 4, 0],
            rarity: vec![0, 0, 0],
        };
        let variant = Variant {
            combat: 0,
            reward: 1,
        };
        let base = table.blocks();
        let mut blocks = base.clone();
        for (index, bytes) in blocks.iter_mut().enumerate() {
            variant.write(index, &base[index], 2, bytes);
        }
        assert_eq!(&blocks[0][8..12], &58i32.to_le_bytes());
        let stats: Vec<_> = blocks[1][48..72]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect();
        assert_eq!(
            stats,
            [18, 90, 13, 74, 108, 550, 150, 320, i16::MAX, 10, 150, 18]
        );
        assert_eq!(&blocks[2][6..9], &[4, 4, 0]);
        // Native rows remain untouched, including the much stronger donor.
        for (index, stride) in [4, 24, 3].into_iter().enumerate() {
            assert_eq!(&blocks[index][..2 * stride], &base[index][..2 * stride]);
        }
    }

    #[test]
    fn localized_variant_databases_resolve_in_every_release() {
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
            let image = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let layout = crate::release::detect(&image)
                .unwrap()
                .layout_in(&image)
                .unwrap();
            let table = EnemyTable::read(&image, &layout).unwrap();
            let mut stage = StageRewards::new(vec![615, 616, 617], vec![66, 69]);
            assert_eq!(stage.assign(127, 0), Some(66));
            assert_eq!(stage.assign(127, 3), Some(69));
            let plan = Plan::build(&image, &table, &BTreeMap::from([(614, stage)])).unwrap();
            assert_eq!(plan.models.len(), 7);
            assert_eq!(plan.blocks.len(), 21);
            let (patched, _) = crate::plan::transform(
                &table,
                &crate::plan::PatchPlan::default(),
                &crate::ratio::RatioVector::derive(&table),
            );
            let before = patched.blocks();
            let mut regions = Vec::new();
            for (old, new) in table.blocks().iter().zip(&before) {
                regions.extend(
                    memchr::memmem::find_iter(&image, old).map(|offset| WriteRegion {
                        offset: offset as u64,
                        bytes: new.clone(),
                    }),
                );
            }
            assert_eq!(plan.apply(&mut regions, &patched).unwrap().len(), 21);
            let changed_stats: Vec<_> = regions
                .iter()
                .filter(|r| r.bytes.len() == before[1].len() && r.bytes != before[1])
                .collect();
            assert_eq!(changed_stats.len(), 7);
            for r in changed_stats {
                assert_eq!(&r.bytes[66 * 24 + 22..66 * 24 + 24], &146i16.to_le_bytes());
                assert_eq!(&r.bytes[69 * 24 + 22..69 * 24 + 24], &496i16.to_le_bytes());
                assert_eq!(&r.bytes[127 * 24..128 * 24], &before[1][127 * 24..128 * 24]);
            }
            eprintln!(
                "PASS {variable}: independent MR2 rewards 146 and 496; native MR2 row stays 58"
            );
        }
    }
}
