//! Per-spawn cosmetic colours, using a private PRNG at the icon-argument call.
//! Enemy rows, combat values, drop profiles and the game's RNG are not remapped.
use crate::{
    disc::DiscReport,
    elf::ElfInfo,
    error::{Error, Result},
    region::WriteRegion,
    release::Release,
    rowgroup::ROW_GROUPS,
    table::EnemyTable,
};
use md5::{Digest, Md5};

pub const RANDOM_MARKER: u8 = 127;
const MIPS_CODE: u32 = 0x0011_3b84;
const MIPS_STATE: u32 = 0x0010_02c4;
const PPC_CODE: u32 = 0x8000_4a00;
const PPC_STATE: u32 = PPC_CODE + 64;

/// Boss families remain yellow; regular enemies and the practice variants roll
/// independently each time their icon is generated.
pub fn is_boss_model(model: &str) -> bool {
    model.starts_with("f_")
        || matches!(
            model,
            "e_blosso"
                | "e_mammos"
                | "e_mecha4"
                | "e_mecha4bs"
                | "e_skulgrey"
                | "e_anomalo"
                | "e_tonogeko"
                | "e_mecha3"
                | "e_diabolo"
                | "e_mecha5"
                | "e_ldknight"
        )
}
pub(crate) fn mark_rows(table: &mut EnemyTable) {
    for group in ROW_GROUPS {
        if group.model.starts_with("g_") {
            continue;
        }
        let colour = if is_boss_model(group.model) {
            5
        } else {
            RANDOM_MARKER
        };
        for &row in group.rows {
            if table.rarity[row] != 0 {
                table.rarity[row] = colour;
            }
        }
    }
}
fn bad(message: &str) -> Error {
    Error::CrownData(message.into())
}
fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(bad(message))
    }
}
fn word(bytes: &[u8], offset: usize, big: bool) -> Result<u32> {
    let data: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| bad("truncated colour hook metadata"))?
        .try_into()
        .unwrap();
    Ok(if big {
        u32::from_be_bytes(data)
    } else {
        u32::from_le_bytes(data)
    })
}
fn i(op: u32, rs: u32, rt: u32, imm: i16) -> u32 {
    (op << 26) | (rs << 21) | (rt << 16) | u32::from(imm as u16)
}
fn r(rs: u32, rt: u32, rd: u32, sa: u32, fun: u32) -> u32 {
    (rs << 21) | (rt << 16) | (rd << 11) | (sa << 6) | fun
}
fn mips_jump(target: u32, link: bool) -> u32 {
    ((if link { 3 } else { 2 }) << 26) | ((target >> 2) & 0x03ff_ffff)
}
fn ppc_branch(from: u32, to: u32, link: bool) -> u32 {
    0x4800_0000 | (to.wrapping_sub(from) & 0x03ff_fffc) | u32::from(link)
}
fn hi(address: u32) -> i16 {
    ((address.wrapping_add(0x8000)) >> 16) as i16
}
fn lo(address: u32) -> i16 {
    address as i16
}
fn encoded(words: &[u32], big: bool) -> Vec<u8> {
    words
        .iter()
        .flat_map(|w| {
            if big {
                w.to_be_bytes()
            } else {
                w.to_le_bytes()
            }
        })
        .collect()
}

