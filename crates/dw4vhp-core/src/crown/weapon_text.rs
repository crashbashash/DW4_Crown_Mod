//! Vanilla English text error: Dbl Axe, Rage Hammer, Hekatoncheir, Fang Smasher
//! and Meteor Force (weapon records 100-124) store the Blunt skill, which the
//! equip check uses, but their description says "Bash Skill". Every stage's
//! language packs carry their own weapon database; the US English text has the
//! error on every release (and the unused French slot on the European disc),
//! while the European English pack already says "Blunt Skill".
//!
//! "Blunt" is one byte longer than "Bash", so the first line break also loses
//! its carriage return. The font converter skips every `\r`, so the shop and
//! status windows draw the same text, and each string keeps its exact length.
use super::*;

const BLUNT: i64 = 0;
const WRONG: &[u8] = b"Equip. Skill Bash Skill ";
const RIGHT: &[u8] = b"Equip. Skill Blunt Skill ";
/// The mislabelled weapon records, the same in every release.
const ROWS: Range<usize> = 100..125;

/// The text writes for an AREA archive, as (offset in the archive, bytes).
fn edits(afs: &[u8]) -> Result<Vec<(usize, Vec<u8>)>> {
    need(afs.get(..4) == Some(b"AFS\0"), "invalid language archive")?;
    let count = u32at(afs, 4)? as usize;
    need(
        count > 0 && count <= afs.len().saturating_sub(8) / 8,
        "invalid language archive count",
    )?;
    let mut out = Vec::new();
    let mut packs = 0;
    for entry in 0..count {
        let offset = u32at(afs, 8 + 8 * entry)? as usize;
        let size = u32at(afs, 12 + 8 * entry)? as usize;
        let pack = afs
            .get(offset..offset + size)
            .ok_or_else(|| bad("language pack outside archive"))?;
        if memchr::memmem::find(pack, WRONG).is_none() {
            continue;
        }
        let fixed = fix_pack(pack).map_err(|e| bad(&format!("weapon text entry {entry}: {e}")))?;
        if fixed.is_empty() {
            continue;
        }
        packs += 1;
        out.extend(fixed.into_iter().map(|(at, bytes)| (offset + at, bytes)));
    }
    // Each release has one mislabelled English pack per stage at least.
    need(packs >= 95, "unexpected weapon text packs")?;
    Ok(out)
}

/// A database column's values, read from the list class's element width.
fn column(p: &Pack, list: u32) -> Result<Vec<i64>> {
    let width = match p.classes[p.rec(list)?.class].name.as_str() {
        "igCharList" | "igUnsignedCharList" => 1,
        "igShortList" | "igUnsignedShortList" => 2,
        "igIntList" | "igUnsignedIntList" => 4,
        _ => return Ok(Vec::new()),
    };
    let count = p.val(list, 2)? as usize;
    if count == 0 {
        return Ok(Vec::new());
    }
    let range = p
        .memory
        .get(p.val(list, 4)? as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing database column memory"))?;
    need(
        count * width <= range.len(),
        "database column exceeds memory",
    )?;
    let bytes = &p.b[range.start..range.start + count * width];
    Ok(bytes
        .chunks_exact(width)
        .map(|c| match width {
            1 => i64::from(c[0] as i8),
            2 => i64::from(i16::from_le_bytes([c[0], c[1]])),
            _ => i64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])),
        })
        .collect())
}

/// The weapon database's `SKILL` values and `TXT` string objects, by record.
fn weapons(p: &Pack) -> Result<Option<(Vec<i64>, Vec<u32>)>> {
    let mut found = None;
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beDBManagerInfo").is_err()
            || graft::name_of(p, p.rec(id)?) != Some(b"weapon")
        {
            continue;
        }
        need(found.is_none(), "duplicate weapon database")?;
        let db = p.val(id, 7)?;
        p.class(db, "beDataBase")?;
        let names = |field: u16| -> Result<Vec<Vec<u8>>> {
            let list = p.val(db, field)?;
            if list == NULL {
                return Ok(Vec::new());
            }
            p.array(list)?
                .into_iter()
                .map(|n| Ok(graft::name_of(p, p.rec(n)?).unwrap_or_default().to_vec()))
                .collect()
        };
        let mut skills = None;
        for field in [2, 4, 6] {
            let columns = names(field)?;
            let Some(index) = columns.iter().position(|c| c == b"SKILL") else {
                continue;
            };
            let values = column(p, p.val(db, field + 1)?)?;
            need(
                !values.is_empty() && values.len().is_multiple_of(columns.len()),
                "uneven weapon columns",
            )?;
            skills = Some(
                values
                    .chunks_exact(columns.len())
                    .map(|row| row[index])
                    .collect::<Vec<_>>(),
            );
        }
        let skills = skills.ok_or_else(|| bad("weapon database has no SKILL"))?;
        let columns = names(12)?;
        let index = columns
            .iter()
            .position(|c| c == b"TXT")
            .ok_or_else(|| bad("weapon database has no TXT"))?;
        let values = p.array(p.val(db, 13)?)?;
        need(
            values.len() == skills.len() * columns.len(),
            "weapon text and skill rows differ",
        )?;
        let texts = values
            .chunks_exact(columns.len())
            .map(|row| row[index])
            .collect();
        found = Some((skills, texts));
    }
    Ok(found)
}

