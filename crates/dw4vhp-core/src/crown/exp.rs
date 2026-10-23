//! Five-digit EXP popups. Retail already selects SQ_EXPNM04 for both four and
//! five digits and caps the shared display helper at 99999. Expand only expnm0's
//! private digit controllers and meshes; award calculations and scripts stay intact.
use super::*;
use std::collections::BTreeSet;

const DIGITS: usize = 4;
const VERTICES: usize = DIGITS * 6;
const MAX_VALUE: u32 = 99_999;

pub(super) fn candidate(b: &[u8]) -> bool {
    memchr::memmem::find(b, b"expnm0\0").is_some()
}

fn memory<'a>(p: &Pack<'a>, id: u32) -> Result<&'a [u8]> {
    let range = p
        .memory
        .get(id as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing EXP vertex memory"))?;
    Ok(&p.b[range.clone()])
}

fn next_positions(bytes: &[u8]) -> Result<Vec<u8>> {
    need(
        bytes.len() == VERTICES * 12,
        "unexpected EXP position count",
    )?;
    let mut extra = Vec::with_capacity(6 * 12);
    for vertex in 0..6 {
        for axis in 0..3 {
            let at = ((DIGITS - 1) * 6 + vertex) * 12 + axis * 4;
            let last = f32::from_bits(u32at(bytes, at)?);
            let before = f32::from_bits(u32at(bytes, at - 6 * 12)?);
            let next = if last == before {
                last
            } else {
                last + (last - before)
            };
            need(
                last.is_finite() && before.is_finite() && next.is_finite(),
                "nonfinite EXP vertex position",
            )?;
            extra.extend(next.to_le_bytes());
        }
    }
    Ok(extra)
}

/// The EXP word has its own animated quad. Reserve a digit-width of space to
/// its right, otherwise that quad covers the new most-significant digit.
fn label_positions(p: &Pack<'_>, shift: f32, instances: usize) -> Result<BTreeMap<u32, Vec<u8>>> {
    let mut blocks = BTreeMap::new();
    let mut count = 0;
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let record = &p.objects[*index];
        if p.classes[record.class].name != "igTransform" || !record.named(p.b, "expmoji") {
            continue;
        }
        let children = p.array(p.val(id as u32, 7)?)?;
        need(children.len() == 1, "unexpected EXP label children")?;
        p.class(children[0], "igGeometry")?;
        let attrs = p.array(p.val(children[0], 8)?)?;
        need(attrs.len() == 1, "unexpected EXP label attributes")?;
        p.class(attrs[0], "igGeometryAttr1_5")?;
        let va = p.val(attrs[0], 4)?;
        p.class(va, "igVertexArray1_1")?;
        need(p.val(va, 3)? == 6, "unexpected EXP label vertex count")?;
        let streams = memory(p, p.val(va, 2)?)?;
        let block = u32at(streams, 0)?;
        let mut positions = memory(p, block)?.to_vec();
        need(positions.len() == 6 * 12, "unexpected EXP label positions")?;
        for at in (0..positions.len()).step_by(12) {
            let x = f32::from_bits(u32at(&positions, at)?) + shift;
            need(x.is_finite(), "nonfinite EXP label position")?;
            put(&mut positions, at, x.to_bits());
        }
        blocks.insert(block, positions);
        count += 1;
    }
    need(count == instances, "EXP labels and number instances differ")?;
    Ok(blocks)
}

