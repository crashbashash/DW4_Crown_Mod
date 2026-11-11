//! Scripted enemy spawns carry their own HP. An enemy GENERATE string
//! `[SQ_x/]I<row>/I<hp>/...` stores `hp` as both maximum and current HP
//! (`beNDMWMdlEnemy` vfunc_36); only the model, stats and crown rank come from
//! `row`. Pharaohmon's summons, Daemon's return from its split form and the
//! practice lesson's sleeping Goburimon use this, so they showed a crowned
//! row's icon with retail HP. Scale each literal by the HP factor the final
//! table gave the row that supplies the spawn's stats.
use super::rewards::Variant;
use super::*;
use crate::table::EnemyTable;

/// `(text before the HP digits, row, HP, text after the HP digits)`.
fn parse(text: &[u8]) -> Option<(&[u8], usize, i64, &[u8])> {
    let text = std::str::from_utf8(text).ok()?;
    let body = match text.strip_prefix("SQ_") {
        Some(rest) => {
            let slash = rest.find('/')?;
            let sequence = &rest[..slash];
            if sequence.is_empty()
                || !sequence
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            {
                return None;
            }
            &rest[slash + 1..]
        }
        None => text,
    };
    let int = |s: &str| -> Option<(i64, usize)> {
        let digits = s.strip_prefix("I<")?;
        let close = digits.find('>')?;
        let value = &digits[..close];
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some((value.parse().ok()?, 2 + close + 1))
    };
    let (row, used) = int(body)?;
    let rest = body[used..].strip_prefix('/')?;
    let (hp, used) = int(rest)?;
    let tail = &rest[used..];
    if !(tail.is_empty() || tail.starts_with('/')) {
        return None;
    }
    let start = text.len() - rest.len() + 2;
    let end = text.len() - tail.len() - 1;
    Some((
        &text.as_bytes()[..start],
        usize::try_from(row).ok()?,
        hp,
        &text.as_bytes()[end..],
    ))
}

/// Whether `bytes` may hold a GENERATE HP literal (`digit>/I<digit`).
pub(super) fn candidate(bytes: &[u8]) -> bool {
    memchr::memmem::find_iter(bytes, b">/I<").any(|at| {
        at > 0
            && bytes[at - 1].is_ascii_digit()
            && bytes.get(at + 4).is_some_and(u8::is_ascii_digit)
    })
}

