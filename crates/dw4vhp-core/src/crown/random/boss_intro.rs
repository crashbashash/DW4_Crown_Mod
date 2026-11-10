//! Keep native language selection and Unicode glyph codes. Add donor message
//! banks to each arena’s and supported introduction’s localized packs;
//! difficulty-specific intro scripts use their new indexes. Original messages
//! and databases retain their identities.
use super::*;

type Messages = [Vec<u8>; 4];

const FONTS: [&str; 4] = [
    "DMINFOFONT01/font00/",
    "DMINFOFONT02/font00/",
    "DMINFOFONT02/font01/",
    "DMINFOFONT02/font02/",
];

/// Retail information cards for importable families: model, stage and the
/// card's four message rows. Several stages hold two cards, so the rows are
/// explicit and checked against the stage's scripts. MR04 and MR05 use their
/// boss-arena cards; MR01-MR03 and MarineDevimon have no retail card.
pub(in crate::crown) const FAMILY_CARDS: &[(&str, usize, [usize; 4])] = &[
    ("e_scum", 187, [9, 10, 11, 12]),
    ("e_minotaru", 241, [9, 10, 11, 12]),
    ("e_pharaoh", 257, [10, 11, 12, 13]),
    ("e_mecha4", 319, [10, 11, 12, 13]),
    ("e_rare", 394, [9, 10, 11, 12]),
    ("e_mecha5", 394, [14, 15, 16, 17]),
    ("e_goburi", 614, [13, 10, 14, 15]),
    ("e_nume", 614, [16, 17, 18, 19]),
    ("e_ogre", 621, [9, 10, 11, 12]),
    ("e_cockatri", 635, [9, 10, 11, 12]),
    ("e_mummy", 642, [9, 10, 11, 12]),
    ("e_tyrano", 642, [13, 14, 15, 16]),
    ("e_arachne", 649, [9, 10, 11, 12]),
    ("e_otama", 679, [9, 10, 11, 12]),
    ("e_geko", 679, [13, 10, 14, 15]),
    ("e_torta", 686, [10, 11, 12, 13]),
    ("e_infermo", 709, [9, 10, 11, 12]),
    ("e_megadra", 709, [13, 14, 15, 16]),
    ("e_impdra", 723, [9, 10, 11, 12]),
];

pub(in crate::crown) fn has_family_card(model: &str) -> bool {
    FAMILY_CARDS.iter().any(|(m, _, _)| *m == model)
}

/// Whether `stage`'s introduction shows a retail card for its native `model`.
pub(in crate::crown) fn has_intro_card(stage: usize, model: &str) -> bool {
    FAMILY_CARDS
        .iter()
        .any(|(m, entry, _)| *m == model && *entry == stage)
}

fn card_destinations() -> impl Iterator<Item = usize> {
    ARENAS
        .iter()
        .map(|a| a.entry)
        .chain(INTRO_STAGES.iter().map(|s| s.model_entry))
}

fn card_constant(row: usize, line: usize) -> String {
    format!("{}I<{row}>", FONTS[line])
}

/// Script constant renames that make an introduction show each site's
/// selected family card from the appended bank. `sites` lists each carded
/// site's (native family, selected family). A site whose family has no card
/// keeps its native card. Cards can share a row (Otamamon's and Gekomon's
/// type line); it changes only when every card using it agrees.
pub(in crate::crown) fn intro_cards(
    stage: usize,
    sites: &[(&str, &str)],
    base: usize,
) -> Result<BTreeMap<String, String>> {
    let mut wanted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for &(native, model) in sites {
        let (_, _, rows) = FAMILY_CARDS
            .iter()
            .find(|(m, entry, _)| *m == native && *entry == stage)
            .ok_or_else(|| bad("introduction has no native card"))?;
        let donor = FAMILY_CARDS
            .iter()
            .position(|(m, _, _)| *m == model)
            .filter(|_| model != native);
        for (line, &row) in rows.iter().enumerate() {
            let to = match donor {
                Some(card) => card_constant(base + 4 * (ARENAS.len() + card) + line, line),
                None => card_constant(row, line),
            };
            wanted
                .entry(card_constant(row, line))
                .or_default()
                .insert(to);
        }
    }
    Ok(wanted
        .into_iter()
        .filter(|(_, to)| to.len() == 1)
        .filter_map(|(from, to)| {
            let to = to.into_iter().next()?;
            (from != to).then_some((from, to))
        })
        .collect())
}

