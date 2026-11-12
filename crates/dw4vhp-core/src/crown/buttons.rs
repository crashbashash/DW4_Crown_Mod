//! Solo co-op button pairs. `g_dbutton2` releases as soon as its rider steps
//! off and `g_dwbutton2` releases after a short timer, so a room with a pair
//! of them needs two players even in single player. When both buttons of a
//! pair are down, retail holds them with `SQ_ONLOOP` and cancels a pending
//! timed release by selecting the empty section of the switch-off sequence
//! (`SETLABEL "SQ_SWOF/OFF"`). In single player, each pair button's own hit
//! script does both as it is pressed, so a pressed button stays down until
//! its partner is pressed. Rooms with three or four buttons already scale
//! with the party (retail pre-locks the extra buttons) and stay unchanged.
use super::river::lua::{call, conditional, signed, Chunk, BIAS};
use super::*;
use std::collections::BTreeSet;

const MODELS: [&str; 2] = ["g_dbutton2", "g_dwbutton2"];

/// Whether `bytes` may generate a co-op button.
pub(super) fn candidate(bytes: &[u8]) -> bool {
    MODELS
        .iter()
        .any(|m| memchr::memmem::find(bytes, format!("{m}\0").as_bytes()).is_some())
}

/// Latch the stage's co-op button pairs in single player. `None` when it has none.
pub(super) fn latch(pack: &[u8]) -> Result<Option<Vec<u8>>> {
    if !candidate(pack) {
        return Ok(None);
    }
    let p = Pack::parse(pack)?;
    let mut instances = BTreeMap::<&str, BTreeSet<i64>>::new();
    for id in 0..p.ids.len() as u32 {
        if p.class(id, "beLuaData").is_err() {
            continue;
        }
        let object = p.val(id, 3)?;
        let range = p
            .memory
            .get(p.val(object, 3)? as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing Lua memory"))?;
        let found = lua_names::constants(&pack[range.clone()])?;
        if !found.iter().any(|c| c.text == b"GENERATE")
            || !MODELS
                .iter()
                .any(|m| found.iter().any(|c| c.text == m.as_bytes()))
        {
            continue;
        }
        for (model, ram) in generate_calls(&Chunk::read(&pack[range.clone()])?) {
            instances.entry(model).or_default().insert(ram);
        }
    }
    let scripts = instances
        .into_iter()
        .filter(|(_, ram)| *ram == BTreeSet::from([0, 1]))
        .map(|(model, _)| format!("hit_{model}.lua"))
        .collect::<Vec<_>>();
    if scripts.is_empty() {
        return Ok(None);
    }
    let edits = lua_names::rewrite_in_pack(pack, &scripts, |_, raw| hold_on_hit(raw))?;
    Ok(Some(materialize(pack, &edits)?))
}

/// `be:sd("MODELCTRL", button, n, "GENERATE", "<literal>")`: the model and
/// its instance number `n`.
fn generate_calls(c: &Chunk) -> Vec<(&'static str, i64)> {
    (4..c.code.len().saturating_sub(2))
        .filter_map(|pc| {
            let model = MODELS.into_iter().find(|m| c.is(c.code[pc - 2], 7, m))?;
            (c.is(c.code[pc], 7, "GENERATE")
                && c.code[pc - 1] & 63 == 6
                && c.is(c.code[pc - 3], 7, "MODELCTRL")
                && c.is(c.code[pc - 4], 16, "sd")
                && c.code[pc + 1] & 63 == 7
                && c.code[pc + 2] & 63 == 2)
                .then(|| (model, i64::from(c.code[pc - 1] >> 6) - BIAS))
        })
        .collect()
}

/// Prefix the hit script with
/// `if SYSgetPlayerNinzu() <= 1 then MDLsetSD("SETLABEL", "SQ_SWOF/OFF"); MDLsetSeq("SQ_ONLOOP") end`.
/// No local is live before the first statement, so the calls use stack base 0.
fn hold_on_hit(raw: &[u8]) -> Result<Vec<u8>> {
    let mut c = Chunk::read(raw)?;
    let mut code = c.method("SYSgetPlayerNinzu").to_vec();
    code.extend([call(0, 1), signed(6, 1)]);
    // JMPGT: skip the latch when more than one player joined.
    conditional(&mut code, 36, |out| {
        out.extend(c.method("MDLsetSD"));
        out.extend([c.literal("SETLABEL"), c.literal("SQ_SWOF/OFF"), call(0, 0)]);
        out.extend(c.method("MDLsetSeq"));
        out.extend([c.literal("SQ_ONLOOP"), call(0, 0)]);
    });
    let code = c.rewrite(
        &BTreeMap::from([(0, code)]),
        &BTreeMap::new(),
        &BTreeMap::new(),
    )?;
    c.write(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two-button rooms latch in single player; rooms whose button count
    /// scales with the party (three or four buttons) stay retail.
    #[test]
    fn retail_two_player_button_pairs_latch_in_single_player() {
        let expected: BTreeSet<usize> = [
            218, 226, 233, 249, 265, 273, 327, 441, 486, 494, 502, 510, 518, 526,
        ]
        .into();
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
            let (base, size) = if crate::release::detect(&iso).unwrap().is_gamecube() {
                let f = crate::gcm::find_file(&iso, "ndmw.afs").unwrap();
                (f.offset as usize, f.size as usize)
            } else {
                let f = crate::iso9660::find_file(&iso, "NDMW.AFS").unwrap();
                (f.lba as usize * 2048, f.size as usize)
            };
            let afs = &iso[base..base + size];
            let mut latched = BTreeSet::new();
            for i in 0..u32at(afs, 4).unwrap() as usize {
                let off = u32at(afs, 8 + 8 * i).unwrap() as usize;
                let len = u32at(afs, 12 + 8 * i).unwrap() as usize;
                let Some(out) = latch(&afs[off..off + len]).unwrap() else {
                    continue;
                };
                // Each pair button's hit script starts with the guarded latch
                // and keeps the retail handler after it.
                let before = hit_scripts(&afs[off..off + len]);
                let after = hit_scripts(&out);
                let mut hits = 0;
                for (name, code) in &after {
                    let old = &before[name];
                    if code == old {
                        continue;
                    }
                    let added = code.len() - old.len();
                    assert_eq!(&code[added..], &old[..], "{variable} entry {i} {name}");
                    let c = Chunk::read(&chunk(&out, name)).unwrap();
                    let text = |s| {
                        code[..added]
                            .iter()
                            .any(|&x| c.is(x, 7, s) || c.is(x, 16, s))
                    };
                    assert!(
                        [
                            "SYSgetPlayerNinzu",
                            "MDLsetSD",
                            "SETLABEL",
                            "SQ_SWOF/OFF",
                            "MDLsetSeq",
                            "SQ_ONLOOP"
                        ]
                        .into_iter()
                        .all(text),
                        "{variable} entry {i} {name}"
                    );
                    hits += 1;
                }
                assert!(hits > 0, "{variable} entry {i}");
                latched.insert(i);
            }
            assert_eq!(latched, expected, "{variable}");
        }
    }

    fn chunk(pack: &[u8], name: &str) -> Vec<u8> {
        let p = Pack::parse(pack).unwrap();
        (0..p.ids.len() as u32)
            .filter(|&id| p.class(id, "beLuaData").is_ok())
            .find(|&id| graft::name_of(&p, p.rec(id).unwrap()) == Some(name.as_bytes()))
            .map(|id| {
                let object = p.val(id, 3).unwrap();
                pack[p.memory[p.val(object, 3).unwrap() as usize]
                    .clone()
                    .unwrap()]
                .to_vec()
            })
            .unwrap()
    }

    /// The code of every `hit_<button>.lua` script in the pack.
    fn hit_scripts(pack: &[u8]) -> BTreeMap<String, Vec<u32>> {
        MODELS
            .iter()
            .map(|m| format!("hit_{m}.lua"))
            .filter(|name| memchr::memmem::find(pack, name.as_bytes()).is_some())
            .map(|name| {
                let code = Chunk::read(&chunk(pack, &name)).unwrap().code;
                (name, code)
            })
            .collect()
    }
}
