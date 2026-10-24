//! Per-spawn rewards without global changes to a replacement enemy's EXP.
//! Each localized AREA pack owns its enemy database. Its unfilled rows can
//! represent a combat variant with the original spawn's reward independently.
use super::*;
use crate::{
    region::WriteRegion,
    table::{EnemyTable, EXP_COLUMN},
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Variant {
    pub combat: usize,
    pub reward: usize,
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
    /// generators equivalent unscaled variants from each original local block.
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
                bytes[row * stride..(row + 1) * stride]
                    .copy_from_slice(&base[variant.combat * stride..(variant.combat + 1) * stride]);
                if block.index == 1 {
                    let to = row * 24 + EXP_COLUMN * 2;
                    let from = variant.reward * 24 + EXP_COLUMN * 2;
                    bytes[to..to + 2].copy_from_slice(&base[from..from + 2]);
                }
            }
            regions.push(WriteRegion { offset, bytes });
        }
        Ok(regions)
    }

    /// Merge variants into full-block writes. Read both combat and EXP from
    /// the final transformed table, so multipliers, exclusions and caps apply
    /// exactly once, in the same way as the unrandomized original encounter.
    pub(crate) fn apply(
        &self,
        regions: &mut [WriteRegion],
        table: &EnemyTable,
    ) -> Result<Vec<WriteRegion>> {
        let base = table.blocks();
        let strides = [4, 24, 3];
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
            let stride = strides[block.index];
            for (&row, variant) in &block.rows {
                need(
                    row < table.len()
                        && variant.combat < table.len()
                        && variant.reward < table.len(),
                    "invalid reward table row",
                )?;
                region.bytes[row * stride..(row + 1) * stride].copy_from_slice(
                    &base[block.index][variant.combat * stride..(variant.combat + 1) * stride],
                );
                if block.index == 1 {
                    let at = row * 24 + EXP_COLUMN * 2;
                    region.bytes[at..at + 2]
                        .copy_from_slice(&table.stat[variant.reward][EXP_COLUMN].to_le_bytes());
                }
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
            assert_eq!(
                &regions[0].bytes[clone * 4..clone * 4 + 4],
                &100i32.to_le_bytes()
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