fn selectors(bytes: &[u8], arena: &Arena) -> Result<[String; 4]> {
    let p = Pack::parse(bytes)?;
    let constants = lua_names::constants(script(&p, arena.intro)?)?;
    let mut out = Vec::new();
    for font in FONTS {
        let matches = constants
            .iter()
            .filter(|c| c.text.starts_with(font.as_bytes()))
            .collect::<Vec<_>>();
        need(matches.len() == 1, "ambiguous boss intro text constant")?;
        out.push(
            String::from_utf8(matches[0].text.clone())
                .map_err(|_| bad("invalid boss intro text constant"))?,
        );
    }
    out.try_into().map_err(|_| bad("missing boss intro text"))
}

/// A family card must be the exact card one of its stage's scripts shows.
fn check_family_rows(stage: &[u8], rows: [usize; 4]) -> Result<()> {
    let p = Pack::parse(stage)?;
    let wanted = FONTS
        .iter()
        .zip(rows)
        .map(|(font, row)| format!("{font}I<{row}>"))
        .collect::<Vec<_>>();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaData").is_err() {
            continue;
        }
        let Some(name) = graft::name_of(&p, p.rec(id)?).and_then(|s| std::str::from_utf8(s).ok())
        else {
            continue;
        };
        let constants = lua_names::constants(script(&p, name)?)?;
        if wanted
            .iter()
            .all(|w| constants.iter().any(|c| c.text == w.as_bytes()))
        {
            return Ok(());
        }
    }
    Err(bad("family card rows are not shown by their stage"))
}

fn message_indexes(bytes: &[u8], arena: &Arena) -> Result<[usize; 4]> {
    FONTS
        .iter()
        .zip(selectors(bytes, arena)?)
        .map(|(font, key)| {
            key.strip_prefix(font)
                .and_then(|s| s.strip_prefix("I<"))
                .and_then(|s| s.strip_suffix('>'))
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| bad("boss intro lacks a message index"))
        })
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| bad("missing boss message index"))
}

fn unicode_list(p: &Pack) -> Result<u32> {
    let fonts = (0..p.ids.len() as u32)
        .filter(|&id| p.class(id, "beFontInfo").is_ok())
        .collect::<Vec<_>>();
    need(fonts.len() == 1, "ambiguous boss message table")?;
    p.val(fonts[0], 7)
}

fn indexed_text(p: &Pack, row: usize) -> Result<Vec<u8>> {
    let rows = p.array(unicode_list(p)?)?;
    let id = *rows
        .get(row)
        .ok_or_else(|| bad("boss message outside table"))?;
    p.class(id, "beUnicodeObj")?;
    let range = p
        .memory
        .get(p.val(id, 3)? as usize)
        .and_then(Option::as_ref)
        .ok_or_else(|| bad("missing boss message memory"))?;
    let bytes = &p.b[range.clone()];
    need(
        bytes.len().is_multiple_of(2),
        "invalid boss Unicode message size",
    )?;
    let end = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .position(|b| *b == [0, 0])
        .ok_or_else(|| bad("unterminated boss message"))?;
    // These are retail glyph codes, including locale-specific mappings. Copy
    // them verbatim rather than interpreting them as UTF-8 or Shift-JIS.
    Ok(bytes[..2 * (end + 1)].to_vec())
}

#[derive(Clone)]
struct Language {
    entry: usize,
    offset: usize,
    size: usize,
    count: usize,
}

struct Library {
    /// Each card stage's localized packs, by language file name.
    packs: BTreeMap<usize, BTreeMap<Vec<u8>, Language>>,
    /// The appended bank, in order: the boss arenas' cards, then [`FAMILY_CARDS`].
    cards: Vec<BTreeMap<Vec<u8>, Messages>>,
}

