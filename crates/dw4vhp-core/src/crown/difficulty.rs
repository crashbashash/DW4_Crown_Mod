//! Difficulty-specific stage files retain the native bootstrap and sound order.
//! Original entries hold Normal; appended entries hold Hard and Very Hard.
//! The filename hook selects a manifest's pipe-separated alternative before IO.
use super::*;
use std::collections::BTreeSet;

fn manifest(iso: &[u8]) -> Result<(usize, &[u8])> {
    let (offset, size) = if crate::gcm::game_id(iso).is_ok() {
        let file = crate::gcm::find_file(iso, "NDMW.IGB")?;
        (file.offset as usize, file.size as usize)
    } else {
        let file = crate::iso9660::find_file(iso, "NDMW.IGB")?;
        (file.lba as usize * 2048, file.size as usize)
    };
    Ok((
        offset,
        iso.get(offset..offset + size)
            .ok_or_else(|| bad("resource manifest outside image"))?,
    ))
}

pub(super) fn validate_tiers<'a>(
    iso: &[u8],
    stages: impl Iterator<Item = &'a [usize]>,
) -> Result<()> {
    let (_, bytes) = manifest(iso)?;
    let p = Pack::parse(bytes)?;
    let mut names = BTreeMap::new();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "igAdxAfsFile").is_ok() && p.val(id, 4)? == 0 {
            if let Some(name) = graft::name_of(&p, p.rec(id)?) {
                names.insert(p.val(id, 5)? as usize, name.to_ascii_lowercase());
            }
        }
    }
    for gens in stages.filter(|gens| gens.len() == 3) {
        for (&pack, expected) in
            gens.iter()
                .zip([b"generatorh.igb", b"generatorn.igb", b"generatorv.igb"])
        {
            need(
                names.get(&pack).is_some_and(|name| name == expected),
                "unrecognised generator difficulty order",
            )?;
        }
    }
    Ok(())
}

pub(super) fn file_count(iso: &[u8], original: usize, expanded: usize) -> Result<WriteRegion> {
    let (offset, bytes) = manifest(iso)?;
    let p = Pack::parse(bytes)?;
    let mut fields = Vec::new();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beNDMWAfsSetupData").is_ok()
            && graft::name_of(&p, p.rec(id)?)
                .is_some_and(|name| name.eq_ignore_ascii_case(b"ndmw.afs"))
        {
            need(
                p.val(id, 3)? as usize == original,
                "resource manifest archive count differs",
            )?;
            fields.push(p.rec(id)?.field(3)?);
        }
    }
    need(
        fields.len() == 1 && expanded <= u16::MAX as usize,
        "invalid difficulty archive count",
    )?;
    Ok(WriteRegion {
        offset: (offset + fields[0].start) as u64,
        bytes: (expanded as u32).to_le_bytes().to_vec(),
    })
}

pub(super) fn retarget_manifest(
    bytes: &[u8],
    targets: &BTreeMap<usize, [usize; 2]>,
) -> Result<Option<Vec<u8>>> {
    if bytes.len() > 100_000 || memchr::memmem::find(bytes, b"beFileListInfo\0").is_none() {
        return Ok(None);
    }
    let p = Pack::parse(bytes)?;
    let mut edits = Vec::new();
    let mut changed = BTreeSet::new();
    let mut growth = 0usize;
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beFileListInfo").is_err() {
            continue;
        }
        let list = p.val(id, 6)?;
        if list == NULL {
            continue;
        }
        for name_id in p.array(list)? {
            if !changed.insert(name_id) {
                continue;
            }
            let record = p.rec(name_id)?;
            let Some(name) = graft::name_of(&p, record) else {
                continue;
            };
            let Some(body) = name.strip_prefix(b"[0.") else {
                continue;
            };
            let Some(close) = body.iter().position(|b| *b == b']') else {
                continue;
            };
            let original = std::str::from_utf8(&body[..close])
                .ok()
                .and_then(|s| s.parse::<usize>().ok());
            let Some(original) = original else { continue };
            let Some(target) = targets.get(&original) else {
                continue;
            };
            let suffix = &body[close + 1..];
            need(
                suffix.is_empty() || suffix == b"/s",
                "unexpected stage resource selector",
            )?;
            p.class(name_id, "igNamedObject")?;
            need(record.fields.len() == 1, "unexpected stage filename record")?;
            let encoded = format!(
                "[0.{original}]|[0.{}]|[0.{}]{}",
                target[0],
                target[1],
                if suffix.is_empty() { "" } else { "/s" }
            );
            let width = a4(encoded.len() + 1);
            let mut raw = words(&[record.class as u32, (12 + width) as u32, width as u32]);
            raw.extend(encoded.as_bytes());
            raw.resize(12 + width, 0);
            growth += raw.len() - record.range.len();
            edits.push(Edit {
                range: record.range.clone(),
                bytes: raw,
            });
        }
    }
    if edits.is_empty() {
        return Ok(None);
    }
    edits.push(Edit {
        range: 16..20,
        bytes: (u32at(bytes, 16)? + growth as u32).to_le_bytes().to_vec(),
    });
    edits.sort_by_key(|edit| edit.range.start);
    let result = materialize(bytes, &edits)?;
    need(
        Pack::parse(&result)?.ids == p.ids,
        "stage manifest IDs changed",
    )?;
    Ok(Some(result))
}
