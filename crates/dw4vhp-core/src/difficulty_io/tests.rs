use super::*;

fn cases() -> Vec<(&'static str, u32, &'static str, usize)> {
    vec![
        ("stag0101.igb", 2, "stag0101.igb", 0),
        ("[0.614]", 1, "[0.614]", 0),
        ("ordinary|name", 1, "ordinary|name", 0),
        ("[0.614]|[0.972]|[0.973]", 0, "[0.614]", 1),
        ("[0.614]|[0.972]|[0.973]", 1, "[0.972]", 1),
        ("[0.614]|[0.972]|[0.973]", 2, "[0.973]", 1),
        ("[0.613]|[0.1072]|[0.1073]/s", 1, "[0.1072]", 1),
        ("[0.613]|[0.1072]|[0.1073]/s", 2, "[0.1073]", 1),
        ("[0.614]|[0.972]|[0.973]", u32::MAX, "[0.614]", 1),
        ("[0.614]|[0.972]", 2, "[0.614]|[0.972]", 1),
        (
            "[0.12345678901234567890]|[0.2]|[0.3]",
            0,
            "[0.12345678901234567890]|[0.2]|[0.3]",
            1,
        ),
    ]
}

#[derive(Default)]
struct Memory(BTreeMap<u32, u8>);
impl Memory {
    fn store(&mut self, at: u32, value: u128, size: usize, big: bool) {
        let bytes = if big {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        };
        let bytes = if big {
            &bytes[16 - size..]
        } else {
            &bytes[..size]
        };
        for (index, &byte) in bytes.iter().enumerate() {
            self.0.insert(at + index as u32, byte);
        }
    }
    fn load(&self, at: u32, size: usize, big: bool) -> u128 {
        let mut bytes = [0; 16];
        let dst = if big {
            &mut bytes[16 - size..]
        } else {
            &mut bytes[..size]
        };
        for (index, byte) in dst.iter_mut().enumerate() {
            *byte = *self
                .0
                .get(&(at + index as u32))
                .expect("uninitialised instruction read");
        }
        if big {
            u128::from_be_bytes(bytes)
        } else {
            u128::from_le_bytes(bytes)
        }
    }
    fn name(&mut self, at: u32, name: &str) {
        for (index, byte) in name.bytes().chain([0]).enumerate() {
            self.0.insert(at + index as u32, byte);
        }
    }
    fn text(&self, at: u32) -> String {
        (0..100)
            .map(|i| self.load(at + i, 1, false) as u8)
            .take_while(|b| *b != 0)
            .map(char::from)
            .collect()
    }
}

