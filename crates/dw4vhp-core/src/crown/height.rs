//! Imported Imperialdramon uses a flying model origin. Its actor basis and
//! model-space collision volumes move together; terrain following stays intact.
use super::*;

pub(super) const MODEL: &str = "e_impdra";
// The native body box spans Y=13..28. Lowering by 13 puts its bottom on
// the floor and its centre at 7.5, without burying half of the visible model.
const LOWER_BY: f32 = 13.0;

fn float_at(b: &[u8], at: usize) -> Result<f32> {
    let value = f32::from_bits(u32at(b, at)?);
    need(value.is_finite(), "nonfinite Imperialdramon height")?;
    Ok(value)
}

fn lower(b: &[u8], at: usize, edits: &mut BTreeMap<usize, Edit>) -> Result<()> {
    edits.entry(at).or_insert(Edit {
        range: at..at + 4,
        bytes: (float_at(b, at)? - LOWER_BY).to_le_bytes().to_vec(),
    });
    Ok(())
}

fn lower_box(p: &Pack<'_>, id: u32, edits: &mut BTreeMap<usize, Edit>) -> Result<()> {
    p.class(id, "igAABox")?;
    for field in [2, 3] {
        let r = p.rec(id)?.field(field)?;
        need(r.len() == 12, "unexpected Imperialdramon bounding box")?;
        lower(p.b, r.start + 4, edits)?;
    }
    Ok(())
}

