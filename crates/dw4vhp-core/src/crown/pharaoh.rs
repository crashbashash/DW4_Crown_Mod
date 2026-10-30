//! Per-generation Pharaohmon summons. Keep the retail callback's control flow,
//! twelve logical summons, positions, rotations and magic coordination. Only
//! model/actor lookups and the three difficulty-specific spawn prefixes change.
use super::river::lua::{call, conditional, signed, unsigned, Chunk};
use super::*;

pub(super) const LIMIT: usize = 12;
const MODELS: &str = "g_crownPharaohModels";
const RAMS: &str = "g_crownPharaohRams";
const PICK: &str = "g_crownPharaohPick";
const LAST: &str = "g_crownPharaohLast";
const COUNT: &str = "g_crownPharaohChoices";
const PREFIXES: [&str; 3] = [
    "g_crownPharaohNormal",
    "g_crownPharaohHard",
    "g_crownPharaohVeryHard",
];

#[derive(Debug)]
pub(super) struct Target {
    pub model: String,
    pub entrance: String,
    /// Summon-only actors, above every original and planned generator actor.
    pub base: usize,
    /// Local encounter variants, one per difficulty.
    pub rows: [usize; 3],
}

fn table_get(c: &mut Chunk, table: &str, index: u32) -> Vec<u32> {
    vec![c.get(table), index, 13]
}

fn table_set(c: &mut Chunk, table: &str, index: u32, value: Vec<u32>) -> Vec<u32> {
    let mut code = vec![c.get(table), index];
    code.extend(value);
    code.push(20 | (3 << 15) | (3 << 6));
    code
}

fn prefix(row: usize, difficulty: usize, entrance: &str) -> String {
    // Keep the second authored GENERATE parameter unchanged.
    format!("{entrance}/I<{row}>/I<{}>/", [149, 896, 2738][difficulty])
}

fn callback(raw: &[u8], targets: &[Target], entry: usize) -> Result<Vec<u8>> {
    need(!targets.is_empty(), "Pharaohmon needs a summon family")?;
    let mut c = Chunk::read(raw)?;
    let original = c.code.clone();
    let mut before = BTreeMap::<usize, Vec<u32>>::new();
    let mut replace = BTreeMap::new();
    // These are stage-local globals, like the retail callback's g_magic/g_swN.
    // Never reset them on another summoner's callback or on another generation.
    let mut init = vec![c.get("g_crownPharaohStage"), signed(6, entry as i64)];
    conditional(&mut init, 33, |out| {
        out.extend([signed(6, entry as i64), c.set("g_crownPharaohStage")]);
        out.extend([signed(6, -1), c.set(LAST)]);
        out.extend([unsigned(17, LIMIT), c.set(MODELS)]);
        out.extend([unsigned(17, LIMIT), c.set(RAMS)]);
        for slot in 0..LIMIT {
            let value = vec![c.literal(&targets[0].model)];
            out.extend(table_set(&mut c, MODELS, signed(6, slot as i64), value));
            out.extend(table_set(
                &mut c,
                RAMS,
                signed(6, slot as i64),
                vec![signed(6, (targets[0].base + slot) as i64)],
            ));
        }
    });
    before.insert(0, init);
    let mut lookups = 0;
    let mut generations = Vec::new();
    for pc in 0..original.len() {
        if !c.is(original[pc], 7, "e_mummy") {
            continue;
        }
        need(
            original.get(pc + 1).is_some_and(|i| i & 63 == 11),
            "unexpected Pharaohmon summon slot",
        )?;
        let index = original[pc + 1];
        let generate = original
            .get(pc + 2)
            .is_some_and(|&i| c.is(i, 7, "GENERATE"));
        if generate {
            let difficulty = generations.len();
            need(
                difficulty < 3
                    && original.get(pc + 3).is_some_and(|&i| {
                        c.is(
                            i,
                            7,
                            &prefix([116, 120, 124][difficulty], difficulty, "SQ_NENTR"),
                        )
                    }),
                "unexpected Pharaohmon summon prefix",
            )?;
            replace.insert(pc + 3, c.get(PREFIXES[difficulty]));
            generations.push(pc);
        }
        let mut model = table_get(&mut c, MODELS, index);
        let last = model.pop().unwrap();
        before.entry(pc).or_default().extend(model);
        replace.insert(pc, last);
        let mut ram = table_get(&mut c, RAMS, index);
        let last = ram.pop().unwrap();
        before.entry(pc + 1).or_default().extend(ram);
        replace.insert(pc + 1, last);
        lookups += 1;
    }
    need(
        lookups == 7 && generations.len() == 3,
        "unexpected Pharaohmon summon callback",
    )?;
    // Select once after the old occupant is found dead, before difficulty
    // dispatch. The native loop invokes this again for every vacant slot.
    let first = generations[0];
    need(
        first >= 8
            && c.is(original[first - 7], 16, "SYSgetDifficulty")
            && original[first - 6] & 63 == 2,
        "unexpected Pharaohmon difficulty dispatch",
    )?;
    let stack = (original[first - 6] >> 15) as usize;
    let index = original[first + 1];
    let mut choose = vec![signed(6, targets.len() as i64), c.set(COUNT)];
    if targets.len() > 1 {
        choose.extend([c.get(LAST), signed(6, 0)]);
        conditional(&mut choose, 34, |out| {
            out.extend([signed(6, targets.len() as i64 - 1), c.set(COUNT)]);
        });
    }
    choose.extend(c.method("SYSgetRandom"));
    choose.extend([c.get(COUNT), call(stack, 1), c.set(PICK)]);
    if targets.len() > 1 {
        choose.extend([c.get(LAST), signed(6, 0)]);
        conditional(&mut choose, 34, |out| {
            out.extend([c.get(PICK), c.get(LAST)]);
            conditional(out, 34, |out| {
                out.extend([c.get(PICK), signed(6, 1), 23, c.set(PICK)]);
            });
        });
    }
    choose.extend([c.get(PICK), c.set(LAST)]);
    for (i, target) in targets.iter().enumerate() {
        choose.extend([c.get(PICK), signed(6, i as i64)]);
        conditional(&mut choose, 32, |out| {
            let value = vec![c.literal(&target.model)];
            out.extend(table_set(&mut c, MODELS, index, value));
            out.extend(table_set(
                &mut c,
                RAMS,
                index,
                vec![index, signed(6, target.base as i64), 23],
            ));
            for (difficulty, name) in PREFIXES.iter().enumerate() {
                out.extend([
                    c.literal(&prefix(
                        target.rows[difficulty],
                        difficulty,
                        &target.entrance,
                    )),
                    c.set(name),
                ]);
            }
        });
    }
    before.entry(first - 8).or_default().extend(choose);
    let code = c.rewrite(&before, &BTreeMap::new(), &replace)?;
    c.write(&code)
}

