//! Versioned identification inside the logical disc image, outside all game files.
//! This is a checksummed metadata marker, not cryptographic publisher authentication.
use crate::{
    error::{Error, Result},
    plan::PatchPlan,
    release::Release,
};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const SIZE: usize = 2048;
pub const MAGIC: &[u8; 32] = b"DW4_CROWN_MOD_SIGNATURE\0\0\0\0\0\0\0\0\0";
const HEADER: usize = 64;
const SEARCH: usize = 2 * 1024 * 1024 + SIZE;
fn bad(message: &str) -> Error {
    Error::VerificationFailed(format!("Crown Mod signature: {message}"))
}

pub fn metadata(release: Release, plan: &PatchPlan) -> Value {
    json!({
        "formatVersion": 1, "id": "dw4-crown-mod", "version": env!("CARGO_PKG_VERSION"),
        "platform": if release.is_gamecube() { "gc" } else { "ps2" }, "release": release.serial(),
        "options": {
            "fourPlayerSpawns": plan.four_player_spawns, "betterLoot": plan.crown_drops,
            "randomCrownColours": plan.random_crown_colours, "crownColourSeed": plan.crown_colour_seed,
            "randomEnemies": plan.random_enemies, "enemySeed": plan.enemy_seed,
            "crownRank": plan.crown_rank, "hpMultiplier": plan.hp_mult, "statMultipliers": plan.stat_mult,
            "criticalMultiplier": plan.crit_mult, "paralysisMultiplier": plan.para_mult,
            "excludeDestructibles": plan.exclude_destructibles, "excludeTripwire": plan.exclude_tripwire,
            "excludePractice": plan.exclude_practice, "practiceBuff": plan.practice_buff,
            "practiceSpeedMultiplier": plan.practice_speed_mult, "collapseToTop": plan.collapse_to_top,
            "forceVeryHard": plan.force_very_hard, "serialOverride": plan.serial
        }
    })
}

pub fn encode(metadata: &Value) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(metadata).map_err(|_| bad("invalid metadata"))?;
    if payload.len() > SIZE - HEADER {
        return Err(bad("metadata exceeds one sector"));
    }
    let mut block = vec![0; SIZE];
    block[..32].copy_from_slice(MAGIC);
    block[32..36].copy_from_slice(&1u32.to_le_bytes());
    block[36..40].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    block[40..60].copy_from_slice(&Sha1::digest(&payload));
    block[HEADER..HEADER + payload.len()].copy_from_slice(&payload);
    Ok(block)
}

pub fn decode(block: &[u8]) -> Result<Option<Value>> {
    if block.get(..32) != Some(MAGIC) {
        return Ok(None);
    }
    if block.len() != SIZE || u32::from_le_bytes(block[32..36].try_into().unwrap()) != 1 {
        return Err(bad("unsupported or truncated format"));
    }
    let len = u32::from_le_bytes(block[36..40].try_into().unwrap()) as usize;
    if len == 0 || len > SIZE - HEADER {
        return Err(bad("invalid payload length"));
    }
    let payload = &block[HEADER..HEADER + len];
    if Sha1::digest(payload).as_slice() != &block[40..60] {
        return Err(bad("checksum mismatch"));
    }
    let value: Value = serde_json::from_slice(payload).map_err(|_| bad("invalid JSON"))?;
    if value["id"] != "dw4-crown-mod"
        || value["formatVersion"] != 1
        || !value["version"].is_string()
        || !value["options"].is_object()
    {
        return Err(bad("invalid metadata identity"));
    }
    Ok(Some(value))
}

/// Accept sector padding added by GameCube container conversion, without scanning the whole disc.
pub fn find(tail: &[u8]) -> Result<Option<Value>> {
    if tail.len() < SIZE {
        return Ok(None);
    }
    for sector in (0..tail.len() / SIZE).rev() {
        let offset = sector * SIZE;
        if tail[offset..offset + 32] == MAGIC[..] {
            return decode(&tail[offset..offset + SIZE]);
        }
    }
    Ok(None)
}

/// Bounded detection in an already memory-mapped image.
pub fn from_image(image: &[u8]) -> Result<Option<Value>> {
    let start = image.len().saturating_sub(SEARCH) / SIZE * SIZE;
    find(&image[start..])
}

/// Fast raw-image detection: read at most the final 2 MiB plus one sector.
/// For CISO/RVZ use the logical disc bytes after container decoding.
pub fn read_raw(path: &Path) -> Result<Option<Value>> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let start = size.saturating_sub(SEARCH as u64) / SIZE as u64 * SIZE as u64;
    file.seek(SeekFrom::Start(start))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)?;
    find(&tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifies_version_release_and_options_with_padding() {
        for release in [
            Release::Usa,
            Release::Europe,
            Release::Japan,
            Release::GcUsa,
            Release::GcJapan,
        ] {
            let plan = PatchPlan {
                four_player_spawns: true,
                crown_drops: true,
                random_crown_colours: true,
                crown_colour_seed: Some(123),
                ..PatchPlan::default()
            };
            let value = metadata(release, &plan);
            let block = encode(&value).unwrap();
            let mut padded = vec![0; SIZE];
            padded.extend(&block);
            padded.extend(vec![0; SIZE * 2]);
            assert_eq!(find(&padded).unwrap(), Some(value));
        }
        assert_eq!(find(&vec![0; SIZE * 3]).unwrap(), None);
    }

    #[test]
    fn records_the_enemy_seed_and_still_fits_one_sector() {
        let plan = PatchPlan {
            four_player_spawns: true,
            crown_drops: true,
            random_crown_colours: true,
            crown_colour_seed: Some(u32::MAX),
            random_enemies: true,
            enemy_seed: Some(u32::MAX),
            serial: Some("SLUS_208.36".into()),
            ..PatchPlan::default()
        };
        let mut value = metadata(Release::Usa, &plan);
        assert_eq!(value["options"]["randomEnemies"], true);
        assert_eq!(value["options"]["enemySeed"], u32::MAX);
        value["randomEnemies"] = json!({
            "rulesVersion": 99, "seed": u32::MAX, "donorsPerStage": 99, "stages": 9999,
            "generators": 99999, "keptOriginal": 99999, "modelsCopied": 99999,
            "approximateEffects": 99999, "undrawnEffects": 99999, "planDigest": "0123456789abcdef",
        });
        value["recovery"] = json!({
            "formatVersion": 1, "offset": 2_179_188_736u64, "length": 99_999_999u64,
            "sha1": "0123456789abcdef0123456789abcdef01234567", "originalSize": 2_179_188_736u64,
            "regionCount": 99999, "archiveEnd": 2_179_188_736u64,
        });
        let block = encode(&value).expect("worst case fits one sector");
        assert_eq!(decode(&block).unwrap(), Some(value));
    }
    #[test]
    fn refuses_corrupted_and_future_records() {
        let block = encode(&metadata(Release::Usa, &PatchPlan::default())).unwrap();
        let mut corrupt = block.clone();
        corrupt[100] ^= 1;
        assert!(decode(&corrupt).is_err());
        let mut future = block.clone();
        future[32] = 2;
        assert!(decode(&future).is_err());
        assert!(decode(&block[..100]).is_err());
        let mut oversized = block;
        oversized[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&oversized).is_err());
    }
}