#[test]
fn mips_router_executes_all_tiers_and_preserves_the_native_call_contract() {
    const BASE: u32 = 0x580000;
    const GET: u32 = 0x400000;
    const NATIVE: u32 = 0x590080;
    const RETURN: u32 = 0x123456;
    for retain in [false, true] {
        for (input, mode, expected, expected_gets) in cases() {
            let mut code = mips_code(GET, 0x660000, 0x401000, 0x402000);
            code[0] = mi(15, 0, 1, ((NATIVE + 0x8000) >> 16) as i32);
            let mut r: [u128; 32] =
                std::array::from_fn(|i| ((0xaabbcc00 + i as u128) << 64) | (0x770000 + i as u128));
            r[0] = 0;
            r[4] = 0x112233445566778899aabbcc00006600;
            r[5] = 0x10000;
            r[25] = NATIVE as i16 as i64 as u64 as u128;
            r[29] = 0x1ff0000;
            r[31] = RETURN as u128;
            let before = r;
            let mut memory = Memory::default();
            memory.name(0x10000, input);
            memory.store(0x660000, 0x661000, 4, false);
            let mut pc = BASE;
            let (mut gets, mut calls, mut releases) = (0, 0, 0);
            let execute = |word: u32, r: &mut [u128; 32], m: &mut Memory| {
                let rs = (word >> 21 & 31) as usize;
                let rt = (word >> 16 & 31) as usize;
                let rd = (word >> 11 & 31) as usize;
                let address = (r[rs] as u64).wrapping_add(word as i16 as i64 as u64) as u32;
                let mut set = |reg: usize, value: u64| {
                    if reg != 0 {
                        r[reg] = (r[reg] & (!0u128 << 64)) | value as u128;
                    }
                };
                match word >> 26 {
                    9 => set(rt, address as i32 as i64 as u64),
                    11 => {
                        let value = u64::from((r[rs] as u64) < u64::from(word as u16));
                        if rt != 0 {
                            r[rt] = (r[rt] & (!0u128 << 64)) | value as u128;
                        }
                    }
                    15 => set(rt, ((word & 65535) << 16) as i32 as i64 as u64),
                    35 => set(rt, m.load(address, 4, false) as i32 as i64 as u64),
                    36 => set(rt, m.load(address, 1, false) as u64),
                    40 => m.store(address, r[rt], 1, false),
                    43 => m.store(address, r[rt], 4, false),
                    55 => set(rt, m.load(address, 8, false) as u64),
                    63 => m.store(address, r[rt], 8, false),
                    31 => m.store(address, r[rt], 16, false),
                    30 => r[rt] = m.load(address, 16, false),
                    0 if word == 0 => (),
                    0 => {
                        let value = match word & 63 {
                            0x21 => (r[rs] as u32).wrapping_add(r[rt] as u32) as i32 as i64 as u64,
                            0x2d => (r[rs] as u64).wrapping_add(r[rt] as u64),
                            _ => panic!("unknown MIPS {word:x}"),
                        };
                        if rd != 0 {
                            r[rd] = (r[rd] & (!0u128 << 64)) | value as u128;
                        }
                    }
                    _ => panic!("unknown MIPS {word:x}"),
                }
            };
            for _ in 0..2000 {
                if pc == RETURN {
                    break;
                }
                if pc == GET {
                    gets += 1;
                    for reg in (1..16).chain([24, 25]) {
                        r[reg] = 0xdead0000 + reg as u128;
                    }
                    r[2] = mode as u128;
                    pc = r[31] as u32;
                    continue;
                }
                if pc == 0x401000 {
                    assert_eq!(r[4] as u32, 0x661000);
                    assert_eq!(memory.text(r[5] as u32), expected);
                    memory.name(0x20000, expected);
                    memory.store(0x1fffc, 1, 4, false);
                    memory.store(0x1fff8, 0x20100, 4, false);
                    for reg in (1..16).chain([24, 25]) {
                        r[reg] = 0xeeee0000 + reg as u128;
                    }
                    r[2] = 0x20000;
                    pc = r[31] as u32;
                    continue;
                }
                if pc == 0x402000 {
                    releases += 1;
                    assert_eq!(r[4] as u32, 0x20100);
                    assert_eq!(memory.load(0x1fffc, 4, false), 0);
                    r[2] = 0xdeadbeef;
                    pc = r[31] as u32;
                    continue;
                }
                if pc == NATIVE {
                    calls += 1;
                    assert_eq!(r[4], before[4]);
                    assert_eq!(memory.text(r[5] as u32), expected, "{input}, tier {mode}");
                    if expected != input {
                        assert_eq!(r[29], before[29] - 64);
                        assert_eq!(r[5] as u32, 0x20000);
                        assert_eq!(memory.load(0x1fffc, 4, false), 1);
                        if retain {
                            memory.store(0x1fffc, 2, 4, false);
                        }
                    }
                    for reg in (1..16).chain([24, 25]) {
                        r[reg] = 0xbeef0000 + reg as u128;
                    }
                    r[2] = 0x87654321;
                    pc = r[31] as u32;
                    continue;
                }
                let index = ((pc - BASE) / 4) as usize;
                let w = code[index];
                let rs = (w >> 21 & 31) as usize;
                let rt = (w >> 16 & 31) as usize;
                match w >> 26 {
                    4 | 5 => {
                        let eq = r[rs] as u64 == r[rt] as u64;
                        let take = if w >> 26 == 4 { eq } else { !eq };
                        execute(code[index + 1], &mut r, &mut memory);
                        pc = if take {
                            pc.wrapping_add(4)
                                .wrapping_add_signed((w as i16 as i32) * 4)
                        } else {
                            pc + 8
                        };
                    }
                    3 => {
                        r[31] = (pc + 8) as u128;
                        execute(code[index + 1], &mut r, &mut memory);
                        pc = (w & 0x3ffffff) << 2;
                    }
                    0 if matches!(w & 63, 8 | 9) => {
                        let target = r[rs] as u32;
                        if w & 63 == 9 {
                            r[31] = (pc + 8) as u128;
                        }
                        execute(code[index + 1], &mut r, &mut memory);
                        pc = target;
                    }
                    _ => {
                        execute(w, &mut r, &mut memory);
                        pc += 4;
                    }
                }
            }
            assert_eq!(pc, RETURN);
            assert_eq!((gets, calls), (expected_gets, 1));
            assert_eq!(releases, usize::from(expected != input && !retain));
            assert_eq!(r[2], 0x87654321);
            if expected != input {
                assert_eq!(memory.load(0x1fffc, 4, false), u128::from(retain));
                assert_eq!(memory.text(0x20000), expected);
            }
            assert_eq!(r[29], before[29]);
            for reg in (16..24).chain([28, 30]) {
                assert_eq!(r[reg], before[reg]);
            }
            assert!(memory
                .0
                .keys()
                .filter(|a| **a >= 0x1fe0000)
                .all(|a| (before[29] as u32 - 64..before[29] as u32).contains(a)));
        }
    }
}

