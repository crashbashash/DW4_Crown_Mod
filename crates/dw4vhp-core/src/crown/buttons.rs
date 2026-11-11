//! Solo co-op button pairs. `g_dbutton2` releases as soon as its rider steps
//! off and `g_dwbutton2` releases after a short timer, so a room with a pair
//! of them needs two players even in single player. Retail latches a finished
//! button by disabling its switch-off label (`SETLABEL "SQ_SWOF/OFF"`). In
//! single player, do that as each button of a pair is generated: a pressed
//! button stays down until its partner is pressed. Rooms with three or four
//! buttons already scale with the party (retail pre-locks the extra buttons)
//! and stay unchanged.
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
    let mut scripts = Vec::new();
    let mut instances = BTreeMap::<&str, BTreeSet<i64>>::new();
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
        if !found.iter().any(|c| c.text == b"GENERATE")
            || !MODELS
                .iter()
                .any(|m| found.iter().any(|c| c.text == m.as_bytes()))
        {
            continue;
        }
        let calls = generate_calls(&Chunk::read(&pack[range.clone()])?);
        if !calls.is_empty() {
            for &(_, model, ram) in &calls {
                instances.entry(model).or_default().insert(ram);
            }
            let models = calls.iter().map(|&(_, m, _)| m).collect::<BTreeSet<_>>();
            scripts.push((String::from_utf8_lossy(name).into_owned(), models));
        }
    }
    let pairs: BTreeSet<&str> = instances
        .into_iter()
        .filter(|(_, ram)| *ram == BTreeSet::from([0, 1]))
        .map(|(model, _)| model)
        .collect();
    let scripts = scripts
        .into_iter()
        .filter(|(_, models)| !models.is_disjoint(&pairs))
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    if scripts.is_empty() {
        return Ok(None);
    }
    let edits = lua_names::rewrite_in_pack(pack, &scripts, |_, raw| rewrite(raw, &pairs))?;
    Ok(Some(materialize(pack, &edits)?))
}

/// `be:sd("MODELCTRL", button, n, "GENERATE", "<literal>")`: the call's
/// position, the model and its instance number `n`.
fn generate_calls(c: &Chunk) -> Vec<(usize, &'static str, i64)> {
    (4..c.code.len().saturating_sub(2))
        .filter_map(|pc| {
            let model = MODELS.into_iter().find(|m| c.is(c.code[pc - 2], 7, m))?;
            (c.is(c.code[pc], 7, "GENERATE")
                && c.code[pc - 1] & 63 == 6
                && c.is(c.code[pc - 3], 7, "MODELCTRL")
                && c.is(c.code[pc - 4], 16, "sd")
                && c.code[pc + 1] & 63 == 7
                && c.code[pc + 2] & 63 == 2)
                .then(|| (pc + 2, model, i64::from(c.code[pc - 1] >> 6) - BIAS))
        })
        .collect()
}

fn rewrite(raw: &[u8], pairs: &BTreeSet<&str>) -> Result<Vec<u8>> {
    let mut c = Chunk::read(raw)?;
    let calls = generate_calls(&c);
    let mut after = BTreeMap::new();
    for (at, model, _) in calls.into_iter().filter(|(_, m, _)| pairs.contains(m)) {
        let generate = c.code[at];
        // The statement's call base is free again once GENERATE returns.
        let stack = (generate >> 15) as usize;
        let ram = c.code[at - 3];
        let mut code = c.method("SYSgetPlayerNinzu").to_vec();
        code.extend([call(stack, 1), signed(6, 1)]);
        // JMPGT: skip the latch when more than one player joined.
        conditional(&mut code, 36, |out| {
            out.extend(c.method("sd"));
            out.extend([
                c.literal("MODELCTRL"),
                c.literal(model),
                ram,
                c.literal("SETLABEL"),
                c.literal("SQ_SWOF/OFF"),
                call(stack, 0),
            ]);
        });
        after.insert(at, code);
    }
    need(!after.is_empty(), "no co-op button pair generation")?;
    let code = c.rewrite(&BTreeMap::new(), &after, &BTreeMap::new())?;
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
                // Every pair button gets exactly one guarded latch.
                let p = Pack::parse(&out).unwrap();
                let mut labels = 0;
                let mut pairs = 0;
                for id in 0..p.ids.len() as u32 {
                    if p.class(id, "beLuaData").is_err() {
                        continue;
                    }
                    let object = p.val(id, 3).unwrap();
                    let range = p.memory[p.val(object, 3).unwrap() as usize]
                        .clone()
                        .unwrap();
                    let Ok(c) = Chunk::read(&out[range]) else {
                        continue;
                    };
                    let calls = generate_calls(&c);
                    for &(at, model, _) in &calls {
                        let latches = c.code[at + 1..]
                            .iter()
                            .take(20)
                            .any(|&i| c.is(i, 7, "SQ_SWOF/OFF"));
                        assert!(latches || !matches!(model, "g_dbutton2" | "g_dwbutton2"));
                        labels += usize::from(latches);
                    }
                    pairs += calls.len();
                }
                assert!(labels > 0 && labels <= pairs, "{variable} entry {i}");
                latched.insert(i);
            }
            assert_eq!(latched, expected, "{variable}");
        }
    }
}