fn file<'a>(iso: &'a [u8], name: &str) -> Result<(usize, &'a [u8])> {
    let (at, size) = if crate::release::detect(iso)?.is_gamecube() {
        let f = crate::gcm::find_file(iso, name)?;
        (f.offset as usize, f.size as usize)
    } else {
        let f = crate::iso9660::find_file(iso, name)?;
        (f.lba as usize * 2048, f.size as usize)
    };
    Ok((
        at,
        iso.get(at..at + size)
            .ok_or_else(|| bad("boss text file outside image"))?,
    ))
}

fn load(iso: &[u8], afs: &[u8], entries: &[(usize, usize)]) -> Result<Library> {
    let (_, bytes) = file(iso, "NDMW.IGB")?;
    let manifest = Pack::parse(bytes)?;
    let mut files = Vec::new();
    for id in 0..manifest.ids.len() as u32 {
        if manifest.class(id, "igAdxAfsFile").is_err() || manifest.val(id, 3)? == NULL {
            continue;
        }
        let path = manifest
            .array(manifest.val(id, 3)?)?
            .into_iter()
            .map(|node| {
                graft::name_of(&manifest, manifest.rec(node)?)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| bad("unnamed boss text directory"))
            })
            .collect::<Result<Vec<_>>>()?;
        files.push((
            manifest.val(id, 4)?,
            manifest.val(id, 5)? as usize,
            graft::name_of(&manifest, manifest.rec(id)?)
                .unwrap_or_default()
                .to_ascii_lowercase(),
            path,
        ));
    }
    let (base, area) = file(iso, "AREA.AFS")?;
    need(area.get(..4) == Some(b"AFS\0"), "invalid boss text archive")?;
    let language_pack = |entry: usize| -> Result<(usize, &[u8])> {
        need(
            entry < u32at(area, 4)? as usize,
            "boss language entry outside archive",
        )?;
        let at = u32at(area, 8 + 8 * entry)? as usize;
        let size = u32at(area, 12 + 8 * entry)? as usize;
        Ok((
            at,
            area.get(at..at + size)
                .ok_or_else(|| bad("boss language pack outside archive"))?,
        ))
    };
    let mut packs = BTreeMap::new();
    let stages = card_destinations()
        .chain(FAMILY_CARDS.iter().map(|(_, entry, _)| *entry))
        .collect::<BTreeSet<_>>();
    for stage in stages {
        let directories = files
            .iter()
            .filter(|(part, entry, _, _)| *part == 0 && *entry == stage + 5)
            .map(|(_, _, _, path)| path)
            .collect::<BTreeSet<_>>();
        need(directories.len() == 1, "ambiguous boss text directory")?;
        let directory = *directories.first().unwrap();
        let mut languages = BTreeMap::new();
        for (_, entry, name, _) in files.iter().filter(|(part, _, name, path)| {
            *part == 1 && path == directory && name.ends_with(b"_pack.igb")
        }) {
            let (at, bytes) = language_pack(*entry)?;
            let p = Pack::parse(bytes)?;
            need(
                languages
                    .insert(
                        name.clone(),
                        Language {
                            entry: *entry,
                            offset: base + at,
                            size: bytes.len(),
                            count: p.array(unicode_list(&p)?)?.len(),
                        },
                    )
                    .is_none(),
                "duplicate boss language pack",
            )?;
        }
        need(
            !languages.is_empty(),
            "missing localized boss language packs",
        )?;
        packs.insert(stage, languages);
    }
    let stage = |entry: usize| {
        let (at, size) = entries[entry];
        afs.get(at..at + size)
            .ok_or_else(|| bad("card stage outside archive"))
    };
    let mut sources = Vec::new();
    for arena in ARENAS {
        sources.push((arena.entry, message_indexes(stage(arena.entry)?, arena)?));
    }
    for &(_, entry, rows) in FAMILY_CARDS {
        check_family_rows(stage(entry)?, rows)?;
        sources.push((entry, rows));
    }
    let mut cards = Vec::new();
    for (entry, rows) in sources {
        let mut texts = BTreeMap::new();
        for (name, language) in &packs[&entry] {
            let (_, bytes) = language_pack(language.entry)?;
            let p = Pack::parse(bytes)?;
            let messages = rows
                .map(|row| indexed_text(&p, row))
                .into_iter()
                .collect::<Result<Vec<_>>>()?
                .try_into()
                .map_err(|_| bad("missing boss messages"))?;
            texts.insert(name.clone(), messages);
        }
        cards.push(texts);
    }
    Ok(Library { packs, cards })
}

