//! Numenume River's missile damage and cosmetic turret crowns. These objects
//! use scripted durability and the shared boat, not the 649-row enemy table.
use super::*;
mod lua;
use lua::{call, conditional, signed, unsigned, Chunk, BIAS};

const TURRET: &str = "g_numepodz01";
const MISSILE_KIND: i64 = 70108;
const FIRST_CROWN: i64 = 56;
const DAMAGE: &str = "g_crownRiverDamage";
const PREVIOUS: &str = "g_crownRiverPreviousHp";
const MATCH_HP: &str = "g_crownRiverThreshold";
const RNG: &str = "g_crownRiverColourRng";
const COLOUR: &str = "g_crownRiverColour";

fn nonnegative(c: &mut Chunk, name: &str) -> Vec<u32> {
    let mut out = vec![c.get(name), signed(6, 0)];
    conditional(&mut out, 37, |out| out.extend([signed(6, 0), c.set(name)]));
    out
}

fn damage(c: &mut Chunk) -> Result<Vec<u8>> {
    let mut before = BTreeMap::new();
    let mut replace = BTreeMap::new();
    let mut prefix = vec![
        c.get("g_dmgnum"),
        c.set(DAMAGE),
        c.get("g_boathp"),
        c.set(PREVIOUS),
    ];
    prefix.extend(c.method("MDLgetBulletKind"));
    prefix.extend([call(0, 1), signed(6, MISSILE_KIND)]);
    conditional(&mut prefix, 32, |out| {
        out.extend([c.get("g_dmgnum"), signed(6, 3), 26, c.set(DAMAGE)])
    });
    before.insert(0, prefix);
    let mut changes = 0;
    let mut thresholds = 0;
    for pc in 0..c.code.len() {
        let ins = c.code[pc];
        if c.is(ins, 12, "g_dmgnum") {
            replace.insert(pc, c.get(DAMAGE));
            changes += 1;
        }
        if c.is(ins, 19, "g_boathp") {
            before
                .entry(pc + 1)
                .or_insert_with(Vec::new)
                .extend(nonnegative(c, "g_boathp"));
        }
        if c.is(ins, 19, "g_meta") {
            before
                .entry(pc + 1)
                .or_insert_with(Vec::new)
                .extend(nonnegative(c, "g_meta"));
        }
        for n in 1..=4 {
            let threshold = format!("g_hpnum{n}");
            if pc + 2 < c.code.len()
                && c.is(ins, 12, "g_boathp")
                && c.is(c.code[pc + 1], 12, &threshold)
                && c.code[pc + 2] & 63 == 32
            {
                // Preserve the original effects/branches, selecting the current
                // damage band only when this hit crosses into it. Death must
                // also fire when a larger hit overshoots zero.
                let mut check = vec![
                    signed(6, 999999),
                    c.set(MATCH_HP),
                    c.get("g_boathp"),
                    c.get(&threshold),
                ];
                conditional(&mut check, 36, |out| {
                    if n == 4 {
                        out.extend([c.get(&threshold), c.set(MATCH_HP)]);
                    } else {
                        out.extend([c.get("g_boathp"), c.get(&format!("g_hpnum{}", n + 1))]);
                        conditional(out, 35, |out| {
                            out.extend([c.get(PREVIOUS), c.get(&threshold)]);
                            conditional(out, 35, |out| {
                                out.extend([c.get(&threshold), c.set(MATCH_HP)])
                            });
                        });
                    }
                });
                before.entry(pc).or_insert_with(Vec::new).extend(check);
                replace.insert(pc, c.get(MATCH_HP));
                thresholds += 1;
            }
        }
    }
    need(
        changes == 5 && thresholds == 4,
        "unexpected river boat damage script",
    )?;
    let code = c.rewrite(&before, &BTreeMap::new(), &replace)?;
    c.write(&code)
}