pub(super) fn expand(b: &[u8]) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let info = p.named("beNumberCtrlInfo", "expnm0")?;
    let rams = p.array(p.val(info, 6)?)?;
    need(!rams.is_empty(), "no EXP number instances")?;
    let mut builder = Builder::new(&p).reuse_directories();
    let mut grown = BTreeSet::new();
    let mut changed = BTreeSet::new();
    let mut template = None;
    let mut label_shift = None;
    for &ram in &rams {
        p.class(ram, "beNumberCtrlInfoRam")?;
        need(p.val(ram, 13)? == 9999, "unexpected EXP display cap")?;
        builder.modify(ram, &[(13, MAX_VALUE)])?;
        let list = p.val(ram, 15)?;
        p.class(list, "beNumVerDataList")?;
        let digits = p.array(list)?;
        need(digits.len() == DIGITS, "unexpected EXP digit count")?;
        if changed.insert(list) {
            // Each digit addresses six UV vertices. Clone its mutable state and
            // corner records, pointing the new digit at the six appended vertices.
            let last = digits[DIGITS - 1];
            p.class(last, "beNumVerData")?;
            let inside = p.val(last, 3)?;
            p.class(inside, "beNumVerInsideDataList")?;
            let corners = p.array(inside)?;
            need(corners.len() == 6, "unexpected EXP digit corners")?;
            let mut new_corners = Vec::new();
            for (index, &corner) in corners.iter().enumerate() {
                p.class(corner, "beNumVerInsideData")?;
                need(
                    p.val(corner, 2)? as usize == (DIGITS - 1) * 6 + index,
                    "unexpected EXP corner index",
                )?;
                new_corners.push(builder.obj(corner, &[(2, (VERTICES + index) as u32)])?);
            }
            let block = builder.mem(words(&new_corners), p.val(inside, 4)?)?;
            let new_inside = builder.obj(inside, &[(4, block)])?;
            let new_digit = builder.obj(last, &[(3, new_inside)])?;
            let block = p.val(list, 4)?;
            builder.grow_mem(block, words(&[new_digit]))?;
            builder.modify(list, &[(2, 5), (3, 5)])?;
            template = Some(block);
        }
        let geometry = p.val(ram, 11)?;
        p.class(geometry, "igGeometry")?;
        let attrs = p.array(p.val(geometry, 8)?)?;
        need(attrs.len() == 1, "unexpected EXP geometry attributes")?;
        let attr = attrs[0];
        p.class(attr, "igGeometryAttr1_5")?;
        need(
            p.val(attr, 6)? == 3 && p.val(attr, 7)? == 8 && p.val(attr, 5)? == NULL,
            "unexpected EXP triangle layout",
        )?;
        if changed.insert(attr) {
            builder.modify(attr, &[(7, 10)])?;
        }
        let va = p.val(attr, 4)?;
        p.class(va, "igVertexArray1_1")?;
        need(
            p.val(va, 3)? as usize == VERTICES,
            "unexpected EXP vertex count",
        )?;
        if changed.insert(va) {
            builder.modify(va, &[(3, 30)])?;
        }
        let streams = memory(&p, p.val(va, 2)?)?;
        need(
            streams.len().is_multiple_of(4),
            "unaligned EXP stream table",
        )?;
        let mut sizes = Vec::new();
        for at in (0..streams.len()).step_by(4) {
            let block = u32at(streams, at)?;
            if block == NULL {
                continue;
            }
            let bytes = memory(&p, block)?;
            let extra = match (at / 4, bytes.len()) {
                (0, n) if n == VERTICES * 12 => {
                    let shift = f32::from_bits(u32at(bytes, 18 * 12)?)
                        - f32::from_bits(u32at(bytes, 12 * 12)?);
                    need(
                        shift.is_finite() && shift < 0.0,
                        "unexpected EXP digit spacing",
                    )?;
                    need(
                        label_shift.is_none_or(|s: f32| (s - shift).abs() < 0.0001),
                        "EXP instance spacing differs",
                    )?;
                    label_shift = Some(shift);
                    next_positions(bytes)?
                }
                (1, n) if n == VERTICES * 12 => bytes[bytes.len() - 6 * 12..].to_vec(),
                (11, n) if n == VERTICES * 8 => bytes[bytes.len() - 6 * 8..].to_vec(),
                _ => return Err(bad("unknown EXP vertex stream")),
            };
            sizes.push(bytes.len());
            if grown.insert(block) {
                builder.grow_mem(block, extra)?;
            }
        }
        sizes.sort_unstable();
        need(
            sizes == [VERTICES * 8, VERTICES * 12]
                || sizes == [VERTICES * 8, VERTICES * 12, VERTICES * 12],
            "unexpected EXP vertex streams",
        )?;
    }
    let edits = builder.finish(template.ok_or_else(|| bad("no EXP digit list"))?)?;
    let mut out = materialize(b, &edits)?;
    let v = Pack::parse(&out)?;
    for &ram in &rams {
        need(
            v.val(ram, 13)? == MAX_VALUE && v.array(v.val(ram, 15)?)?.len() == 5,
            "expanded EXP controller failed verification",
        )?;
    }
    let labels = label_positions(
        &p,
        label_shift.ok_or_else(|| bad("no EXP spacing"))?,
        rams.len(),
    )?;
    let changes = labels
        .into_iter()
        .map(|(id, bytes)| {
            let range = v.memory[id as usize]
                .clone()
                .ok_or_else(|| bad("missing expanded EXP label memory"))?;
            Ok(Edit { range, bytes })
        })
        .collect::<Result<Vec<_>>>()?;
    for edit in changes {
        out[edit.range].copy_from_slice(&edit.bytes);
    }
    Ok(vec![Edit {
        range: 0..b.len(),
        bytes: out,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifth_quad_continues_the_authored_spacing() {
        let vertices = (0..VERTICES)
            .flat_map(|n| [-((n / 6) as f32) * 2.5, (n % 2) as f32 * -4.0, 0.0])
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        let next = next_positions(&vertices).unwrap();
        for i in 0..6 {
            assert_eq!(f32::from_bits(u32at(&next, i * 12).unwrap()), -10.0);
            assert_eq!(
                &next[i * 12 + 4..i * 12 + 12],
                &vertices[(18 + i) * 12 + 4..(18 + i) * 12 + 12]
            );
        }
        assert!(next_positions(&vertices[..vertices.len() - 4]).is_err());
        let mut invalid = vertices;
        put(&mut invalid, 18 * 12, f32::NAN.to_bits());
        assert!(next_positions(&invalid).is_err());
    }

    #[test]
    fn retail_exp_popups_expand_on_every_configured_release() {
        use memmap2::Mmap;
        use std::fs::File;
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
            let file = File::open(path).unwrap();
            // The original disc is mapped read-only throughout this audit.
            let image = unsafe { Mmap::map(&file).unwrap() };
            let release = crate::release::detect(&image).unwrap();
            let (base, size) = if release.is_gamecube() {
                let file = gcm::find_file(&image, "ndmw.afs").unwrap();
                (file.offset as usize, file.size as usize)
            } else {
                let file = iso9660::find_file(&image, "NDMW.AFS").unwrap();
                (file.lba as usize * 2048, file.size as usize)
            };
            let afs = &image[base..base + size];
            let mut count = 0;
            for entry in 0..u32at(afs, 4).unwrap() as usize {
                let off = u32at(afs, 8 + 8 * entry).unwrap() as usize;
                let size = u32at(afs, 12 + 8 * entry).unwrap() as usize;
                let b = &afs[off..off + size];
                if !candidate(b) {
                    continue;
                }
                let edits = expand(b).unwrap_or_else(|e| panic!("{key} entry {entry}: {e}"));
                let out = materialize(b, &edits).unwrap();
                let p = Pack::parse(b).unwrap();
                let v = Pack::parse(&out).unwrap();
                let info = p.named("beNumberCtrlInfo", "expnm0").unwrap();
                let rams = p.array(p.val(info, 6).unwrap()).unwrap();
                let mut expected_changes = BTreeMap::new();
                for &ram in &rams {
                    expected_changes.insert(ram, vec![(13, MAX_VALUE)]);
                    let list = p.val(ram, 15).unwrap();
                    expected_changes.insert(list, vec![(2, 5), (3, 5)]);
                    let original = p.array(list).unwrap();
                    let expanded = v.array(list).unwrap();
                    assert_eq!(&expanded[..DIGITS], &original);
                    assert!(!original.contains(&expanded[DIGITS]));
                    assert_eq!(p.val(list, 4).unwrap(), v.val(list, 4).unwrap());
                    let inside = v.val(expanded[DIGITS], 3).unwrap();
                    for (i, corner) in v.array(inside).unwrap().iter().enumerate() {
                        assert_eq!(v.val(*corner, 2).unwrap() as usize, VERTICES + i);
                    }
                    let geometry = p.val(ram, 11).unwrap();
                    let attr = p.array(p.val(geometry, 8).unwrap()).unwrap()[0];
                    let va = p.val(attr, 4).unwrap();
                    expected_changes.insert(attr, vec![(7, 10)]);
                    expected_changes.insert(va, vec![(3, 30)]);
                    assert_eq!(v.val(attr, 7).unwrap(), 10);
                    assert_eq!(v.val(va, 3).unwrap(), 30);
                    let streams = memory(&p, p.val(va, 2).unwrap()).unwrap();
                    for at in (0..streams.len()).step_by(4) {
                        let block = u32at(streams, at).unwrap();
                        if block == NULL {
                            continue;
                        }
                        let before = memory(&p, block).unwrap();
                        let after = memory(&v, block).unwrap();
                        assert_eq!(&after[..before.len()], before);
                        assert_eq!(after.len() * DIGITS, before.len() * 5);
                    }
                }
                let labels = label_positions(&p, -2.5, rams.len()).unwrap();
                for (block, expected) in labels {
                    assert_eq!(memory(&v, block).unwrap(), expected);
                }
                // Other number displays, scripts, textures, model sequences and
                // original digit state stay byte-identical.
                for (id, index) in p.ids.iter().enumerate() {
                    let Some(index) = index else { continue };
                    let r = &p.objects[*index];
                    let changes = expected_changes
                        .get(&(id as u32))
                        .map_or(&[][..], Vec::as_slice);
                    assert_eq!(
                        r.patch(b, changes).unwrap(),
                        out[v.rec(id as u32).unwrap().range.clone()]
                    );
                }
                count += 1;
            }
            assert!(
                count >= 85,
                "{key}: expected all stage EXP packs, found {count}"
            );
            eprintln!("PASS: {key}: {count} five-digit EXP packs");
        }
    }
}