fn append_messages(bytes: &[u8], base: usize, messages: &[Messages]) -> Result<Vec<u8>> {
    let p = Pack::parse(bytes)?;
    let list = unicode_list(&p)?;
    let mut ids = p.array(list)?;
    let original_count = ids.len();
    need(
        original_count <= base && original_count > 0,
        "invalid boss message bank base",
    )?;
    // Every locale uses the same new indexes. Existing rows retain their IDs;
    // padding references an existing message and is never displayed.
    let template = ids[0];
    ids.resize(base, template);
    let mut builder = Builder::new(&p).reuse_directories();
    for text in messages.iter().flatten() {
        let memory = builder.mem(text.clone(), p.val(template, 3)?)?;
        ids.push(builder.obj(template, &[(3, memory)])?);
    }
    let template = p.val(list, 4)?;
    let memory = builder.mem(words(&ids), template)?;
    builder.modify(
        list,
        &[(2, ids.len() as u32), (3, ids.len() as u32), (4, memory)],
    )?;
    let output = materialize(bytes, &builder.finish(template)?)?;
    let after = Pack::parse(&output)?;
    let actual = after.array(list)?;
    need(
        actual[..original_count] == ids[..original_count],
        "original message IDs changed",
    )?;
    for row in 0..original_count {
        need(
            indexed_text(&after, row)? == indexed_text(&p, row)?,
            "unrelated stage message changed",
        )?;
    }
    for (index, text) in messages.iter().flatten().enumerate() {
        need(
            indexed_text(&after, base + index)? == *text,
            "boss message differs after writing",
        )?;
    }
    Ok(output)
}

fn bases(library: &Library) -> BTreeMap<usize, usize> {
    library
        .packs
        .iter()
        .map(|(&entry, languages)| {
            (
                entry,
                languages.values().map(|lang| lang.count).max().unwrap(),
            )
        })
        .collect()
}

pub(in crate::crown) fn message_bases(
    iso: &[u8],
    afs: &[u8],
    entries: &[(usize, usize)],
) -> Result<BTreeMap<usize, usize>> {
    Ok(bases(&load(iso, afs, entries)?))
}

pub(super) fn retarget(bytes: &[u8], arena: &Arena, boss: &str, base: usize) -> Result<Vec<u8>> {
    let donor = ARENAS
        .iter()
        .position(|a| a.model == boss)
        .ok_or_else(|| bad("unknown boss text donor"))?;
    let keys = selectors(bytes, arena)?;
    let names = keys
        .into_iter()
        .zip(
            FONTS
                .iter()
                .enumerate()
                .map(|(i, font)| format!("{font}I<{}>", base + 4 * donor + i)),
        )
        .collect();
    materialize(
        bytes,
        &lua_names::rename_in_pack(bytes, &[arena.intro.to_string()], &names)?,
    )
}