/// Mummymon's request/hit callbacks address g_sw0..11 using its actor number.
/// Translate only the new summon actors back to their logical summon index;
/// ordinary Mummymon actors retain their authored indices and scripts otherwise.
fn mummy(raw: &[u8], base: usize) -> Result<Vec<u8>> {
    let c = Chunk::read(raw)?;
    let mut before = BTreeMap::new();
    let mut found = 0;
    for pc in 0..c.code.len().saturating_sub(2) {
        if found == 0
            && c.is(c.code[pc], 12, "be")
            && c.is(c.code[pc + 1], 16, "MDLgetRamNo")
            && c.code[pc + 2] & 63 == 2
        {
            let stack = (c.code[pc + 2] >> 15) as usize;
            need(
                c.code[pc + 2] == call(0, 1),
                "unexpected Mummymon logical index local",
            )?;
            // Original call leaves the actor index in the new local slot.
            let local = unsigned(11, stack);
            let mut code = vec![local, signed(6, base as i64)];
            conditional(&mut code, 34, |out| {
                out.extend([local, signed(6, (base + LIMIT) as i64)]);
                conditional(out, 37, |out| {
                    out.extend([local, signed(6, base as i64), 25, unsigned(18, stack)])
                });
            });
            before.insert(pc + 3, code);
            found += 1;
        }
    }
    need(found == 1, "unexpected Mummymon actor lookup")?;
    let code = c.rewrite(&before, &BTreeMap::new(), &BTreeMap::new())?;
    c.write(&code)
}

pub(super) fn rewrite(b: &[u8], targets: &[Target], entry: usize) -> Result<Vec<u8>> {
    let p = Pack::parse(b)?;
    let mut builder = Builder::new(&p);
    let mut template = None;
    let mut changed = 0;
    for (name, base) in std::iter::once(("cal_e_pharaoh.lua", None)).chain(
        targets
            .iter()
            .find(|t| t.model == "e_mummy")
            .into_iter()
            .flat_map(|t| {
                [
                    ("hit_e_mummy.lua", Some(t.base)),
                    ("mag_e_mummy.lua", Some(t.base)),
                ]
            }),
    ) {
        let id = graft::find_named(&p, "beLuaData", name)?
            .ok_or_else(|| bad(&format!("missing summon script {name}")))?;
        let object = p.val(id, 3)?;
        let mem = p.val(object, 3)?;
        let range = p
            .memory
            .get(mem as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| bad("missing summon Lua memory"))?;
        need(
            range.len() == p.val(object, 2)? as usize,
            "unexpected summon script size",
        )?;
        let raw = &b[range.clone()];
        let output = match base {
            None => callback(raw, targets, entry)?,
            Some(base) => mummy(raw, base)?,
        };
        let len = output.len();
        let newmem = builder.mem(output, mem)?;
        builder.modify(object, &[(2, len as u32), (3, newmem)])?;
        template = Some(mem);
        changed += 1;
    }
    need(changed > 0, "missing Pharaohmon summon callback")?;
    materialize(b, &builder.finish(template.unwrap())?)
}
