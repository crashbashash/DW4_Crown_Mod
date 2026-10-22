//! Embedded original bytes allow rebuilding from a clean base without stacking patches.
use crate::{
    container::{self, Format},
    error::{Error, Result},
    patch::{patch_file, PatchOptions, PatchOutcome},
    plan::PatchPlan,
    region::WriteRegion,
    writer::{Phase, Progress},
};
use memmap2::Mmap;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
const LIMIT: u64 = 128 << 20;
fn scaled(phase: Phase, done: u64, total: u64, start: u64, end: u64) -> Progress {
    Progress {
        phase,
        done: start
            + if total == 0 {
                0
            } else {
                ((end - start) as u128 * done.min(total) as u128 / total as u128) as u64
            },
        total: 10000,
    }
}
fn bad(message: &str) -> Error {
    Error::VerificationFailed(format!("Crown Mod update: {message}"))
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

// Compressed binary: magic, original size, SHA-1, count, offset/length/original bytes.
pub(crate) fn capture(
    iso: &[u8],
    regions: &[WriteRegion],
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<u8>> {
    let mut raw = b"DW4UNDO1".to_vec();
    raw.extend((iso.len() as u64).to_le_bytes());
    let mut hash = Sha1::new();
    progress(0, 10000);
    for (index, chunk) in iso.chunks(8 << 20).enumerate() {
        hash.update(chunk);
        let done = ((index + 1) * (8 << 20)).min(iso.len());
        progress(9000 * done as u64 / iso.len() as u64, 10000);
    }
    raw.extend(hash.finalize());
    raw.extend((regions.len() as u32).to_le_bytes());
    for (index, region) in regions.iter().enumerate() {
        let start = usize::try_from(region.offset).map_err(|_| bad("invalid change offset"))?;
        let end = start
            .checked_add(region.bytes.len())
            .ok_or_else(|| bad("invalid change length"))?;
        let original = iso
            .get(start..end)
            .ok_or_else(|| bad("change outside original image"))?;
        raw.extend(region.offset.to_le_bytes());
        raw.extend(
            u32::try_from(original.len())
                .map_err(|_| bad("change too large"))?
                .to_le_bytes(),
        );
        raw.extend(original);
        if index % 100 == 0 || index + 1 == regions.len() {
            progress(
                9000 + 800 * (index + 1) as u64 / regions.len() as u64,
                10000,
            );
        }
    }
    if raw.len() as u64 > LIMIT {
        return Err(bad("change manifest too large"));
    }
    let compressed = zstd::stream::encode_all(raw.as_slice(), 3)?;
    progress(10000, 10000);
    Ok(compressed)
}
fn logical<T>(path: &Path, f: impl FnOnce(&Path) -> Result<T>) -> Result<T> {
    match container::detect(path)? {
        Format::Raw => f(path),
        format => {
            let work = tempfile::tempdir()?;
            let raw = work.path().join("disc.iso");
            container::unpack(format, path, &raw, &mut |_, _| {})?;
            f(&raw)
        }
    }
}
pub fn metadata(path: &Path) -> Result<Option<Value>> {
    logical(path, crate::signature::read_raw)
}

/// Read the actual platform/release and cross-check any embedded mod identity.
pub fn identify(path: &Path) -> Result<(crate::release::Release, Option<Value>)> {
    logical(path, |raw| {
        let file = File::open(raw)?;
        // Safety: identification only reads the selected image.
        let image = unsafe { Mmap::map(&file)? };
        let release = crate::release::detect(&image)?;
        let metadata = crate::signature::from_image(&image)?;
        if let Some(ref value) = metadata {
            if value["release"] != release.serial()
                || value["platform"] != if release.is_gamecube() { "gc" } else { "ps2" }
            {
                return Err(bad("signature release does not match disc"));
            }
        }
        Ok((release, metadata))
    })
}
fn restore(
    iso: &[u8],
    metadata: &Value,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    let descriptor = &metadata["recovery"];
    if descriptor["formatVersion"] != 1 {
        return Err(bad(
            "this image needs its original disc image for its first update",
        ));
    }
    let number = |key: &str| {
        descriptor[key]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| bad("invalid change manifest"))
    };
    let start = number("offset")?;
    let end = start
        .checked_add(number("length")?)
        .ok_or_else(|| bad("invalid change manifest length"))?;
    let compressed = iso
        .get(start..end)
        .ok_or_else(|| bad("truncated change manifest"))?;
    if descriptor["sha1"].as_str() != Some(digest(compressed).as_str()) {
        return Err(bad("change manifest checksum mismatch"));
    }
    let mut raw = Vec::new();
    zstd::stream::read::Decoder::new(compressed)?
        .take(LIMIT + 1)
        .read_to_end(&mut raw)?;
    if raw.len() as u64 > LIMIT || raw.len() < 40 || &raw[..8] != b"DW4UNDO1" {
        return Err(bad("invalid change manifest data"));
    }
    let size = u64::from_le_bytes(raw[8..16].try_into().unwrap());
    if size > start as u64 || descriptor["originalSize"].as_u64() != Some(size) {
        return Err(bad("invalid original size"));
    }
    let count = u32::from_le_bytes(raw[36..40].try_into().unwrap());
    if descriptor["regionCount"].as_u64() != Some(count as u64) {
        return Err(bad("invalid change count"));
    }
    let mut out = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(dest)?;
    progress(0, 10000);
    for (index, chunk) in iso[..size as usize].chunks(8 << 20).enumerate() {
        out.write_all(chunk)?;
        let done = ((index + 1) * (8 << 20)).min(size as usize);
        progress(5000 * done as u64 / size, 10000);
    }
    let mut cursor = 40usize;
    for _ in 0..count {
        let header = raw
            .get(cursor..cursor + 12)
            .ok_or_else(|| bad("truncated change entry"))?;
        let offset = u64::from_le_bytes(header[..8].try_into().unwrap());
        let length = u32::from_le_bytes(header[8..].try_into().unwrap()) as usize;
        cursor += 12;
        if offset
            .checked_add(length as u64)
            .is_none_or(|end| end > size)
        {
            return Err(bad("change outside original image"));
        }
        let bytes = raw
            .get(cursor..cursor + length)
            .ok_or_else(|| bad("truncated original bytes"))?;
        out.seek(SeekFrom::Start(offset))?;
        out.write_all(bytes)?;
        cursor += length;
    }
    if cursor != raw.len() {
        return Err(bad("unexpected change manifest data"));
    }
    out.sync_all()?;
    // Safety: private restored image is no longer being written.
    let mapped = unsafe { Mmap::map(&out)? };
    let mut hash = Sha1::new();
    for (index, chunk) in mapped.chunks(8 << 20).enumerate() {
        hash.update(chunk);
        let done = ((index + 1) * (8 << 20)).min(mapped.len());
        progress(5000 + 5000 * done as u64 / size, 10000);
    }
    if hash.finalize().as_slice() != &raw[16..36] {
        return Err(bad(
            "restored image checksum mismatch; the modded image may have been edited",
        ));
    }
    Ok(())
}

/// Verify a private rebuild, then replace the selected image at the same path.
pub fn update_image(
    path: &Path,
    original: Option<&Path>,
    plan: &PatchPlan,
    progress: &mut dyn FnMut(Progress),
) -> Result<PatchOutcome> {
    let target = fs::canonicalize(path)?;
    let before = crate::writer::md5_hex_with_progress(&target, &mut |d, t| {
        progress(scaled(Phase::Inspecting, d, t, 0, 500))
    })?;
    let format = container::detect(&target)?;
    let work = tempfile::Builder::new()
        .prefix(".dw4-update-")
        .tempdir_in(target.parent().unwrap())?;
    let baseline = work.path().join("original.iso");
    let raw_selected = if format == Format::Raw {
        target.clone()
    } else {
        let raw = work.path().join("selected.iso");
        container::unpack(format, &target, &raw, &mut |d, t| {
            progress(scaled(Phase::Inspecting, d, t, 500, 1000))
        })?;
        raw
    };
    progress(scaled(Phase::Inspecting, 1, 1, 500, 1000));
    let previous = (|raw: &Path| {
        let file = File::open(raw)?;
        // Safety: selected image is opened read-only.
        let iso = unsafe { Mmap::map(&file)? };
        let metadata = crate::signature::from_image(&iso)?;
        if let Some(ref metadata) = metadata {
            let release = crate::release::detect(&iso)?;
            if metadata["release"] != release.serial()
                || metadata["platform"] != if release.is_gamecube() { "gc" } else { "ps2" }
            {
                return Err(bad("signature release does not match disc"));
            }
            if !metadata["recovery"].is_null() {
                restore(&iso, metadata, &baseline, &mut |d, t| {
                    progress(scaled(Phase::Inspecting, d, t, 1000, 2500))
                })?;
                return Ok((release, metadata.clone()));
            }
        } else if !matches!(
            crate::disc::inspect_retail_bytes(&iso),
            Err(Error::AlreadyModded { .. })
        ) {
            return Err(bad("select a Crown Mod image to update"));
        }
        let release = crate::release::detect(&iso)?;
        let original =
            original.ok_or_else(|| bad("choose the original disc image for this older build"))?;
        logical(original, |raw| {
            let report = crate::disc::inspect_retail(raw)?;
            if report.release != Some(release) {
                return Err(bad(
                    "the original disc must match the selected image's platform and release",
                ));
            }
            fs::copy(raw, &baseline)?;
            Ok(())
        })?;
        Ok((release, metadata.unwrap_or(Value::Null)))
    })(&raw_selected)?;
    if format != Format::Raw {
        fs::remove_file(raw_selected)?;
    }
    progress(scaled(Phase::Inspecting, 1, 1, 1000, 2500));
    {
        let file = File::open(&baseline)?;
        // Safety: the private base is complete and read-only during detection.
        let image = unsafe { Mmap::map(&file)? };
        if crate::release::detect(&image)? != previous.0 {
            return Err(bad("restored release mismatch"));
        }
    }
    let mut plan = plan.clone();
    if plan.random_crown_colours && plan.crown_colour_seed.is_none() {
        plan.crown_colour_seed = previous.1["options"]["crownColourSeed"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok());
    }
    if plan.random_enemies && plan.enemy_seed.is_none() {
        plan.enemy_seed = previous.1["options"]["enemySeed"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok());
    }
    let raw_out = work.path().join("rebuilt.iso");
    let mut outcome = patch_file(
        &baseline,
        &raw_out,
        &plan,
        &PatchOptions { overwrite: false },
        &mut |p| progress(scaled(p.phase, p.done, p.total, 2500, 7500)),
    )?;
    let staged = if format == Format::Raw {
        raw_out
    } else {
        let packed = work.path().join(format!("rebuilt.{}", format.extension()));
        container::pack(format, &raw_out, &packed, &mut |d, t| {
            progress(scaled(Phase::Writing, d, t, 7500, 8500))
        })?;
        let check = work.path().join("check.iso");
        container::unpack(format, &packed, &check, &mut |d, t| {
            progress(scaled(Phase::Verifying, d, t, 8500, 9000))
        })?;
        if !crate::patch::same_image(&raw_out, &check, &mut |d, t| {
            progress(scaled(Phase::Verifying, d, t, 9000, 9500))
        })? {
            return Err(bad("packed image verification failed"));
        }
        outcome.bytes = fs::metadata(&packed)?.len();
        outcome.md5 = crate::writer::md5_hex_with_progress(&packed, &mut |_, _| {})?;
        packed
    };
    OpenOptions::new().write(true).open(&staged)?.sync_all()?;
    fs::set_permissions(&staged, fs::metadata(&target)?.permissions())?;
    if crate::writer::md5_hex_with_progress(&target, &mut |d, t| {
        progress(scaled(Phase::Verifying, d, t, 9500, 9900))
    })? != before
    {
        return Err(bad(
            "selected image changed during the update; please select it again",
        ));
    }
    fs::rename(staged, &target)?;
    progress(scaled(Phase::Verifying, 1, 1, 9900, 10000));
    Ok(outcome)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_uses_the_disc_header_and_refuses_mismatched_signatures() {
        use crate::release::Release;
        let mut image = crate::testkit::iso_with_elf_and_cnf(Release::Usa.serial());
        let work = tempfile::tempdir().unwrap();
        let path = work.path().join("selected.iso");
        fs::write(&path, &image).unwrap();
        assert_eq!(identify(&path).unwrap(), (Release::Usa, None));
        image.resize(image.len().div_ceil(2048) * 2048, 0);
        let mut signed = image.clone();
        signed.extend(
            crate::signature::encode(&crate::signature::metadata(
                Release::Usa,
                &PatchPlan::default(),
            ))
            .unwrap(),
        );
        fs::write(&path, &signed).unwrap();
        assert_eq!(identify(&path).unwrap().0, Release::Usa);
        assert!(identify(&path).unwrap().1.is_some());
        image.extend(
            crate::signature::encode(&crate::signature::metadata(
                Release::GcUsa,
                &PatchPlan::default(),
            ))
            .unwrap(),
        );
        fs::write(&path, &image).unwrap();
        assert!(identify(&path).is_err());
    }

    #[test]
    fn restore_checks_original_bytes_and_rejects_damage() {
        let original = vec![7u8; 8192];
        let regions = vec![
            WriteRegion {
                offset: 100,
                bytes: vec![9; 400],
            },
            WriteRegion {
                offset: 1000,
                bytes: vec![2; 24],
            },
        ];
        let mut progress = Vec::new();
        let recovery = capture(&original, &regions, &mut |d, t| progress.push((d, t))).unwrap();
        assert_eq!(progress.first(), Some(&(0, 10000)));
        assert_eq!(progress.last(), Some(&(10000, 10000)));
        assert!(progress.windows(2).all(|p| p[0].0 <= p[1].0));
        assert!(progress.iter().any(|&(d, _)| d > 0 && d < 10000));
        let mut iso = original.clone();
        for region in &regions {
            iso[region.offset as usize..region.offset as usize + region.bytes.len()]
                .copy_from_slice(&region.bytes);
        }
        iso.extend(&recovery);
        let metadata = serde_json::json!({"recovery": {"formatVersion": 1, "offset": original.len(), "length": recovery.len(), "sha1": digest(&recovery), "originalSize": original.len(), "regionCount": regions.len()}});
        let work = tempfile::tempdir().unwrap();
        let dest = work.path().join("original.iso");
        restore(&iso, &metadata, &dest, &mut |_, _| {}).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), original);
        fs::remove_file(&dest).unwrap();
        iso[5000] ^= 1;
        assert!(restore(&iso, &metadata, &dest, &mut |_, _| {}).is_err());
        fs::remove_file(&dest).unwrap();
        iso[8192] ^= 1;
        assert!(restore(&iso, &metadata, &dest, &mut |_, _| {}).is_err());
    }
}