/// Append the card bank to the localized packs of `stages`.
pub(in crate::crown) fn build(
    iso: &[u8],
    afs: &[u8],
    entries: &[(usize, usize)],
    rewards: &super::super::rewards::Plan,
    table: &EnemyTable,
    stages: &BTreeSet<usize>,
) -> Result<Vec<u8>> {
    let library = load(iso, afs, entries)?;
    let bases = bases(&library);
    let (offset, area) = file(iso, "AREA.AFS")?;
    let balanced = rewards.balanced_copy(iso, offset, area.len(), table)?;
    let mut replacements = BTreeMap::new();
    for stage in stages {
        let languages = library
            .packs
            .get(stage)
            .ok_or_else(|| bad("stage has no supported intro card"))?;
        for (language, destination) in languages {
            let messages = library
                .cards
                .iter()
                .map(|card| {
                    card.get(language)
                        .cloned()
                        .ok_or_else(|| bad("missing donor boss translation"))
                })
                .collect::<Result<Vec<_>>>()?;
            let at = destination.offset - offset;
            let bytes = append_messages(
                &balanced[at..at + destination.size],
                bases[stage],
                &messages,
            )?;
            need(
                replacements.insert(destination.entry, bytes).is_none(),
                "shared boss language pack",
            )?;
        }
    }
    let count = u32at(area, 4)? as usize;
    let first = u32at(area, 8)? as usize;
    need(
        count > 0 && count < 65536 && first >= 16 + 8 * count && first <= area.len(),
        "invalid localized archive prefix",
    )?;
    let mut output = balanced[..first].to_vec();
    let mut lengths = Vec::new();
    for entry in 0..count {
        let at = u32at(area, 8 + 8 * entry)? as usize;
        let len = u32at(area, 12 + 8 * entry)? as usize;
        let original = balanced
            .get(
                at..at
                    .checked_add(len)
                    .ok_or_else(|| bad("localized archive extent overflow"))?,
            )
            .ok_or_else(|| bad("localized archive entry outside image"))?;
        let bytes = replacements
            .get(&entry)
            .map(Vec::as_slice)
            .unwrap_or(original);
        let at = output.len();
        put(&mut output, 8 + 8 * entry, at as u32);
        put(&mut output, 12 + 8 * entry, bytes.len() as u32);
        lengths.push(bytes.len());
        output.extend(bytes);
        output.resize(a2048(output.len()), 0);
    }
    if let Some(range) = super::super::archive::filename_table(area, count, first)? {
        let mut names = area[range].to_vec();
        for (entry, len) in lengths.into_iter().enumerate() {
            put(&mut names, 48 * entry + 44, len as u32);
        }
        let at = output.len();
        for pos in [8 + 8 * count, first - 8] {
            put(&mut output, pos, at as u32);
            put(&mut output, pos + 4, names.len() as u32);
        }
        output.extend(names);
        output.resize(a2048(output.len()), 0);
    }
    Ok(output)
}

/// Check a rebuilt AREA archive: each (stage, model, line, row) holds that
/// line of the family's retail card in every language. Returns the lines checked.
#[cfg(test)]
pub(in crate::crown) fn verify_intro_cards(
    iso: &[u8],
    afs: &[u8],
    entries: &[(usize, usize)],
    area: &[u8],
    lines: &[(usize, &str, usize, usize)],
) -> Result<usize> {
    let library = load(iso, afs, entries)?;
    let mut checked = 0;
    for &(stage, model, line, row) in lines {
        let card = FAMILY_CARDS
            .iter()
            .position(|(m, _, _)| *m == model)
            .ok_or_else(|| bad("introduction family has no card"))?;
        for (language, pack) in &library.packs[&stage] {
            let at = u32at(area, 8 + 8 * pack.entry)? as usize;
            let len = u32at(area, 12 + 8 * pack.entry)? as usize;
            let p = Pack::parse(&area[at..at + len])?;
            need(
                indexed_text(&p, row)? == library.cards[ARENAS.len() + card][language][line],
                "introduction card differs from its family",
            )?;
            checked += 1;
        }
    }
    Ok(checked)
}

/// Parse a card constant into its font line and message row.
#[cfg(test)]
pub(in crate::crown) fn card_line(constant: &str) -> Option<(usize, usize)> {
    FONTS.iter().enumerate().find_map(|(line, font)| {
        constant
            .strip_prefix(font)?
            .strip_prefix("I<")?
            .strip_suffix('>')?
            .parse()
            .ok()
            .map(|row| (line, row))
    })
}