#[test]
fn powerpc_router_executes_all_tiers_and_preserves_the_native_call_contract() {
    const BASE: u32 = 0x800046c0;
    const GET: u32 = 0x802ebb4c;
    const NATIVE: u32 = 0x80004800;
    const RETURN: u32 = 0x81234560;
    for retain in [false, true] {
        for (input, mode, expected, expected_gets) in cases() {
            let code = ppc_code(BASE, GET, 0xffe0, 0x80053000, 0x8007381c);
            let mut r: [u32; 32] = std::array::from_fn(|i| 0x770000 + i as u32);
            r[1] = 0x17ff000;
            r[3] = 0x11223344;
            r[4] = 0x10000;
            r[12] = NATIVE & 65535;
            r[13] = 0x80400000;
            let before = r;
            let mut memory = Memory::default();
            memory.name(0x10000, input);
            memory.store(r[13] - 32, 0x661000, 4, true);
            let (mut pc, mut lr, mut ctr) = (BASE, RETURN, 0u32);
            let (mut gets, mut calls, mut releases) = (0, 0, 0);
            let mut cr = [false; 32];
            for (i, value) in cr.iter_mut().enumerate() {
                *value = i % 3 == 0;
            }
            let original_cr = cr;
            for _ in 0..2000 {
                if pc == RETURN {
                    break;
                }
                if pc == GET {
                    gets += 1;
                    for (reg, value) in r.iter_mut().enumerate().take(13).skip(3) {
                        *value = 0xdead0000 + reg as u32;
                    }
                    r[0] = 0xdead0000;
                    r[3] = mode;
                    pc = lr;
                    continue;
                }
                if pc == 0x80053000 {
                    assert_eq!(r[3], 0x661000);
                    assert_eq!(memory.text(r[4]), expected);
                    memory.name(0x20000, expected);
                    memory.store(0x1fffc, 1, 4, true);
                    memory.store(0x1fff8, 0x20100, 4, true);
                    for (reg, value) in r.iter_mut().enumerate().take(13).skip(3) {
                        *value = 0xeeee0000 + reg as u32;
                    }
                    r[3] = 0x20000;
                    pc = lr;
                    continue;
                }
                if pc == 0x8007381c {
                    releases += 1;
                    assert_eq!(r[3], 0x20100);
                    assert_eq!(memory.load(0x1fffc, 4, true), 0);
                    r[3] = 0xdeadbeef;
                    pc = lr;
                    continue;
                }
                if pc == NATIVE {
                    calls += 1;
                    assert_eq!(r[3], before[3]);
                    assert_eq!(memory.text(r[4]), expected, "{input}, tier {mode}");
                    if expected != input {
                        assert_eq!(r[1], before[1] - 80);
                        assert_eq!(r[4], 0x20000);
                        assert_eq!(memory.load(0x1fffc, 4, true), 1);
                        if retain {
                            memory.store(0x1fffc, 2, 4, true);
                        }
                    }
                    for (reg, value) in r.iter_mut().enumerate().take(13).skip(3) {
                        *value = 0xbeef0000 + reg as u32;
                    }
                    r[0] = 0xbeef0000;
                    r[3] = 0x87654321;
                    pc = lr;
                    continue;
                }
                let word = code[((pc - BASE) / 4) as usize];
                let rt = (word >> 21 & 31) as usize;
                let ra = (word >> 16 & 31) as usize;
                let rb = (word >> 11 & 31) as usize;
                let imm = word as i16 as i32;
                let address = (if ra == 0 { 0 } else { r[ra] }).wrapping_add_signed(imm);
                let mut next = pc + 4;
                match word {
                    0x7c0802a6 => r[0] = lr,
                    0x7c0803a6 => lr = r[0],
                    0x7d8903a6 => ctr = r[12],
                    0x4e800020 => next = lr,
                    0x4e800420 => next = ctr,
                    0x4e800421 => {
                        lr = pc + 4;
                        next = ctr;
                    }
                    _ => match word >> 26 {
                        13 => {
                            r[rt] = address;
                            cr[0] = (address as i32) < 0;
                            cr[1] = (address as i32) > 0;
                            cr[2] = address == 0;
                        }
                        14 => r[rt] = address,
                        15 => {
                            r[rt] =
                                (if ra == 0 { 0 } else { r[ra] }).wrapping_add((imm as u32) << 16)
                        }
                        32 => r[rt] = memory.load(address, 4, true) as u32,
                        34 => r[rt] = memory.load(address, 1, true) as u32,
                        36 => memory.store(address, r[rt] as u128, 4, true),
                        37 => {
                            memory.store(address, r[rt] as u128, 4, true);
                            r[ra] = address;
                        }
                        38 => memory.store(address, r[rt] as u128, 1, true),
                        10 | 11 => {
                            let ordering = if word >> 26 == 10 {
                                r[ra].cmp(&u32::from(word as u16))
                            } else {
                                (r[ra] as i32).cmp(&imm)
                            };
                            cr[0] = ordering.is_lt();
                            cr[1] = ordering.is_gt();
                            cr[2] = ordering.is_eq();
                        }
                        31 => match word >> 1 & 1023 {
                            444 => r[ra] = r[rt] | r[rb],
                            32 => {
                                let order = r[ra].cmp(&r[rb]);
                                cr[0] = order.is_lt();
                                cr[1] = order.is_gt();
                                cr[2] = order.is_eq();
                            }
                            _ => panic!("unknown PowerPC {word:x}"),
                        },
                        16 => {
                            let bo = word >> 21 & 31;
                            let bi = (word >> 16 & 31) as usize;
                            let take = match bo {
                                4 => !cr[bi],
                                12 => cr[bi],
                                _ => panic!("unknown branch {word:x}"),
                            };
                            if take {
                                next = pc.wrapping_add_signed((word & 0xfffc) as i16 as i32);
                            }
                        }
                        18 => {
                            let delta = ((word & 0x03fffffc) << 6) as i32 >> 6;
                            if word & 1 != 0 {
                                lr = pc + 4;
                            }
                            next = pc.wrapping_add_signed(delta);
                        }
                        _ => panic!("unknown PowerPC {word:x}"),
                    },
                }
                pc = next;
            }
            assert_eq!(pc, RETURN);
            assert_eq!((gets, calls), (expected_gets, 1));
            assert_eq!(releases, usize::from(expected != input && !retain));
            assert_eq!(r[3], 0x87654321);
            if expected != input {
                assert_eq!(memory.load(0x1fffc, 4, true), u128::from(retain));
                assert_eq!(memory.text(0x20000), expected);
            }
            assert_eq!(r[1], before[1]);
            assert_eq!(&r[13..], &before[13..]);
            assert_eq!(&cr[8..20], &original_cr[8..20]);
            assert!(memory
                .0
                .keys()
                .filter(|a| (0x17f0000..0x1800000).contains(*a))
                .all(|a| (before[1] - 80..before[1]).contains(a)));
        }
    }
}