/// Same-length writes, relative to `pack`, for every weapon description that
/// names Bash while the record stores Blunt.
fn fix_pack(pack: &[u8]) -> Result<Vec<(usize, Vec<u8>)>> {
    let p = Pack::parse(pack)?;
    let Some((skills, texts)) = weapons(&p)? else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    let mut out: Vec<(usize, Vec<u8>)> = Vec::new();
    for (row, (&skill, &text)) in skills.iter().zip(&texts).enumerate() {
        let field = p.rec(text)?.field(2)?;
        let start = field.start + 4;
        let len = pack[start..field.end]
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| bad("unterminated weapon text"))?;
        let old = &pack[start..start + len];
        if skill != BLUNT || !old.starts_with(WRONG) {
            continue;
        }
        let digits = old[WRONG.len()..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let rest = &old[WRONG.len() + digits..];
        need(
            digits > 0 && rest.starts_with(b"\r\n"),
            "unexpected weapon text layout",
        )?;
        let mut new = RIGHT.to_vec();
        new.extend(&old[WRONG.len()..WRONG.len() + digits]);
        new.push(b'\n');
        new.extend(&rest[2..]);
        need(new.len() == old.len(), "weapon text length changed")?;
        rows.push(row);
        // Records may share one string object; write it once.
        if !out.iter().any(|(at, _)| *at == start) {
            out.push((start, new));
        }
    }
    need(
        rows.is_empty() || rows == ROWS.collect::<Vec<_>>(),
        "unexpected mislabelled weapons",
    )?;
    Ok(out)
}

/// Writes for the image's original AREA archive.
pub(crate) fn regions(iso: &[u8]) -> Result<Vec<WriteRegion>> {
    let (at, size) = if crate::release::detect(iso)?.is_gamecube() {
        let f = gcm::find_file(iso, "AREA.AFS")?;
        (f.offset as usize, f.size as usize)
    } else {
        let f = iso9660::find_file(iso, "AREA.AFS")?;
        (f.lba as usize * 2048, f.size as usize)
    };
    let afs = iso
        .get(at..at + size)
        .ok_or_else(|| bad("language archive outside image"))?;
    Ok(edits(afs)?
        .into_iter()
        .map(|(offset, bytes)| WriteRegion {
            offset: (at + offset) as u64,
            bytes,
        })
        .collect())
}

/// Fix a rebuilt AREA archive in place, as [`regions`] does the original.
pub(super) fn fix_archive(afs: &mut [u8]) -> Result<()> {
    for (at, bytes) in edits(afs)? {
        afs[at..at + bytes.len()].copy_from_slice(&bytes);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retail(name: &str) -> Option<Vec<u8>> {
        std::env::var_os(name).map(|path| std::fs::read(path).unwrap())
    }

    /// The US English pack of every stage, and the European disc's French slot.
    #[test]
    fn every_mislabelled_blunt_weapon_is_fixed_in_each_release() {
        for (name, packs) in [
            ("DW4_RETAIL_USA_ISO", 95),
            ("DW4_RETAIL_EUROPE_ISO", 190),
            ("DW4_RETAIL_JAPAN_ISO", 95),
            ("DW4_RETAIL_GC_USA_ISO", 95),
            ("DW4_RETAIL_GC_JAPAN_ISO", 95),
        ] {
            let Some(iso) = retail(name) else { continue };
            let writes = regions(&iso).unwrap();
            assert_eq!(writes.len(), packs * ROWS.len(), "{name}");
            let mut patched = iso.clone();
            for region in &writes {
                let at = region.offset as usize;
                let old = &iso[at..at + region.bytes.len()];
                assert!(old.starts_with(WRONG), "{name}");
                assert!(region.bytes.starts_with(RIGHT), "{name}");
                // Only the label and the first carriage return change.
                let strip = |s: &[u8]| {
                    let mut v = s.to_vec();
                    let cr = v.iter().position(|&c| c == b'\r').unwrap();
                    v.remove(cr);
                    v
                };
                assert_eq!(
                    strip(old)[WRONG.len()..],
                    region.bytes[RIGHT.len()..],
                    "{name}"
                );
                patched[at..at + region.bytes.len()].copy_from_slice(&region.bytes);
            }
            // A fixed image has nothing left to fix and still parses.
            let err = regions(&patched).unwrap_err().to_string();
            assert!(
                err.contains("unexpected weapon text packs"),
                "{name}: {err}"
            );
        }
    }

    /// Random bosses rebuild the AREA archive; its grown packs get the same writes.
    #[test]
    fn rebuilt_language_archive_is_fixed_too() {
        let Some(iso) = retail("DW4_RETAIL_USA_ISO") else {
            return;
        };
        let ndmw = iso9660::find_file(&iso, "NDMW.AFS").unwrap();
        let afs = &iso[ndmw.lba as usize * 2048..][..ndmw.size as usize];
        let entries = (0..u32at(afs, 4).unwrap() as usize)
            .map(|i| {
                let at = u32at(afs, 8 + 8 * i).unwrap() as usize;
                (at, u32at(afs, 12 + 8 * i).unwrap() as usize)
            })
            .collect::<Vec<_>>();
        let release = crate::release::detect(&iso).unwrap();
        let table =
            crate::table::EnemyTable::read(&iso, &release.layout_in(&iso).unwrap()).unwrap();
        let stages = random::boss::ARENAS.iter().map(|a| a.entry).collect();
        let mut area =
            random::boss::intro::build(&iso, afs, &entries, &Default::default(), &table, &stages)
                .unwrap();
        assert_eq!(edits(&area).unwrap().len(), 95 * ROWS.len());
        fix_archive(&mut area).unwrap();
        assert!(edits(&area).is_err(), "nothing left to fix");
        // Dbl Axe α, once per US English pack.
        let fixed = b"Equip. Skill Blunt Skill 210\nMaul made of Klondigizoit metal";
        assert_eq!(memchr::memmem::find_iter(&area, fixed).count(), 95);
    }
}