#[test]
#[ignore]
fn regional_boss_cards_preserve_native_glyphs_and_other_messages() {
    let iso = std::fs::read(env::var_os("DW4_BOSS_AUDIT_ISO").expect("retail ISO")).unwrap();
    let (_, afs) = file(&iso, "NDMW.AFS").unwrap();
    let entries = (0..u32at(afs, 4).unwrap() as usize)
        .map(|i| {
            (
                u32at(afs, 8 + 8 * i).unwrap() as usize,
                u32at(afs, 12 + 8 * i).unwrap() as usize,
            )
        })
        .collect::<Vec<_>>();
    let library = load(&iso, afs, &entries).unwrap();
    let bases = bases(&library);
    assert_eq!(library.cards.len(), ARENAS.len() + FAMILY_CARDS.len());
    let (mut checked, mut intros) = (0, 0);
    for stage in card_destinations().collect::<BTreeSet<_>>() {
        let (at, size) = entries[stage];
        let model = &afs[at..at + size];
        let arena = ARENAS.iter().find(|a| a.entry == stage);
        let intro = INTRO_STAGES.iter().find(|s| s.model_entry == stage);
        for (language, destination) in &library.packs[&stage] {
            let messages = library
                .cards
                .iter()
                .map(|card| card[language].clone())
                .collect::<Vec<_>>();
            let output = append_messages(
                &iso[destination.offset..destination.offset + destination.size],
                bases[&stage],
                &messages,
            )
            .unwrap();
            let p = Pack::parse(&output).unwrap();
            for (index, text) in messages.iter().enumerate() {
                for (line, message) in text.iter().enumerate() {
                    assert_eq!(
                        indexed_text(&p, bases[&stage] + 4 * index + line).unwrap(),
                        *message
                    );
                }
                let expected = std::array::from_fn(|i| bases[&stage] + 4 * index + i);
                if let (Some(arena), Some(donor)) = (arena, ARENAS.get(index)) {
                    let script = retarget(model, arena, donor.model, bases[&stage]).unwrap();
                    assert_eq!(message_indexes(&script, arena).unwrap(), expected);
                    checked += 1;
                }
                let Some((intro, card)) = intro.zip(index.checked_sub(ARENAS.len())) else {
                    continue;
                };
                for &(_, native) in intro.sites {
                    let (_, _, rows) = FAMILY_CARDS
                        .iter()
                        .find(|(m, entry, _)| *m == native && *entry == stage)
                        .expect("every supported introduction shows a retail card");
                    let others = intro
                        .sites
                        .iter()
                        .filter(|(_, m)| *m != native)
                        .map(|(_, m)| (*m, *m));
                    let picks = std::iter::once((native, FAMILY_CARDS[card].0))
                        .chain(others)
                        .collect::<Vec<_>>();
                    let names = intro_cards(stage, &picks, bases[&stage]).unwrap();
                    if !names.is_empty() {
                        let edits =
                            lua_names::rename_in_pack(model, &[intro.script.to_string()], &names)
                                .unwrap();
                        let renamed = materialize(model, &edits).unwrap();
                        let constants = lua_names::constants(
                            script(&Pack::parse(&renamed).unwrap(), intro.script).unwrap(),
                        )
                        .unwrap();
                        for to in names.values() {
                            assert!(constants.iter().any(|c| c.text == to.as_bytes()));
                        }
                    }
                    for (line, &row) in rows.iter().enumerate() {
                        let shared = picks.iter().skip(1).any(|(other, _)| {
                            FAMILY_CARDS.iter().any(|(m, entry, r)| {
                                m == other && *entry == stage && r[line] == row
                            })
                        });
                        let from = card_constant(row, line);
                        if FAMILY_CARDS[card].0 == native {
                            assert!(!names.contains_key(&from));
                        } else if shared {
                            assert!(!names.contains_key(&from), "shared card row changed");
                        } else {
                            assert_eq!(names[&from], card_constant(expected[line], line));
                        }
                    }
                    intros += 1;
                }
            }
        }
    }
    eprintln!("REGIONAL BOSS CARDS: {checked} arena/donor/language combinations preserve native glyph bytes and all unrelated stage messages");
    eprintln!("REGIONAL INTRO CARDS: {intros} introduction/family/language combinations select the appended family card");
}
#[cfg(all(test, feature = "devtools"))]
pub(in crate::crown::random) fn archive(iso: &[u8]) -> Result<&[u8]> {
    let (at, size) = if crate::release::detect(iso)?.is_gamecube() {
        let f = crate::gcm::find_file(iso, "NDMW.AFS")?;
        (f.offset as usize, f.size as usize)
    } else {
        let f = crate::iso9660::find_file(iso, "NDMW.AFS")?;
        (f.lba as usize * 2048, f.size as usize)
    };
    iso.get(at..at + size)
        .ok_or_else(|| bad("boss archive outside image"))
}