/// Independent cosmetic RNG, evaluated on each turret generation. Arithmetic
/// stays exact in Lua's doubles and requires no math library or game RNG calls.
fn colour(c: &mut Chunk, random: bool, seed: u32) -> Vec<u32> {
    let seed = if random { seed } else { 0 };
    let mut out = vec![
        signed(6, 5),
        c.set(COLOUR),
        signed(6, i64::from(random)),
        signed(6, 1),
    ];
    conditional(&mut out, 32, |out| {
        out.push(c.get(RNG));
        conditional(out, 38, |out| {
            out.extend([signed(6, (seed & 0xffff) as i64), c.set(RNG)])
        });
        out.extend([
            c.get(RNG),
            signed(6, 25173),
            26,
            signed(6, 13849),
            23,
            c.set(RNG),
        ]);
        for bit in (0..15).rev() {
            // PUSHINT has a signed 25-bit operand. Large exact powers are
            // built with multiplication instead of adding number constants.
            out.push(c.get(RNG));
            out.extend([signed(6, 65536), signed(6, 1 << bit), 26]);
            conditional(out, 34, |out| {
                out.extend([
                    c.get(RNG),
                    signed(6, 65536),
                    signed(6, 1 << bit),
                    26,
                    25,
                    c.set(RNG),
                ])
            });
        }
        out.extend([c.get(RNG), c.set(COLOUR)]);
        for bit in (0..14).rev() {
            let divisor = 5i64 << bit;
            out.extend([c.get(COLOUR), signed(6, divisor)]);
            conditional(out, 34, |out| {
                out.extend([c.get(COLOUR), signed(6, divisor), 25, c.set(COLOUR)])
            });
        }
        out.extend([c.get(COLOUR), signed(6, 1), 23, c.set(COLOUR)]);
    });
    out
}

fn sd(c: &mut Chunk, slot: i64, command: &str, arg: Option<&str>) -> Vec<u32> {
    let mut out = c.method("sd").to_vec();
    out.extend([
        c.literal("MODELCTRL"),
        c.literal("i_rareicon"),
        signed(6, slot),
        c.literal(command),
    ]);
    if let Some(arg) = arg {
        out.push(c.literal(arg));
    }
    out.push(call(0, 0));
    out
}

fn crown(c: &mut Chunk, slot: i64, position: &str, random: bool, seed: u32) -> Result<Vec<u32>> {
    need((0..8).contains(&slot), "unexpected river turret slot")?;
    let xyz = position
        .strip_prefix("V<")
        .and_then(|s| s.split_once('>'))
        .ok_or_else(|| bad("unexpected turret position"))?
        .0;
    let xyz = xyz
        .split(',')
        .map(|s| {
            s.parse::<f32>()
                .map_err(|_| bad("invalid turret coordinate"))
        })
        .collect::<Result<Vec<_>>>()?;
    need(xyz.len() == 3, "invalid turret position width")?;
    let at = format!("V<{},{},{}>/V<0,0,0>", xyz[0], xyz[1] + 5.0, xyz[2]);
    let mut out = colour(c, random, seed);
    out.extend(sd(c, FIRST_CROWN + slot, "GENERATE", Some(&at)));
    out.extend(c.method("sd"));
    out.extend([
        c.literal("MODELCTRL"),
        c.literal("i_rareicon"),
        signed(6, FIRST_CROWN + slot),
        c.literal("SQ_OPEN0"),
        c.get(COLOUR),
        unsigned(29, 2),
        call(0, 0),
    ]);
    Ok(out)
}

fn spawns(c: &mut Chunk, random: bool, seed: u32) -> Result<(Vec<u8>, Vec<i64>)> {
    let mut before = BTreeMap::new();
    let mut slots = Vec::new();
    for pc in 3..c.code.len().saturating_sub(4) {
        if !c.is(c.code[pc], 7, TURRET) || !c.is(c.code[pc + 2], 7, "GENERATE") {
            continue;
        }
        need(
            c.is(c.code[pc - 3], 12, "be")
                && c.is(c.code[pc - 2], 16, "sd")
                && c.is(c.code[pc - 1], 7, "MODELCTRL")
                && c.code[pc + 1] & 63 == 6
                && c.code[pc + 3] & 63 == 7
                && c.code[pc + 4] == call(0, 0),
            "unexpected river turret spawn call",
        )?;
        let slot = (c.code[pc + 1] >> 6) as i64 - BIAS;
        let position = c.text((c.code[pc + 3] >> 6) as usize)?.to_string();
        before.insert(pc + 4, crown(c, slot, &position, random, seed)?);
        slots.push(slot);
    }
    need(!slots.is_empty(), "river turret spawn script missing")?;
    let code = c.rewrite(&BTreeMap::new(), &before, &BTreeMap::new())?;
    Ok((c.write(&code)?, slots))
}