#[test]
fn regional_routing_hooks_validate_and_fit_the_original_executables() {
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
        let iso = std::fs::read(path).unwrap();
        let release = crate::release::detect(&iso).unwrap();
        let edits = if release.is_gamecube() {
            gamecube_regions(&iso, release).unwrap()
        } else {
            crate::ps2_memory::regions_for_release(
                &iso,
                release,
                true,
                crate::ps2_memory::relocated_module(&iso, release)
                    .unwrap()
                    .as_ref(),
            )
            .unwrap()
        };
        assert!(edits.len() >= 3);
        for edit in &edits {
            assert!(edit.offset as usize + edit.bytes.len() <= iso.len());
            assert_ne!(
                &iso[edit.offset as usize..edit.offset as usize + edit.bytes.len()],
                edit.bytes
            );
        }
        if let Some(dir) = std::env::var_os("DW4_ROUTING_EXPORT") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            let data = edits
                .iter()
                .map(|r| serde_json::json!({"offset":r.offset,"bytes":r.bytes}))
                .collect::<Vec<_>>();
            std::fs::write(
                dir.join(format!("{variable}.json")),
                serde_json::to_vec_pretty(&data).unwrap(),
            )
            .unwrap();
        }
        println!(
            "{variable}: {} guarded executable edits, {} emitted bytes",
            edits.len(),
            edits.iter().map(|edit| edit.bytes.len()).sum::<usize>()
        );
    }
}