/// Rescale the GENERATE HP literals of every script in `pack`. `variants` are
/// the stage's local rows, whose HP comes from their original encounter.
/// Returns `None` when nothing changes.
pub(super) fn rescale(
    pack: &[u8],
    authored: &EnemyTable,
    balanced: &EnemyTable,
    variants: Option<&BTreeMap<usize, Variant>>,
) -> Result<Option<Vec<u8>>> {
    if !candidate(pack) {
        return Ok(None);
    }
    let p = Pack::parse(pack)?;
    let mut scripts = BTreeMap::new();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaData").is_err() {
            continue;
        }
        let Some(name) = graft::name_of(&p, p.rec(id)?) else {
            continue;
        };
        let object = p.val(id, 3)?;
        let range = p
            .memory
            .get(p.val(object, 3)? as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing Lua memory"))?;
        let found = lua_names::constants(&pack[range.clone()])?;
        if !found.iter().any(|c| c.text == b"GENERATE") {
            continue;
        }
        let mut map = BTreeMap::new();
        for c in &found {
            let Some((head, row, hp, tail)) = parse(&c.text) else {
                continue;
            };
            let source = variants.and_then(|v| v.get(&row)).map_or(row, |v| v.reward);
            need(
                source < authored.len() && authored.hp[source] > 0,
                &format!(
                    "script HP row {row} has no authored HP: {}",
                    String::from_utf8_lossy(&c.text)
                ),
            )?;
            let scaled = (hp as f64 * balanced.hp[source] as f64 / authored.hp[source] as f64)
                .round_ties_even()
                .clamp(1.0, crate::plan::HP_CAP) as i64;
            if scaled != hp {
                let mut text = head.to_vec();
                text.extend(scaled.to_string().bytes());
                text.extend(tail);
                map.insert(
                    String::from_utf8_lossy(&c.text).into_owned(),
                    String::from_utf8(text).unwrap(),
                );
            }
        }
        if !map.is_empty() {
            let name = String::from_utf8_lossy(name).into_owned();
            need(
                scripts.insert(name.clone(), map).is_none(),
                &format!("duplicate Lua script {name}"),
            )?;
        }
    }
    if scripts.is_empty() {
        return Ok(None);
    }
    let names = scripts.keys().cloned().collect::<Vec<_>>();
    let edits = lua_names::rewrite_in_pack(pack, &names, |script, raw| {
        lua_names::rename(raw, &scripts[script])?
            .ok_or_else(|| bad(&format!("no HP literal in {script}")))
    })?;
    Ok(Some(materialize(pack, &edits)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_hp_literals_keep_their_surrounding_text() {
        assert_eq!(
            parse(b"SQ_NENTR/I<116>/I<149>/"),
            Some((&b"SQ_NENTR/I<116>/I<"[..], 116, 149, &b">/"[..]))
        );
        assert_eq!(
            parse(b"I<638>/I<30>/V<-65,10,-70>/V<0,0,0>/SQ_SLEP"),
            Some((
                &b"I<638>/I<"[..],
                638,
                30,
                &b">/V<-65,10,-70>/V<0,0,0>/SQ_SLEP"[..]
            ))
        );
        assert_eq!(
            parse(b"I<360>/I<2620>"),
            Some((&b"I<360>/I<"[..], 360, 2620, &b">"[..]))
        );
        for other in [
            &b"TALKFONT05/font00/I<1>"[..],
            b"f_devl1/I<0>",
            b"I<3>",
            b"I<1>/F<2>/",
            b"I<1>/I<2>x",
            b"SQ_/I<1>/I<2>/",
            b"I<-1>/I<2>/",
        ] {
            assert_eq!(parse(other), None, "{}", String::from_utf8_lossy(other));
        }
    }

    /// Every retail scripted HP literal is found, belongs to the expected
    /// spawn, and receives its row's Crown HP factor; nothing else matches.
    #[test]
    fn retail_script_hp_literals_receive_the_crown_hp_factor() {
        let expected = BTreeMap::from([
            ((116, 149), "e_pharaoh summon"),
            ((120, 896), "e_pharaoh summon"),
            ((124, 2738), "e_pharaoh summon"),
            ((360, 2620), "f_devl1 return"),
            ((361, 14520), "f_devl1 return"),
            ((362, 26620), "f_devl1 return"),
            ((638, 30), "practice e_goburi"),
        ]);
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
            let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let release = crate::release::detect(&iso).unwrap();
            let (base, size) = if release.is_gamecube() {
                let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
                (f.offset as usize, f.size as usize)
            } else {
                let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (f.lba as usize * 2048, f.size as usize)
            };
            let afs = &iso[base..base + size];
            let authored = EnemyTable::read(&iso, &release.layout_in(&iso).unwrap()).unwrap();
            let (balanced, _) = crate::plan::transform(
                &authored,
                &crate::plan::PatchPlan::default(),
                &crate::ratio::RatioVector::derive(&authored),
            );
            let mut found = BTreeMap::<(usize, i64), usize>::new();
            for i in 0..u32at(afs, 4).unwrap() as usize {
                let off = u32at(afs, 8 + 8 * i).unwrap() as usize;
                let len = u32at(afs, 12 + 8 * i).unwrap() as usize;
                let data = &afs[off..off + len];
                assert!(
                    rescale(data, &authored, &authored, None).unwrap().is_none(),
                    "{variable} entry {i}: the authored table changed scripts"
                );
                let Some(out) = rescale(data, &authored, &balanced, None).unwrap() else {
                    continue;
                };
                let (before, after) = (Pack::parse(data).unwrap(), Pack::parse(&out).unwrap());
                let text = |p: &Pack, b: &[u8], id: u32| {
                    let object = p.val(id, 3).unwrap();
                    let range = p.memory[p.val(object, 3).unwrap() as usize]
                        .clone()
                        .unwrap();
                    lua_names::constants(&b[range])
                        .unwrap()
                        .into_iter()
                        .map(|c| c.text)
                        .collect::<Vec<_>>()
                };
                for id in 0..before.ids.len() as u32 {
                    if before.class(id, "beLuaData").is_err() {
                        continue;
                    }
                    let (old, new) = (text(&before, data, id), text(&after, &out, id));
                    assert_eq!(old.len(), new.len());
                    for (old, new) in old.iter().zip(&new).filter(|(a, b)| a != b) {
                        let (head, row, hp, tail) = parse(old).unwrap();
                        let (new_head, new_row, scaled, new_tail) = parse(new).unwrap();
                        assert_eq!((head, row, tail), (new_head, new_row, new_tail));
                        assert!(
                            expected.contains_key(&(row, hp)),
                            "{variable} entry {i}: unexpected {}",
                            String::from_utf8_lossy(old)
                        );
                        let factor = balanced.hp[row] as f64 / authored.hp[row] as f64;
                        assert!(factor > 1.0);
                        assert!(
                            (scaled as f64 - hp as f64 * factor).abs() <= 0.5,
                            "{variable} entry {i}: {hp} -> {scaled}"
                        );
                        *found.entry((row, hp)).or_default() += 1;
                    }
                }
            }
            assert_eq!(
                found.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>(),
                "{variable}"
            );
            println!("{variable}: {found:?}");
        }
    }
}