// Tail calls retain the return address of the original icon argument push.
// The EE's three-operand MULTU writes its low word to the destination register.
fn mips_stub(callee: u32) -> Vec<u8> {
    encoded(
        &[
            i(9, 5, 2, -(RANDOM_MARKER as i16)), // v0 = a1 - marker
            i(5, 2, 0, 11),                      // ordinary colour -> tail call
            i(15, 0, 2, hi(MIPS_STATE)),         // delay slot: state base
            i(35, 2, 5, lo(MIPS_STATE)),
            i(9, 0, 3, 25173),
            r(5, 3, 5, 0, 0x19), // EE MULTU a1,a1,v1
            i(9, 5, 5, 13849),
            i(43, 2, 5, lo(MIPS_STATE)),
            r(0, 5, 5, 16, 2), // high 16 bits
            i(9, 0, 3, 5),
            r(5, 3, 0, 0, 0x1b), // DIVU
            r(0, 0, 5, 0, 0x10), // MFHI a1
            i(9, 5, 5, 1),
            mips_jump(callee, false),
            0,
        ],
        false,
    )
}
fn ppc_stub(callee: u32) -> Vec<u8> {
    encoded(
        &[
            0x2c00_0000 | (4 << 16) | u32::from(RANDOM_MARKER), // cmpwi r4,marker
            0x4082_0000 | (12 * 4),                             // bne tail
            i(15, 5, 0, hi(PPC_STATE)),                         // lis r5,state@ha
            i(32, 4, 5, lo(PPC_STATE)),                         // lwz r4,state@l(r5)
            i(7, 4, 4, 25173),                                  // mulli r4,r4,multiplier
            i(14, 4, 4, 13849),
            i(36, 4, 5, lo(PPC_STATE)),
            (21 << 26) | (4 << 21) | (4 << 16) | (16 << 11) | (16 << 6) | (31 << 1), // srwi r4,r4,16
            i(14, 5, 0, 5),                                                          // li r5,5
            (31 << 26) | (6 << 21) | (4 << 16) | (5 << 11) | (459 << 1), // divwu r6,r4,r5
            (31 << 26) | (6 << 21) | (6 << 16) | (5 << 11) | (235 << 1), // mullw r6,r6,r5
            (31 << 26) | (4 << 21) | (6 << 16) | (4 << 11) | (40 << 1),  // subf r4,r6,r4
            i(14, 4, 4, 1),
            ppc_branch(PPC_CODE + 13 * 4, callee, false),
        ],
        true,
    )
}