/// Apply once, after the imported model's shared instance pool has been grown.
/// Native Imperialdramon encounters are left alone by the randomizer's caller.
pub(super) fn imperial(b: &[u8]) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let model = p.named("beModelCtrlInfo", MODEL)?;
    let action = p.named("beAction2Info", MODEL)?;
    let actors = p.array(p.val(action, 6)?)?;
    need(
        !actors.is_empty() && actors.len() == p.array(p.val(model, 6)?)?.len(),
        "Imperialdramon model and action pools differ",
    )?;
    let mut edits = BTreeMap::new();
    for ram in actors {
        p.class(ram, "beAction2InfoRam")?;
        let list = p.array(p.val(ram, 13)?)?;
        need(list.len() == 1, "unexpected Imperialdramon actor count")?;
        let actor = list[0];
        p.class(actor, "igActor")?;
        let transform = p.rec(actor)?.field(14)?;
        need(
            transform.len() == 64,
            "unexpected Imperialdramon actor transform",
        )?;
        let y = transform.start + 13 * 4;
        need(
            float_at(b, y)? == 0.0,
            "Imperialdramon actor already has a height offset",
        )?;
        lower(b, y, &mut edits)?;
        // The engine also uses the actor's bounds for model-space queries.
        lower_box(&p, p.val(actor, 3)?, &mut edits)?;
        let appearance = p.val(actor, 11)?;
        p.class(appearance, "igAppearance")?;
        let skin = p.val(appearance, 3)?;
        p.class(skin, "igSkin")?;
        lower_box(&p, p.val(skin, 4)?, &mut edits)?;
    }
    let mut hitboxes = std::collections::BTreeSet::new();
    let mut bullet_boxes = 0;
    for data in p.array(p.val(model, 5)?)? {
        let r = p.rec(data)?;
        match p.classes[r.class].name.as_str() {
            "beModelCtrlInfoDataHit" => {
                for bbox in p.array(p.val(data, 6)?)? {
                    lower_box(&p, bbox, &mut edits)?;
                    bullet_boxes += 1;
                }
            }
            "beModelCtrlInfoDataSeq" => {
                for sequence in p.array(p.val(data, 4)?)? {
                    p.class(sequence, "beModelCtrlData")?;
                    for command in p.array(p.val(sequence, 3)?)? {
                        if command == NULL {
                            continue;
                        }
                        let r = p.rec(command)?;
                        if p.classes[r.class].name == "beModelCtrlHITBOX" {
                            hitboxes.insert(command);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    need(
        !hitboxes.is_empty() && bullet_boxes > 0,
        "missing Imperialdramon collision volumes",
    )?;
    for id in hitboxes {
        let r = p.rec(id)?;
        let offset = r.field(6)?;
        need(
            offset.len() == 12,
            "unexpected Imperialdramon hitbox offset",
        )?;
        need(
            float_at(b, offset.start + 4)? == 20.5,
            "unexpected Imperialdramon body height",
        )?;
        lower(b, offset.start + 4, &mut edits)?;
    }
    Ok(edits.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use memmap2::Mmap;
    use std::{fs::File, path::Path};

    #[test]
    fn imported_imperial_height_preserves_terrain_and_other_models() {
        for key in [
            "DW4_RETAIL_USA_ISO",
            "DW4_RETAIL_EUROPE_ISO",
            "DW4_RETAIL_JAPAN_ISO",
            "DW4_RETAIL_GC_USA_ISO",
            "DW4_RETAIL_GC_JAPAN_ISO",
        ] {
            let Some(path) = std::env::var_os(key) else {
                continue;
            };
            let file = File::open(Path::new(&path)).unwrap();
            // The user's original image is read-only throughout this regression.
            let image = unsafe { Mmap::map(&file).unwrap() };
            let (base, size) = if crate::release::detect(&image).unwrap().is_gamecube() {
                let file = gcm::find_file(&image, "ndmw.afs").unwrap();
                (file.offset as usize, file.size as usize)
            } else {
                let file = iso9660::find_file(&image, "NDMW.AFS").unwrap();
                (file.lba as usize * 2048, file.size as usize)
            };
            let afs = &image[base..base + size];
            let entry = |i: usize| {
                let off = u32at(afs, 8 + 8 * i).unwrap() as usize;
                let size = u32at(afs, 12 + 8 * i).unwrap() as usize;
                &afs[off..off + size]
            };
            let src = entry(723);
            let profile = graft::profile(&Pack::parse(src).unwrap(), MODEL).unwrap();
            let dst = entry(614);
            let base = add_classes(dst, &profile.missing(&Pack::parse(dst).unwrap())).unwrap();
            let (edits, _) = graft::graft(&base, src, &[MODEL.into()]).unwrap();
            let imported = materialize(&base, &edits).unwrap();
            let expanded = capacity::expand(&imported, MODEL, 8).unwrap();
            let edits = imperial(&expanded).unwrap();
            assert!(!edits.is_empty());
            let result = materialize(&expanded, &edits).unwrap();
            let before = Pack::parse(&expanded).unwrap();
            let after = Pack::parse(&result).unwrap();
            assert_eq!(before.mapping, after.mapping);
            assert_eq!(&expanded[before.pool..], &result[after.pool..]);
            let native = Pack::parse(&base).unwrap();
            for (id, index) in native.ids.iter().enumerate() {
                if index.is_some() {
                    assert_eq!(
                        &expanded[before.rec(id as u32).unwrap().range.clone()],
                        &result[after.rec(id as u32).unwrap().range.clone()],
                        "native model resources must remain unchanged"
                    );
                }
            }
            let mut cursor = 0;
            for edit in &edits {
                assert_eq!(
                    &expanded[cursor..edit.range.start],
                    &result[cursor..edit.range.start]
                );
                assert_eq!(edit.range.len(), 4);
                let old = float_at(&expanded, edit.range.start).unwrap();
                let new = float_at(&result, edit.range.start).unwrap();
                assert_eq!(new, old - LOWER_BY);
                cursor = edit.range.end;
            }
            assert_eq!(&expanded[cursor..], &result[cursor..]);
            let action = after.named("beAction2Info", MODEL).unwrap();
            let rams = after.array(after.val(action, 6).unwrap()).unwrap();
            assert_eq!(rams.len(), 8);
            for ram in rams {
                let actor = after.array(after.val(ram, 13).unwrap()).unwrap()[0];
                let matrix = after.rec(actor).unwrap().field(14).unwrap();
                assert_eq!(float_at(&result, matrix.start + 13 * 4).unwrap(), -LOWER_BY);
            }
            let info = after.named("beModelCtrlInfo", MODEL).unwrap();
            for datum in after.array(after.val(info, 5).unwrap()).unwrap() {
                let r = after.rec(datum).unwrap();
                if after.classes[r.class].name == "beModelCtrlInfoDataHit" {
                    for bbox in after.array(after.val(datum, 6).unwrap()).unwrap() {
                        let r = after.rec(bbox).unwrap();
                        assert_eq!(
                            float_at(&result, r.field(2).unwrap().start + 4).unwrap(),
                            0.0
                        );
                        assert_eq!(
                            float_at(&result, r.field(3).unwrap().start + 4).unwrap(),
                            15.0
                        );
                    }
                }
                if after.classes[r.class].name == "beModelCtrlInfoDataSeq" {
                    for sequence in after.array(after.val(datum, 4).unwrap()).unwrap() {
                        for command in after.array(after.val(sequence, 3).unwrap()).unwrap() {
                            if command == NULL {
                                continue;
                            }
                            let r = after.rec(command).unwrap();
                            if after.classes[r.class].name == "beModelCtrlHITBOX" {
                                assert_eq!(
                                    float_at(&result, r.field(6).unwrap().start + 4).unwrap(),
                                    7.5
                                );
                            }
                        }
                    }
                }
            }
            assert!(imperial(&result).is_err(), "normalization must not stack");
            eprintln!("PASS: {key}: eight Imperialdramon instances lowered together; other bytes and memory unchanged");
        }
    }
}