fn hit(c: &mut Chunk) -> Result<Vec<u8>> {
    let mut before = BTreeMap::new();
    let mut count = 0;
    for pc in 2..c.code.len().saturating_sub(1) {
        if !c.is(c.code[pc], 7, "SQ_DDMG") {
            continue;
        }
        need(
            c.is(c.code[pc - 2], 12, "be")
                && c.is(c.code[pc - 1], 16, "MDLsetSeq")
                && c.code[pc + 1] == call(0, 0),
            "unexpected turret death call",
        )?;
        // The method call computes its argument above the sd function,
        // receiver and two strings (stack slots 0..3).
        let mut out = c.method("sd").to_vec();
        out.extend([c.literal("MODELCTRL"), c.literal("i_rareicon")]);
        out.extend(c.method("MDLgetRamNo"));
        out.extend([
            call(4, 1),
            signed(6, FIRST_CROWN),
            23,
            c.literal("SQ_CLOSE"),
            call(0, 0),
        ]);
        before.insert(pc + 1, out);
        count += 1;
    }
    need(count == 2, "unexpected turret death branches")?;
    let code = c.rewrite(&BTreeMap::new(), &before, &BTreeMap::new())?;
    c.write(&code)
}

pub(super) fn expand(b: &[u8], random: bool, seed: u32) -> Result<Vec<Edit>> {
    let p = Pack::parse(b)?;
    let turret = p.named("beModelCtrlInfo", TURRET)?;
    need(
        p.val(p.val(turret, 6)?, 2)? == 8,
        "expected eight river turrets",
    )?;
    let missile = p.named("beModelCtrlInfo", "x_bltnumemisz01")?;
    let seq = p
        .array(p.val(missile, 5)?)?
        .into_iter()
        .find(|id| {
            p.rec(*id)
                .is_ok_and(|r| p.classes[r.class].name == "beModelCtrlInfoDataSeq")
        })
        .ok_or_else(|| bad("missile sequences missing"))?;
    let wait = p
        .array(p.val(seq, 4)?)?
        .into_iter()
        .find(|id| p.rec(*id).is_ok_and(|r| r.named(b, "SQ_WAIT")))
        .ok_or_else(|| bad("missile wait sequence missing"))?;
    let hitboxes = p
        .array(p.val(wait, 3)?)?
        .into_iter()
        .filter(|id| {
            p.rec(*id)
                .is_ok_and(|r| p.classes[r.class].name == "beModelCtrlHITBOX")
        })
        .collect::<Vec<_>>();
    need(
        hitboxes.len() == 1 && p.val(hitboxes[0], 3)? == MISSILE_KIND as u32,
        "unexpected river missile attack ID",
    )?;
    let mut enemy_slots = 0;
    for (id, index) in p.ids.iter().enumerate() {
        if let Some(index) = index {
            let r = &p.objects[*index];
            if p.classes[r.class].name == "beModelCtrlInfo"
                && r.field(7)
                    .is_ok_and(|r| b[r.start + 4..r.end].starts_with(b"KIND_ENEMY\0"))
            {
                enemy_slots += p.val(p.val(id as u32, 6)?, 2)?;
            }
        }
    }
    need(
        enemy_slots + 4 <= FIRST_CROWN as u32,
        "river enemy pools overlap turret crown slots",
    )?;
    let mut builder = Builder::new(&p);
    let mut changed = Vec::new();
    let mut spawned = Vec::new();
    let mut damage_count = 0;
    let mut hit_count = 0;
    let mut memory_template = None;
    let mut seen = std::collections::BTreeSet::new();
    for (id, index) in p.ids.iter().enumerate() {
        let Some(index) = index else { continue };
        let r = &p.objects[*index];
        if p.classes[r.class].name != "beLuaData" {
            continue;
        }
        let object = p.val(id as u32, 3)?;
        if !seen.insert(object) {
            continue;
        }
        p.class(object, "beLuaDataObject")?;
        let mem = p.val(object, 3)?;
        let size = p.val(object, 2)? as usize;
        let range = p
            .memory
            .get(mem as usize)
            .and_then(|r| r.as_ref())
            .ok_or_else(|| bad("missing river Lua memory"))?;
        need(size <= range.len(), "river Lua exceeds memory block")?;
        let raw = &b[range.start..range.start + size];
        let contains = |s: &[u8]| memchr::memmem::find(raw, s).is_some();
        if !(r.named(b, "hit_g_numepodz01.lua")
            || contains(b"g_boathp\0") && contains(b"BLPgetDamageValue\0")
            || contains(b"g_numepodz01\0") && contains(b"GENERATE\0"))
        {
            continue;
        }
        let mut chunk = Chunk::read(raw)?;
        let has = |c: &Chunk, s: &str| {
            c.strings
                .iter()
                .any(|v| v.strip_suffix(&[0]).unwrap_or(v) == s.as_bytes())
        };
        let out = if r.named(b, "hit_p_agumon.lua")
            || has(&chunk, "g_boathp") && has(&chunk, "BLPgetDamageValue")
        {
            damage_count += 1;
            Some(damage(&mut chunk)?)
        } else if r.named(b, "hit_g_numepodz01.lua") {
            hit_count += 1;
            Some(hit(&mut chunk)?)
        } else if has(&chunk, TURRET) && has(&chunk, "GENERATE") {
            let (out, slots) = spawns(&mut chunk, random, seed)?;
            spawned.extend(slots);
            Some(out)
        } else {
            None
        };
        if let Some(out) = out {
            let length = out.len();
            let newmem = builder.mem(out, mem)?;
            builder.modify(object, &[(2, length as u32), (3, newmem)])?;
            changed.push(object);
            memory_template = Some(mem);
        }
    }
    spawned.sort_unstable();
    need(
        damage_count == 1
            && hit_count == 1
            && changed.len() == 4
            && spawned == (0..8).collect::<Vec<_>>(),
        "incomplete river missile patch",
    )?;
    let edits = builder.finish(memory_template.ok_or_else(|| bad("river Lua missing"))?)?;
    let out = materialize(b, &edits)?;
    let v = Pack::parse(&out)?;
    need(
        p.mapping == v.mapping[..p.mapping.len()]
            && b[p.pool..] == out[v.pool..v.pool + b.len() - p.pool],
        "river patch changed original IDs or memory",
    )?;
    for (id, index) in p.ids.iter().enumerate() {
        if let Some(index) = index {
            if !changed.contains(&(id as u32)) {
                need(
                    b[p.objects[*index].range.clone()] == out[v.rec(id as u32)?.range.clone()],
                    "river patch changed unrelated object",
                )?;
            }
        }
    }
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Chunk {
        // Test-owned empty Lua function, not retail bytecode.
        let mut b = b"\x1bLua@\x01\x04\x04\x04\x20\x06\x09\x08".to_vec();
        b.extend([0; 8]);
        b.extend([0; 12]);
        b.push(0);
        b.extend(16u32.to_le_bytes());
        b.extend([0; 20]);
        b.extend(1u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        Chunk::read(&b).unwrap()
    }

    /// Execute inserted scalar instructions independently of the rewriter.
    /// Full retail scripts are also exercised in the reference Lua 4 runtime.
    fn run(c: &Chunk, code: &[u32], g: &mut BTreeMap<String, Option<f64>>) {
        let mut stack = Vec::<Option<f64>>::new();
        let mut pc = 0;
        let mut steps = 0;
        while pc < code.len() {
            steps += 1;
            assert!(steps < 100_000);
            let ins = code[pc];
            let op = ins & 63;
            let u = (ins >> 6) as usize;
            match op {
                0 => break,
                6 => stack.push(Some(u as f64 - BIAS as f64)),
                12 => {
                    if c.text(u).unwrap() == "be" {
                        assert!(c.is(code[pc + 1], 16, "MDLgetBulletKind"));
                        assert_eq!(code[pc + 2], call(0, 1));
                        stack.push(g["attack_kind"]);
                        pc += 2;
                    } else {
                        stack.push(g.get(c.text(u).unwrap()).copied().flatten());
                    }
                }
                19 => {
                    g.insert(c.text(u).unwrap().into(), stack.pop().unwrap());
                }
                23 | 25 | 26 => {
                    let b = stack.pop().unwrap().unwrap();
                    let a = stack.pop().unwrap().unwrap();
                    stack.push(Some(match op {
                        23 => a + b,
                        25 => a - b,
                        _ => a * b,
                    }));
                }
                32..=37 => {
                    let b = stack.pop().unwrap().unwrap();
                    let a = stack.pop().unwrap().unwrap();
                    let take = match op {
                        32 => a != b,
                        33 => a == b,
                        34 => a < b,
                        35 => a <= b,
                        36 => a > b,
                        _ => a >= b,
                    };
                    if take {
                        pc = (pc as i64 + 1 + u as i64 - BIAS) as usize;
                        continue;
                    }
                }
                38 | 39 => {
                    let truthy = stack.pop().unwrap().is_some();
                    if truthy == (op == 38) {
                        pc = (pc as i64 + 1 + u as i64 - BIAS) as usize;
                        continue;
                    }
                }
                42 => {
                    pc = (pc as i64 + 1 + u as i64 - BIAS) as usize;
                    continue;
                }
                _ => panic!("unexpected scalar opcode {op}"),
            }
            pc += 1;
        }
        assert!(stack.is_empty(), "inserted code leaked stack slots");
    }

    #[test]
    fn turret_colours_roll_per_generation_without_consuming_game_rng() {
        let mut c = fixture();
        let random = colour(&mut c, true, 0x72abcdef);
        let yellow = colour(&mut c, false, 0);
        let mut globals = BTreeMap::from([("game_rng".into(), Some(123.0))]);
        let mut state = 0xcdefu32;
        for _ in 0..10_000 {
            run(&c, &yellow, &mut globals);
            assert_eq!(globals[COLOUR], Some(5.0));
            run(&c, &random, &mut globals);
            state = (state * 25173 + 13849) & 65535;
            assert_eq!(globals[RNG], Some(state as f64));
            assert_eq!(globals[COLOUR], Some((state % 5 + 1) as f64));
            assert_eq!(globals["game_rng"], Some(123.0));
        }
    }

    #[test]
    fn branches_skip_after_call_hooks_but_enter_before_instruction_checks() {
        let mut c = fixture();
        c.code = vec![signed(42, 1), signed(6, 123), 0];
        let before = BTreeMap::from([(2, vec![signed(6, 7), c.set("before")])]);
        let after = BTreeMap::from([(1, vec![signed(6, 9), c.set("after")])]);
        let code = c.rewrite(&before, &after, &BTreeMap::new()).unwrap();
        let mut globals = BTreeMap::new();
        run(&c, &code, &mut globals);
        assert_eq!(globals["before"], Some(7.0));
        assert!(!globals.contains_key("after"));
    }

    #[test]
    fn missile_damage_crosses_boat_thresholds_and_clamps_lethal_overshoot() {
        let mut c = fixture();
        let mut code = vec![c.get("g_boathp"), c.get("g_dmgnum"), 25, c.set("g_boathp")];
        for _ in 0..4 {
            code.extend([c.get("g_meta"), c.get("g_dmgnum"), 25, c.set("g_meta")]);
        }
        let mut exits = Vec::new();
        for n in 1..=4 {
            code.extend([
                c.get("g_boathp"),
                c.get(&format!("g_hpnum{n}")),
                signed(32, 3),
                signed(6, n),
                c.set("observed"),
                0,
            ]);
            exits.push(code.len() - 1);
        }
        let end = code.len();
        code.push(0);
        for at in exits {
            code[at] = signed(42, end as i64 - at as i64 - 1);
        }
        c.code = code;
        let modified = damage(&mut c).unwrap();
        let c = Chunk::read(&modified).unwrap();
        for (party, (amount, thresholds)) in [
            (5, [75, 50, 25, 0]),
            (4, [76, 52, 28, 0]),
            (3, [76, 52, 28, 1]),
            (2, [74, 50, 24, 0]),
        ]
        .into_iter()
        .enumerate()
        {
            for before in 1..=100 {
                for attack in [0, MISSILE_KIND, MISSILE_KIND - 1, MISSILE_KIND + 1] {
                    let mut g = BTreeMap::from([
                        ("g_boathp".into(), Some(before as f64)),
                        ("g_dmgnum".into(), Some(amount as f64)),
                        ("g_meta".into(), Some(100.0)),
                        ("attack_kind".into(), Some(attack as f64)),
                    ]);
                    for (n, v) in thresholds.iter().enumerate() {
                        g.insert(format!("g_hpnum{}", n + 1), Some(*v as f64));
                    }
                    run(&c, &c.code, &mut g);
                    let after =
                        (before - amount * if attack == MISSILE_KIND { 3 } else { 1 }).max(0);
                    assert_eq!(g["g_boathp"], Some(after as f64), "party {party}");
                    let expected = if after <= thresholds[3] {
                        Some(4.0)
                    } else {
                        (0..3)
                            .find(|&n| {
                                after <= thresholds[n]
                                    && after > thresholds[n + 1]
                                    && before > thresholds[n]
                            })
                            .map(|n| (n + 1) as f64)
                    };
                    assert_eq!(
                        g.get("observed").copied().flatten(),
                        expected,
                        "party {party}, {before} -> {after}"
                    );
                }
            }
        }
    }

    #[test]
    fn retail_river_scripts_and_preservation() {
        let Some(path) = std::env::var_os("DW4_RIVER_PACK") else {
            return;
        };
        let b = std::fs::read(path).unwrap();
        let expanded = materialize(&b, &super::super::expand(&b).unwrap()).unwrap();
        let mut lengths = Vec::new();
        for random in [false, true] {
            let output =
                materialize(&expanded, &expand(&expanded, random, 0x72abcdef).unwrap()).unwrap();
            lengths.push(output.len());
            let p = Pack::parse(&output).unwrap();
            let mut changed = std::collections::BTreeSet::new();
            for (id, index) in p.ids.iter().enumerate() {
                let Some(index) = index else { continue };
                let r = &p.objects[*index];
                if p.classes[r.class].name != "beLuaData" {
                    continue;
                }
                let object = p.val(id as u32, 3).unwrap();
                if !changed.insert(object) {
                    continue;
                }
                let mem = p.val(object, 3).unwrap();
                let size = p.val(object, 2).unwrap() as usize;
                let range = p.memory[mem as usize].as_ref().unwrap();
                let raw = &output[range.start..range.start + size];
                if memchr::memmem::find(raw, b"g_crownRiver").is_none()
                    && !r.named(&output, "hit_g_numepodz01.lua")
                {
                    continue;
                }
                let c = Chunk::read(raw).unwrap();
                if c.strings.iter().any(|s| s.starts_with(b"g_crownRiver"))
                    || r.named(&output, "hit_g_numepodz01.lua")
                {
                    if let Some(dir) = std::env::var_os("DW4_RIVER_AUDIT_DIR") {
                        let dir = std::path::PathBuf::from(dir).join(if random {
                            "random"
                        } else {
                            "yellow"
                        });
                        std::fs::create_dir_all(&dir).unwrap();
                        let name = r.field(2).unwrap();
                        let name = std::str::from_utf8(&output[name.start + 4..name.end])
                            .unwrap()
                            .trim_end_matches('\0');
                        let name = if c
                            .strings
                            .iter()
                            .any(|s| s.starts_with(b"g_crownRiverDamage\0"))
                        {
                            "player-hit.lua"
                        } else {
                            name
                        };
                        std::fs::write(dir.join(name), raw).unwrap();
                    }
                }
            }
            if let Some(dir) = std::env::var_os("DW4_RIVER_AUDIT_DIR") {
                std::fs::write(
                    std::path::PathBuf::from(dir).join(if random {
                        "random.igb"
                    } else {
                        "yellow.igb"
                    }),
                    &output,
                )
                .unwrap();
            }
        }
        assert_eq!(
            lengths[0], lengths[1],
            "colour option must preserve the GUI's size prediction"
        );
    }
}