pub(crate) fn regions(iso: &[u8], report: &DiscReport, seed: u32) -> Result<Vec<WriteRegion>> {
    let release = report
        .release
        .ok_or_else(|| bad("random crowns require a supported retail executable"))?;
    let (site, callee, fingerprint) = match release {
        Release::Usa => (0x003d_1bd4, 0x0038_f250, "3a9397bb5d295a20648cc6eb3b78839d"),
        Release::Europe => (0x003d_1e64, 0x0038_f4e0, "5066b96ae62cbd03f26e1874bfc844e4"),
        Release::Japan => (0x003d_32a4, 0x0039_0920, "e0fe7ee0de9c75ba62e7bd1ef0baff36"),
        Release::GcUsa => (0x8034_bd60, 0x8030_6a40, "1d3b48339fed09c3b429d7eea8309be8"),
        Release::GcJapan => (0x8034_bab4, 0x8030_6aec, "7e8cc7bce2fb2b31e4b970a5922be0f8"),
    };
    let gc = release.is_gamecube();
    let elf = if gc {
        None
    } else {
        Some(ElfInfo::read(
            iso,
            report.boot_elf.as_ref().ok_or(Error::ElfNotFound)?,
        )?)
    };
    let map = |address: u32| -> Result<usize> {
        if let Some(elf) = &elf {
            return elf
                .vaddr_to_iso_offset(address)
                .map(|o| o as usize)
                .ok_or_else(|| bad("unmapped crown hook"));
        }
        let dol = word(iso, 0x420, true)? as usize;
        for index in 0..7 {
            let off = word(iso, dol + index * 4, true)? as usize;
            let base = word(iso, dol + 0x48 + index * 4, true)?;
            let length = word(iso, dol + 0x90 + index * 4, true)?;
            if address >= base && address - base < length {
                return Ok(dol + off + (address - base) as usize);
            }
        }
        Err(bad("unmapped GameCube crown hook"))
    };
    let offset = map(site)?;
    let context_start = offset
        .checked_sub(16)
        .ok_or_else(|| bad("truncated crown call"))?;
    let context = iso
        .get(context_start..offset + 28)
        .ok_or_else(|| bad("truncated crown call"))?;
    need(
        format!("{:x}", Md5::digest(context)) == fingerprint,
        "unrecognised crown argument call",
    )?;
    let code = if gc { PPC_CODE } else { MIPS_CODE };
    let state = if gc { PPC_STATE } else { MIPS_STATE };
    let original = if gc {
        ppc_branch(site, callee, true)
    } else {
        mips_jump(callee, true)
    };
    need(
        word(iso, offset, gc)? == original,
        "crown push call does not match",
    )?;
    let stub = if gc {
        ppc_stub(callee)
    } else {
        mips_stub(callee)
    };
    let code_at = map(code)?;
    let state_at = map(state)?;
    let available = if gc { 128 } else { 60 };
    need(
        iso.get(code_at..code_at + available)
            .is_some_and(|b| b.iter().all(|v| *v == 0)),
        "crown hook padding is not empty",
    )?;
    need(
        iso.get(state_at..state_at + 4) == Some(&[0; 4]),
        "crown RNG state padding is not empty",
    )?;
    need(
        stub.len() <= available,
        "crown stub exceeds reserved padding",
    )?;
    let call = if gc {
        ppc_branch(site, code, true)
    } else {
        mips_jump(code, true)
    };
    Ok(vec![
        WriteRegion {
            offset: offset as u64,
            bytes: encoded(&[call], gc),
        },
        WriteRegion {
            offset: code_at as u64,
            bytes: stub,
        },
        WriteRegion {
            offset: state_at as u64,
            bytes: encoded(&[seed], gc),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn marking_only_changes_cosmetic_bytes_and_keeps_bosses_yellow() {
        let mut table = EnemyTable {
            hp: vec![10; 649],
            stat: vec![[12; 12]; 649],
            crit: vec![1; 649],
            para: vec![2; 649],
            rarity: vec![5; 649],
        };
        for row in 370..=378 {
            table.rarity[row] = 0;
        }
        let before = table.clone();
        mark_rows(&mut table);
        assert_eq!(table.hp, before.hp);
        assert_eq!(table.stat, before.stat);
        assert_eq!(table.crit, before.crit);
        assert_eq!(table.para, before.para);
        for group in ROW_GROUPS {
            for &row in group.rows {
                assert_eq!(
                    table.rarity[row],
                    if group.model.starts_with("g_") {
                        0
                    } else if is_boss_model(group.model) {
                        5
                    } else {
                        RANDOM_MARKER
                    }
                );
            }
        }
        assert!(is_boss_model("e_tonogeko") && is_boss_model("e_anomalo"));
        assert!(!is_boss_model("e_goburi"));
    }
    // Small instruction models execute the emitted code, rather than testing
    // only a Rust copy of the PRNG. Loads/stores outside its private state fail.
    fn run_mips(input: u32, state: &mut u32) -> u32 {
        let words: Vec<_> = mips_stub(0x38f250)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let mut regs = [0u32; 32];
        regs[4] = 0x171000;
        regs[5] = input;
        regs[29] = 0x1fff000;
        regs[31] = 0x3d1bdc;
        let mut pc = 0usize;
        let mut remainder = 0;
        for _ in 0..32 {
            let w = words[pc];
            let op = w >> 26;
            let rs = ((w >> 21) & 31) as usize;
            let rt = ((w >> 16) & 31) as usize;
            let rd = ((w >> 11) & 31) as usize;
            let immediate = (w as i16) as i32 as u32;
            if op == 2 {
                assert_eq!((w & 0x03ffffff) << 2, 0x38f250);
                assert_eq!(regs[4], 0x171000);
                assert_eq!(regs[29], 0x1fff000);
                assert_eq!(regs[31], 0x3d1bdc);
                return regs[5];
            }
            if op == 5 {
                // This hook's branch delay always loads the private-state base.
                assert_eq!(words[pc + 1] >> 26, 15);
                let taken = regs[rs] != regs[rt];
                regs[2] = MIPS_STATE & 0xffff0000;
                pc = if taken {
                    (pc as isize + 1 + w as i16 as isize) as usize
                } else {
                    pc + 2
                };
                continue;
            }
            match op {
                9 => regs[rt] = regs[rs].wrapping_add(immediate),
                15 => regs[rt] = (w & 65535) << 16,
                35 => {
                    assert_eq!(regs[rs].wrapping_add(immediate), MIPS_STATE);
                    regs[rt] = *state;
                }
                43 => {
                    assert_eq!(regs[rs].wrapping_add(immediate), MIPS_STATE);
                    *state = regs[rt];
                }
                0 => match w & 63 {
                    0x19 => regs[rd] = regs[rs].wrapping_mul(regs[rt]),
                    2 => regs[rd] = regs[rt] >> ((w >> 6) & 31),
                    0x1b => remainder = regs[rs] % regs[rt],
                    0x10 => regs[rd] = remainder,
                    _ => panic!("unexpected MIPS instruction {w:x}"),
                },
                _ => panic!("unexpected MIPS opcode {op}"),
            }
            pc += 1;
        }
        panic!("MIPS hook did not tail-call the original icon routine");
    }
    fn run_ppc(input: u32, state: &mut u32) -> u32 {
        let words: Vec<_> = ppc_stub(0x80306a40)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_be_bytes(*b))
            .collect();
        let mut regs = [0u32; 32];
        regs[3] = 0x81710000;
        regs[4] = input;
        regs[1] = 0x817fff00;
        let mut equal = false;
        let mut pc = 0usize;
        for _ in 0..32 {
            let w = words[pc];
            let op = w >> 26;
            let rd = ((w >> 21) & 31) as usize;
            let ra = ((w >> 16) & 31) as usize;
            let rb = ((w >> 11) & 31) as usize;
            let immediate = (w as i16) as i32 as u32;
            match op {
                11 => equal = regs[ra] == immediate,
                16 => {
                    assert_eq!(w & 0xffff0000, 0x40820000);
                    pc = if !equal {
                        (pc as isize + (w as i16 as isize) / 4) as usize
                    } else {
                        pc + 1
                    };
                    continue;
                }
                15 => regs[rd] = (w & 65535) << 16,
                32 => {
                    assert_eq!(regs[ra].wrapping_add(immediate), PPC_STATE);
                    regs[rd] = *state;
                }
                36 => {
                    assert_eq!(regs[ra].wrapping_add(immediate), PPC_STATE);
                    *state = regs[rd];
                }
                7 => regs[rd] = regs[ra].wrapping_mul(immediate),
                14 => regs[rd] = (if ra == 0 { 0 } else { regs[ra] }).wrapping_add(immediate),
                21 => {
                    assert_eq!((w >> 11) & 31, 16);
                    assert_eq!((w >> 6) & 31, 16);
                    assert_eq!((w >> 1) & 31, 31);
                    regs[ra] = regs[rd] >> 16;
                }
                31 => match (w >> 1) & 1023 {
                    459 => regs[rd] = regs[ra] / regs[rb],
                    235 => regs[rd] = regs[ra].wrapping_mul(regs[rb]),
                    40 => regs[rd] = regs[rb].wrapping_sub(regs[ra]),
                    _ => panic!("unexpected PPC instruction {w:x}"),
                },
                18 => {
                    let offset = ((w & 0x03fffffc) << 6) as i32 >> 6;
                    assert_eq!(
                        (PPC_CODE + pc as u32 * 4).wrapping_add_signed(offset),
                        0x80306a40
                    );
                    assert_eq!(w & 1, 0);
                    assert_eq!(regs[3], 0x81710000);
                    assert_eq!(regs[1], 0x817fff00);
                    return regs[4];
                }
                _ => panic!("unexpected PPC opcode {op}"),
            }
            pc += 1;
        }
        panic!("PPC hook did not tail-call the original icon routine");
    }
    #[test]
    fn both_cpu_hooks_roll_per_spawn_and_leave_ordinary_colours_and_bosses_unchanged() {
        for input in 0..=5 {
            let mut state = 0x72abcdef;
            assert_eq!(run_mips(input, &mut state), input);
            assert_eq!(state, 0x72abcdef);
            assert_eq!(run_ppc(input, &mut state), input);
            assert_eq!(state, 0x72abcdef);
        }
        let (mut mips, mut ppc, mut expected) = (0x72abcdefu32, 0x72abcdefu32, 0x72abcdefu32);
        let mut counts = [0usize; 5];
        for _ in 0..10000 {
            expected = expected.wrapping_mul(25173).wrapping_add(13849);
            let colour = ((expected >> 16) % 5) + 1;
            assert_eq!(run_mips(u32::from(RANDOM_MARKER), &mut mips), colour);
            assert_eq!(run_ppc(u32::from(RANDOM_MARKER), &mut ppc), colour);
            assert_eq!(mips, expected);
            assert_eq!(ppc, expected);
            counts[colour as usize - 1] += 1;
        }
        assert!(counts.iter().all(|n| *n > 1700 && *n < 2300), "{counts:?}");
    }

    #[test]
    fn generated_stubs_fit_the_reserved_code_ranges() {
        assert_eq!(mips_stub(0x38f250).len(), 60);
        assert_eq!(ppc_stub(0x80306a40).len(), 56);
    }

    #[test]
    fn existing_uncrowned_enemies_and_props_stay_uncrowned() {
        let mut table = EnemyTable {
            hp: vec![10; 649],
            stat: vec![[12; 12]; 649],
            crit: vec![1; 649],
            para: vec![2; 649],
            rarity: vec![0; 649],
        };
        mark_rows(&mut table);
        assert!(table.rarity.iter().all(|rank| *rank == 0));
    }
}