#[cfg(all(test, feature = "devtools"))]
pub(in crate::crown::random) fn verify_image(
    source: &[u8],
    written: &[u8],
    roster: &serde_json::Value,
) -> Result<()> {
    let original = archive(source)?;
    let output = archive(written)?;
    let entries = |afs: &[u8]| -> Result<Vec<(usize, usize)>> {
        (0..u32at(afs, 4)? as usize)
            .map(|i| {
                Ok((
                    u32at(afs, 8 + 8 * i)? as usize,
                    u32at(afs, 12 + 8 * i)? as usize,
                ))
            })
            .collect()
    };
    let old_entries = entries(original)?;
    let new_entries = entries(output)?;
    let library = load(source, original, &old_entries)?;
    let (_, area) = file(written, "AREA.AFS")?;
    let count = u32at(area, 4)? as usize;
    let first = u32at(area, 8)? as usize;
    let mut cursor = first;
    for i in 0..count {
        need(
            u32at(area, 8 + 8 * i)? as usize == cursor,
            "localized AFS entry is not contiguous",
        )?;
        cursor = a2048(cursor + u32at(area, 12 + 8 * i)? as usize);
    }
    if let Some(table) = super::super::archive::filename_table(area, count, first)? {
        need(
            table.start == cursor && a2048(table.end) == area.len(),
            "localized AFS filename table is misplaced",
        )?;
    } else {
        need(
            cursor == area.len(),
            "localized AFS has unindexed trailing data",
        )?;
    }
    let mut variants = BTreeMap::new();
    for &(at, size) in &new_entries {
        let bytes = &output[at..at + size];
        if size > 100_000 || memchr::memmem::find(bytes, b"beFileListInfo\0").is_none() {
            continue;
        }
        let p = Pack::parse(bytes)?;
        for arena in ARENAS {
            let prefix = format!("[0.{}]|", arena.entry);
            for r in &p.objects {
                if p.classes[r.class].name != "igNamedObject" {
                    continue;
                }
                let Some(name) = graft::name_of(&p, r) else {
                    continue;
                };
                if !name.starts_with(prefix.as_bytes()) {
                    continue;
                }
                let names =
                    std::str::from_utf8(name).map_err(|_| bad("invalid boss file selector"))?;
                let indexes = names
                    .split('|')
                    .map(|s| {
                        s.strip_prefix("[0.")
                            .and_then(|s| s.strip_suffix(']'))
                            .and_then(|s| s.parse::<usize>().ok())
                            .ok_or_else(|| bad("invalid boss difficulty file"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let indexes: [usize; 3] = indexes
                    .try_into()
                    .map_err(|_| bad("missing boss difficulty file"))?;
                if let Some(previous) = variants.insert(arena.entry, indexes) {
                    need(previous == indexes, "conflicting boss difficulty files")?;
                }
            }
        }
    }
    let mut checked = 0;
    let mut localized = 0;
    for (tier, report) in roster["tiers"]
        .as_array()
        .ok_or_else(|| bad("missing boss tiers"))?
        .iter()
        .enumerate()
    {
        for arena in ARENAS {
            let boss = report["bosses"]
                .as_array()
                .and_then(|bosses| {
                    bosses
                        .iter()
                        .find(|boss| boss["stage"].as_u64() == Some(arena.entry as u64))
                })
                .and_then(|boss| boss["boss"].as_str())
                .ok_or_else(|| bad("missing boss selection"))?;
            let native_files = [arena.entry; 3];
            let indexes = match variants.get(&arena.entry) {
                Some(indexes) => indexes,
                None if boss == arena.model => &native_files,
                None => return Err(bad("missing boss resource variants")),
            };
            let (at, size) = new_entries[indexes[tier]];
            let bytes = &output[at..at + size];
            let (old_at, old_size) = old_entries[arena.entry];
            let native = Pack::parse(&original[old_at..old_at + old_size])?;
            let current = Pack::parse(bytes)?;
            let replaced_command = match arena.entry {
                210 => Some("SQ_WALKIN"),
                296 => Some("SQ_SWON"),
                _ => None,
            };
            if boss != arena.model {
                let sequences = model_sequences(&current, boss)?;
                if replaced_command.is_some_and(|command| !sequences.contains(command))
                    && sequences.contains("SQ_ENTR")
                {
                    let constants = lua_names::constants(script(&current, arena.intro)?)?;
                    need(
                        constants.iter().any(|c| c.text == b"SQ_ENTR")
                            && !constants.iter().any(|c| c.text == b"SQ_NENTR"),
                        "boss activation selected normal-enemy death behavior",
                    )?;
                }
            }
            need(
                graft::model_is_drawn(&current, boss)?,
                "final boss is not attached to a registered draw scene",
            )?;
            let registry = u32at(current.b, current.table + 8 + 2 * current.ids.len())?;
            for resource in current.array(registry)? {
                need(
                    resource != NULL && current.rec(resource).is_ok(),
                    "active boss-stage registry contains an empty resource",
                )?;
            }
            let death = script(&native, &format!("ded_{}.lua", arena.model))?;
            let expected_death = if boss == arena.model {
                death.to_vec()
            } else {
                lua_names::rename(
                    death,
                    &BTreeMap::from([
                        (arena.model.to_string(), boss.to_string()),
                        (format!("{}/I<0>", arena.model), format!("{boss}/I<0>")),
                    ]),
                )?
                .ok_or_else(|| bad("missing boss victory model reference"))?
            };
            need(
                script(&current, &format!("ded_{boss}.lua"))? == expected_death,
                "final boss victory flags, doors or rewards differ from destination",
            )?;
            need(
                script(&current, &format!("ded_{}.lua", arena.model))? == expected_death,
                "original arena victory callback alias was removed or changed",
            )?;
            let (_, area) = file(written, "AREA.AFS")?;
            let rows = message_indexes(bytes, arena)?;
            let donor = ARENAS.iter().position(|a| a.model == boss).unwrap();
            for (language, original) in &library.packs[&arena.entry] {
                let at = u32at(area, 8 + 8 * original.entry)? as usize;
                let len = u32at(area, 12 + 8 * original.entry)? as usize;
                let p = Pack::parse(&area[at..at + len])?;
                let expected = &library.cards[donor][language];
                for (row, text) in rows.into_iter().zip(expected) {
                    need(
                        indexed_text(&p, row)? == *text,
                        "localized boss intro differs from spawned boss",
                    )?;
                }
                localized += 1;
                // The original AREA bytes remain in the image and carry the
                // same final balance. Compare all original memory and objects,
                // apart from the extended Unicode list, independently of build.
                let native =
                    Pack::parse(&written[original.offset..original.offset + original.size])?;
                let list = unicode_list(&native)?;
                for (id, memory) in native.memory.iter().enumerate() {
                    if let Some(range) = memory {
                        let actual = p
                            .memory
                            .get(id)
                            .and_then(Option::as_ref)
                            .ok_or_else(|| bad("localized memory missing"))?;
                        need(
                            native.b[range.clone()] == p.b[actual.clone()],
                            "localized database or original memory changed",
                        )?;
                    } else if native.ids[id].is_some() && id as u32 != list {
                        need(
                            native.b[native.rec(id as u32)?.range.clone()]
                                == p.b[p.rec(id as u32)?.range.clone()],
                            "localized resource changed",
                        )?;
                    }
                }
            }
            checked += 1;
        }
    }
    need(
        checked == ARENAS.len() * 3,
        "missing boss intro verification",
    )?;
    eprintln!(
        "BOSS INTRO IMAGE: {checked} boss cards match the final roster across all difficulties and all seven language packs"
    );
    eprintln!("BOSS VICTORY IMAGE: {checked} death callbacks preserve the destination's complete flag, door and reward bytecode");
    eprintln!("BOSS LOCALIZED IMAGE: {localized} language/difficulty cards; original messages, database memory and resource IDs preserved; native AFS layout verified");
    Ok(())
}
