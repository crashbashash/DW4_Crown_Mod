//! Balance deserialized scene objects across the engine's existing arenas.
//!
//! Imported models can fill the stage arena while the general arena still has
//! room. Objects carry their allocator identity, so the normal destructor and
//! list-buffer helpers continue to free through the allocator that owns them.
//! Leave serialized data and explicitly selected pools alone. The soft limit
//! reserves general-arena space for buffers that grow after object creation,
//! Lua and texture conversion; it does not change the randomized resource plan.
use crate::{
    disc::DiscReport,
    error::{Error, Result},
    region::WriteRegion,
    release::Release,
};
use md5::{Digest, Md5};

const CODE: u32 = 0x8000_4b00;
const OWNER_CODE: u32 = 0x8000_4c00;
const ALLOCATION_CODE: u32 = 0x8000_4d00;
const ORIGINAL_CODE: u32 = 0x8000_4e00;
const GENERAL_OBJECT_LIMIT: u32 = 4 * 1024 * 1024;

fn bad() -> Error {
    Error::CrownData("unrecognised GameCube scene allocator hook".into())
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(bad)?
            .try_into()
            .unwrap(),
    ))
}

fn map(iso: &[u8], address: u32, length: usize) -> Result<usize> {
    let dol = word(iso, 0x420)? as usize;
    for index in 0..7 {
        let offset = word(iso, dol + index * 4)? as usize;
        let base = word(iso, dol + 0x48 + index * 4)?;
        let size = word(iso, dol + 0x90 + index * 4)?;
        if address >= base && u64::from(address - base) + length as u64 <= u64::from(size) {
            let at = dol
                .checked_add(offset)
                .and_then(|x| x.checked_add((address - base) as usize))
                .ok_or_else(bad)?;
            iso.get(at..at + length).ok_or_else(bad)?;
            return Ok(at);
        }
    }
    Err(bad())
}

fn encoded(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_be_bytes()).collect()
}

fn branch(from: u32, to: u32) -> u32 {
    0x4800_0000 | (to.wrapping_sub(from) & 0x03ff_fffc)
}

fn owner_stub(_operation: u32, owns: u32, pools_sda: u16) -> Vec<u8> {
    let mut w = Vec::new();
    let mut labels = std::collections::BTreeMap::new();
    let mut fixups = Vec::new();
    macro_rules! emit {
        ($x:expr) => {
            w.push($x)
        };
    }
    macro_rules! mark {
        ($s:literal) => {
            labels.insert($s, w.len())
        };
    }
    macro_rules! jump {
        ($op:expr, $s:literal) => {{
            fixups.push((w.len(), $op, $s));
            emit!(0);
        }};
    }
    macro_rules! call {
        ($target:expr) => {
            emit!(branch(OWNER_CODE + w.len() as u32 * 4, $target) | 1)
        };
    }
    emit!(0x28040000);
    jump!(0x41820000, "original");
    emit!(0x800300b4);
    emit!(0x2c00000a);
    jump!(0x41820000, "setup");
    emit!(0x2c00000c);
    jump!(0x40820000, "original");
    mark!("setup");
    emit!(0x9421ffb0);
    emit!(0x7c0802a6);
    emit!(0x9001004c);
    for (reg, offset) in [(3, 40), (4, 44), (5, 48), (6, 52)] {
        emit!(0x90010000 | reg << 21 | offset);
    }
    call!(owns);
    emit!(0x28030000);
    jump!(0x40820000, "restore");
    emit!(0x816d0000 | u32::from(pools_sda));
    emit!(0x280b0000);
    jump!(0x41820000, "restore");
    emit!(0x800b0004);
    emit!(0x28000007);
    jump!(0x41800000, "restore");
    emit!(0x816b0000);
    emit!(0x81810028);
    emit!(0x800c00b4);
    emit!(0x2c00000a);
    jump!(0x40820000, "general");
    emit!(0x806b0018);
    jump!(0x48000000, "candidate");
    mark!("general");
    emit!(0x806b0014);
    mark!("candidate");
    emit!(0x28030000);
    jump!(0x41820000, "restore");
    emit!(0x80030000);
    emit!(0x816c0000);
    emit!(0x7c005800);
    jump!(0x40820000, "restore");
    emit!(0x90610038);
    emit!(0x8081002c);
    call!(owns);
    emit!(0x28030000);
    jump!(0x41820000, "restore");
    emit!(0x80010038);
    emit!(0x90010028);
    mark!("restore");
    for (reg, offset) in [(3, 40), (4, 44), (5, 48), (6, 52)] {
        emit!(0x80010000 | reg << 21 | offset);
    }
    emit!(0x8001004c);
    emit!(0x7c0803a6);
    emit!(0x38210050);
    // Ownership is resolved before the fresh-allocation fallback. Tail call it
    // with the original caller's stack and return address intact.
    mark!("original");
    emit!(branch(OWNER_CODE + w.len() as u32 * 4, ALLOCATION_CODE));
    for (index, op, label) in fixups {
        let offset = (labels[label] as isize - index as isize) * 4;
        w[index] = op
            | if op == 0x48000000 {
                offset as u32 & 0x03fffffc
            } else {
                offset as u32 & 0xfffc
            };
    }
    encoded(&w)
}

/// A fragmented arena can have ample total free space yet fail a large fresh
/// array allocation. Retry that allocation in the other existing arena. Never
/// retry a resize/free with a foreign pointer; owner_stub handles those first.
fn allocation_stub(operation: u32, pools_sda: u16) -> Vec<u8> {
    let mut w = Vec::new();
    let mut labels = std::collections::BTreeMap::new();
    let mut fixups = Vec::new();
    macro_rules! emit {
        ($op:expr) => {
            w.push($op)
        };
    }
    macro_rules! mark {
        ($name:literal) => {
            labels.insert($name, w.len());
        };
    }
    macro_rules! jump {
        ($op:expr, $name:literal) => {{
            fixups.push((w.len(), $op, $name));
            emit!($op);
        }};
    }
    macro_rules! call_original {
        () => {
            emit!(branch(ALLOCATION_CODE + w.len() as u32 * 4, ORIGINAL_CODE) | 1);
        };
    }
    emit!(0x28040000); // old buffer
    jump!(0x40820000, "original");
    emit!(0x28050000); // zero-size operations retain native behavior
    jump!(0x41820000, "original");
    emit!(0x2c05ffff); // -1 is the native free operation
    jump!(0x41820000, "original");
    emit!(0x800300b4);
    emit!(0x2c00000a);
    jump!(0x41820000, "setup");
    emit!(0x2c00000c);
    jump!(0x40820000, "original");
    mark!("setup");
    emit!(0x9421ffb0);
    emit!(0x7c0802a6);
    emit!(0x9001004c);
    for (reg, offset) in [(3, 40), (4, 44), (5, 48), (6, 52)] {
        emit!(0x90010000 | reg << 21 | offset);
    }
    call_original!();
    emit!(0x28030000);
    jump!(0x40820000, "return");
    emit!(0x816d0000 | u32::from(pools_sda));
    emit!(0x280b0000);
    jump!(0x41820000, "failure");
    emit!(0x800b0004);
    emit!(0x28000007);
    jump!(0x41800000, "failure");
    emit!(0x816b0000);
    emit!(0x81810028);
    emit!(0x800c00b4);
    emit!(0x2c00000a);
    jump!(0x40820000, "general");
    emit!(0x806b0018);
    jump!(0x48000000, "candidate");
    mark!("general");
    emit!(0x806b0014);
    mark!("candidate");
    emit!(0x28030000);
    jump!(0x41820000, "failure");
    emit!(0x80030000);
    emit!(0x816c0000);
    emit!(0x7c005800); // matching allocator class
    jump!(0x40820000, "failure");
    for (reg, offset) in [(4, 44), (5, 48), (6, 52)] {
        emit!(0x80010000 | reg << 21 | offset);
    }
    call_original!();
    jump!(0x48000000, "return");
    mark!("failure");
    emit!(0x38600000);
    mark!("return");
    emit!(0x8001004c);
    emit!(0x7c0803a6);
    emit!(0x38210050);
    emit!(0x4e800020);
    mark!("original");
    emit!(0x9421ffb0);
    emit!(branch(ALLOCATION_CODE + w.len() as u32 * 4, operation + 4));
    for (index, op, label) in fixups {
        let offset = (labels[label] as isize - index as isize) * 4;
        w[index] = op
            | if op == 0x48000000 {
                offset as u32 & 0x03fffffc
            } else {
                offset as u32 & 0xfffc
            };
    }
    encoded(&w)
}

fn stub(site: u32, pools_sda: u16) -> Vec<u8> {
    // The intercepted instruction loads the meta object into r3 immediately
    // before its constructor call. r0/r3/r12 and CR0 are volatile here. Do not
    // touch LR, the stack, object reference counts or the file's pool fields.
    let mut words = vec![
        0x818d_0000 | u32::from(pools_sda), // lwz r12,evenPoolTable(r13)
        0x280c_0000,                        // table must exist
        0x4182_0000,                        // beq original
        0x800c_0004,                        // table count
        0x2800_0007,
        0x4180_0000, // blt original
        0x818c_0000, // table data
        0x806c_0018, // main arena, pool ID 12
        0x7c04_1840, // cmplw r4,r3
        0x4082_0000, // custom pool -> original
        0x818c_0014, // general arena, pool ID 10
        0x280c_0000,
        0x4182_0000,
        0x800c_0038, // used bytes, high half
        0x2800_0000,
        0x4082_0000,
        0x800c_003c, // used bytes, low half
        0x3c60_0000 | (GENERAL_OBJECT_LIMIT >> 16),
        0x7c00_1840, // cmplw r0,r3
        0x4080_0000, // general reserve reached
        0x7d84_6378, // mr r4,r12
        0x807e_001c, // original: lwz r3,28(r30)
        branch(CODE + 22 * 4, site + 4),
    ];
    for index in [2, 5, 9, 12, 15, 19] {
        words[index] |= (21 - index as u32) * 4;
    }
    encoded(&words)
}

pub(crate) fn regions(iso: &[u8], report: &DiscReport) -> Result<Vec<WriteRegion>> {
    let (site, getter, operation, owns) = match report.release {
        Some(Release::GcUsa) => (0x8006_8ed8, 0x8005_929c, 0x8008_5004, 0x8008_4928),
        Some(Release::GcJapan) => (0x8006_8e78, 0x8005_923c, 0x8008_4fa4, 0x8008_48c8),
        _ => return Ok(Vec::new()),
    };
    let context = encoded(&[
        0x809f_0120,
        0x4800_0014,
        0x4bff_792d,
        0x7c64_1b79,
        0x4082_0008,
        0x809f_0120,
        0x807e_001c,
        0x4bff_c671,
        0x7c7d_1b79,
        0x4082_0020,
        0x3800_0001,
    ]);
    let at = map(iso, site - 24, context.len())?;
    if iso[at..at + context.len()] != context {
        return Err(bad());
    }
    // Read the regional SDA displacement from the native pool-ID lookup.
    let getter_at = map(iso, getter, 56)?;
    let expected = [
        0x5460_07ff,
        0x4182_001c,
        0x808d_0000,
        0x7c60_0e70,
        0x5400_103a,
        0x8064_0000,
        0x7c63_002e,
        0x4e80_0020,
        0x808d_0000,
        0x7c60_0e70,
        0x5400_103a,
        0x8064_0000,
        0x7c63_002e,
        0x4e80_0020,
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        let mask = if index == 2 || index == 8 {
            0xffff_0000
        } else {
            u32::MAX
        };
        if word(iso, getter_at + index * 4)? & mask != expected {
            return Err(bad());
        }
    }
    let code = stub(site, word(iso, getter_at + 8 * 4)? as u16);
    let code_at = map(iso, CODE, 256)?;
    if iso[code_at..code_at + 256].iter().any(|byte| *byte != 0) {
        return Err(bad());
    }
    for (address, fingerprint) in [
        (operation, "71a57239b7468038dfc4cf34e69278bd"),
        (owns, "c52539cabfc21bc5d5966949c49f4f46"),
    ] {
        let offset = map(iso, address, 32)?;
        if format!("{:x}", Md5::digest(&iso[offset..offset + 32])) != fingerprint {
            return Err(bad());
        }
    }
    let owner = owner_stub(operation, owns, word(iso, getter_at + 8 * 4)? as u16);
    let owner_at = map(iso, OWNER_CODE, 256)?;
    if owner.len() > 256 || iso[owner_at..owner_at + 256].iter().any(|byte| *byte != 0) {
        return Err(bad());
    }
    let allocation = allocation_stub(operation, word(iso, getter_at + 8 * 4)? as u16);
    let allocation_at = map(iso, ALLOCATION_CODE, 256)?;
    let original_at = map(iso, ORIGINAL_CODE, 8)?;
    if allocation.len() > 256
        || iso[allocation_at..allocation_at + 256]
            .iter()
            .any(|b| *b != 0)
        || iso[original_at..original_at + 8].iter().any(|b| *b != 0)
    {
        return Err(bad());
    }
    Ok(vec![
        WriteRegion {
            offset: (at + 24) as u64,
            bytes: encoded(&[branch(site, CODE)]),
        },
        WriteRegion {
            offset: code_at as u64,
            bytes: code,
        },
        WriteRegion {
            offset: map(iso, operation, 4)? as u64,
            bytes: encoded(&[branch(operation, OWNER_CODE)]),
        },
        WriteRegion {
            offset: owner_at as u64,
            bytes: owner,
        },
        WriteRegion {
            offset: allocation_at as u64,
            bytes: allocation,
        },
        WriteRegion {
            offset: original_at as u64,
            bytes: encoded(&[0x9421ffb0, branch(ORIGINAL_CODE + 4, operation + 4)]),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cmp::Ordering, collections::BTreeMap};

    // Execute the emitted instructions against a deliberately small memory
    // map. Unexpected reads, writes, calls and register clobbers fail the test.
    fn execute(pool: u32, used: u64, count: u32, present: bool) -> u32 {
        let main = 0x8058_04a8;
        let general = 0x8058_03d0;
        let words: Vec<_> = stub(0x8006_8ed8, 0xd258)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_be_bytes(*b))
            .collect();
        let mut regs = std::array::from_fn::<_, 32, _>(|i| 0x8100_0000 + i as u32 * 0x100);
        regs[4] = pool;
        regs[13] = 0x8056_4f00;
        let before = regs;
        let memory = BTreeMap::from([
            (
                regs[13].wrapping_add_signed(-11688),
                if present { 0x8100_4000 } else { 0 },
            ),
            (0x8100_4004, count),
            (0x8100_4000, 0x8100_5000),
            (0x8100_5018, main),
            (0x8100_5014, general),
            (general + 56, (used >> 32) as u32),
            (general + 60, used as u32),
            (regs[30] + 28, 0x806f_1234),
        ]);
        let mut pc = 0usize;
        let mut compare = Ordering::Equal;
        for _ in 0..32 {
            let w = words[pc];
            let rd = ((w >> 21) & 31) as usize;
            let ra = ((w >> 16) & 31) as usize;
            let rb = ((w >> 11) & 31) as usize;
            match w >> 26 {
                32 => regs[rd] = memory[&regs[ra].wrapping_add_signed(w as i16 as i32)],
                10 => compare = regs[ra].cmp(&(w & 65535)),
                15 => {
                    assert_eq!(ra, 0);
                    regs[rd] = (w & 65535) << 16;
                }
                31 => match (w >> 1) & 1023 {
                    32 => compare = regs[ra].cmp(&regs[rb]),
                    444 => regs[ra] = regs[rd] | regs[rb],
                    _ => panic!("unexpected instruction {w:08x}"),
                },
                16 => {
                    let take = match w & 0xffff_0000 {
                        0x4182_0000 => compare == Ordering::Equal,
                        0x4082_0000 => compare != Ordering::Equal,
                        0x4180_0000 => compare == Ordering::Less,
                        0x4080_0000 => compare != Ordering::Less,
                        _ => panic!("unexpected branch {w:08x}"),
                    };
                    if take {
                        pc = (pc as isize + w as i16 as isize / 4) as usize;
                        continue;
                    }
                }
                18 => {
                    assert_eq!(w & 3, 0, "must not change LR or use an absolute branch");
                    let offset = ((w & 0x03ff_fffc) << 6) as i32 >> 6;
                    assert_eq!(
                        (CODE + pc as u32 * 4).wrapping_add_signed(offset),
                        0x8006_8edc
                    );
                    assert_eq!(regs[3], 0x806f_1234);
                    for index in 0..32 {
                        if ![0, 3, 4, 12].contains(&index) {
                            assert_eq!(regs[index], before[index]);
                        }
                    }
                    return regs[4];
                }
                _ => panic!("unexpected instruction {w:08x}"),
            }
            pc += 1;
        }
        panic!("hook did not return to native object construction");
    }

    #[test]
    fn emitted_hook_balances_objects_without_changing_custom_pools_or_registers() {
        assert_eq!(execute(0x8058_04a8, 2 * 1024 * 1024, 7, true), 0x8058_03d0);
        assert_eq!(
            execute(0x8058_04a8, u64::from(GENERAL_OBJECT_LIMIT) - 1, 7, true),
            0x8058_03d0
        );
        for used in [u64::from(GENERAL_OBJECT_LIMIT), u64::from(u32::MAX) + 1] {
            assert_eq!(execute(0x8058_04a8, used, 7, true), 0x8058_04a8);
        }
        assert_eq!(execute(0x8100_9000, 0, 7, true), 0x8100_9000);
        assert_eq!(execute(0x8058_04a8, 0, 6, true), 0x8058_04a8);
        assert_eq!(execute(0x8058_04a8, 0, 7, false), 0x8058_04a8);
    }

    #[test]
    fn buffer_owner_hook_preserves_native_arguments_stack_and_return_address() {
        use std::collections::BTreeMap;
        let general = 0x805803d0u32;
        let main = 0x805804a8u32;
        for (requested, owner, buffer, count, expected_calls, expected) in [
            (general, main, 0x81000000u32, 7, 2, main),
            (main, general, 0x81000000, 7, 2, general),
            (main, main, 0x81000000, 7, 1, main),
            (main, 0, 0x81000000, 7, 2, main),
            (general, main, 0x81000000, 6, 1, general),
            (main, main, 0, 7, 0, main),
        ] {
            let words: Vec<_> = owner_stub(0x80085004, 0x80084928, 0xd258)
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_be_bytes(*w))
                .collect();
            let mut regs = std::array::from_fn::<_, 32, _>(|i| 0x81000000 + i as u32 * 256);
            regs[1] = 0x817ff000;
            regs[3] = requested;
            regs[4] = buffer;
            regs[5] = u32::MAX;
            regs[6] = 16;
            regs[13] = 0x80564f00;
            let before = regs;
            let mut lr = 0x80012344;
            let mut mem = BTreeMap::from([
                (regs[13].wrapping_add_signed(-11688), 0x80700000),
                (0x80700004, count),
                (0x80700000, 0x80701000),
                (0x80701014, general),
                (0x80701018, main),
                (general, 0x80473928),
                (main, 0x80473928),
                (general + 180, 10),
                (main + 180, 12),
            ]);
            let mut compare = std::cmp::Ordering::Equal;
            let mut pc = 0usize;
            let mut calls = 0;
            let mut returned = false;
            for _ in 0..120 {
                let w = words[pc];
                let rd = ((w >> 21) & 31) as usize;
                let ra = ((w >> 16) & 31) as usize;
                let rb = ((w >> 11) & 31) as usize;
                let address = regs[ra].wrapping_add_signed(w as i16 as i32);
                match w >> 26 {
                    32 => {
                        regs[rd] = *mem
                            .get(&address)
                            .unwrap_or_else(|| panic!("unexpected read {address:08x}"))
                    }
                    36 => {
                        mem.insert(address, regs[rd]);
                    }
                    37 => {
                        let old = regs[rd];
                        mem.insert(address, old);
                        regs[ra] = address;
                    }
                    14 => regs[rd] = address,
                    10 => compare = regs[ra].cmp(&(w & 65535)),
                    11 => compare = (regs[ra] as i32).cmp(&(w as i16 as i32)),
                    31 if w == 0x7c0802a6 => regs[0] = lr,
                    31 if w == 0x7c0803a6 => lr = regs[0],
                    31 if (w >> 1) & 1023 == 0 => {
                        compare = (regs[ra] as i32).cmp(&(regs[rb] as i32))
                    }
                    16 => {
                        let take = match w & 0xffff0000 {
                            0x41820000 => compare.is_eq(),
                            0x40820000 => !compare.is_eq(),
                            0x41800000 => compare.is_lt(),
                            _ => panic!("unexpected branch {w:08x}"),
                        };
                        if take {
                            pc = (pc as isize + w as i16 as isize / 4) as usize;
                            continue;
                        }
                    }
                    18 => {
                        let offset = ((w & 0x03fffffc) << 6) as i32 >> 6;
                        let target = (OWNER_CODE + pc as u32 * 4).wrapping_add_signed(offset);
                        if w & 1 != 0 {
                            assert_eq!(target, 0x80084928);
                            assert_eq!(regs[4], buffer);
                            let found = regs[3] == owner;
                            calls += 1;
                            lr = OWNER_CODE + pc as u32 * 4 + 4;
                            for reg in [0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] {
                                regs[reg] = 0xdead0000 + reg as u32;
                            }
                            regs[3] = u32::from(found);
                        } else if target >= OWNER_CODE
                            && target < OWNER_CODE + words.len() as u32 * 4
                        {
                            pc = ((target - OWNER_CODE) / 4) as usize;
                            continue;
                        } else {
                            assert_eq!(target, ALLOCATION_CODE);
                            returned = true;
                            break;
                        }
                    }
                    _ => panic!("unexpected instruction {w:08x}"),
                }
                pc += 1;
            }
            assert!(returned);
            assert_eq!(calls, expected_calls);
            assert_eq!(regs[3], expected);
            assert_eq!(&regs[4..7], &before[4..7]);
            assert_eq!(&regs[13..32], &before[13..32]);
            assert_eq!(regs[1], before[1]);
            assert_eq!(lr, 0x80012344);
        }
    }

    #[test]
    fn fresh_buffer_fallback_preserves_native_contract_and_never_moves_existing_buffers() {
        let general = 0x805803d0u32;
        let main = 0x805804a8u32;
        let custom = 0x80582000u32;
        // (requested pool, old pointer, size, first succeeds, second succeeds,
        //  pool count, table present, alternate class matches, alternate exists)
        let cases = [
            (general, 0, 0x41000, true, false, 7, true, true, true),
            (general, 0, 0x41000, false, true, 7, true, true, true),
            (main, 0, 0x41000, false, true, 7, true, true, true),
            (general, 0, 0x41000, false, false, 7, true, true, true),
            (general, 0, 0x41000, false, true, 6, true, true, true),
            (general, 0, 0x41000, false, true, 7, false, true, true),
            (general, 0, 0x41000, false, true, 7, true, false, true),
            (general, 0, 0x41000, false, true, 7, true, true, false),
            (
                general, 0x81000000, 0x41000, false, true, 7, true, true, true,
            ),
            (general, 0, 0, false, true, 7, true, true, true),
            (general, 0, u32::MAX, false, true, 7, true, true, true),
            (custom, 0, 0x41000, false, true, 7, true, true, true),
        ];
        for (requested, old, size, first_ok, second_ok, count, present, same_class, exists) in cases
        {
            let words: Vec<_> = allocation_stub(0x80085004, 0xd258)
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_be_bytes(*w))
                .collect();
            assert!(words.len() * 4 <= 256);
            let mut regs = std::array::from_fn::<_, 32, _>(|i| 0x81000000 + i as u32 * 256);
            regs[1] = 0x817ff000;
            regs[3] = requested;
            regs[4] = old;
            regs[5] = size;
            regs[6] = 16;
            regs[13] = 0x80564f00;
            let before = regs;
            let caller = 0x80012344;
            let mut lr = caller;
            let alternate = if requested == general { main } else { general };
            let mut mem = BTreeMap::from([
                (
                    regs[13].wrapping_add_signed(-11688),
                    if present { 0x80700000 } else { 0 },
                ),
                (0x80700004, count),
                (0x80700000, 0x80701000),
                (
                    0x80701014,
                    if exists || requested == general {
                        general
                    } else {
                        0
                    },
                ),
                (
                    0x80701018,
                    if exists || requested == main { main } else { 0 },
                ),
                (general, 0x80473928),
                (main, 0x80473928),
                (custom, 0x80473928),
                (general + 180, 10),
                (main + 180, 12),
                (custom + 180, 14),
            ]);
            if !same_class {
                mem.insert(alternate, 0x80470000);
            }
            let mut compare = std::cmp::Ordering::Equal;
            let mut pc = 0usize;
            let mut calls = Vec::new();
            let mut tail = false;
            let mut returned = false;
            for _ in 0..160 {
                let w = words[pc];
                let rd = ((w >> 21) & 31) as usize;
                let ra = ((w >> 16) & 31) as usize;
                let rb = ((w >> 11) & 31) as usize;
                let address = regs[ra].wrapping_add_signed(w as i16 as i32);
                match w >> 26 {
                    32 => {
                        regs[rd] = *mem
                            .get(&address)
                            .unwrap_or_else(|| panic!("unexpected read {address:08x}"))
                    }
                    36 => {
                        mem.insert(address, regs[rd]);
                    }
                    37 => {
                        let value = regs[rd];
                        mem.insert(address, value);
                        regs[ra] = address;
                    }
                    14 => {
                        regs[rd] = if ra == 0 {
                            w as i16 as i32 as u32
                        } else {
                            address
                        }
                    }
                    10 => compare = regs[ra].cmp(&(w & 65535)),
                    11 => compare = (regs[ra] as i32).cmp(&(w as i16 as i32)),
                    31 if w == 0x7c0802a6 => regs[0] = lr,
                    31 if w == 0x7c0803a6 => lr = regs[0],
                    31 if (w >> 1) & 1023 == 0 => {
                        compare = (regs[ra] as i32).cmp(&(regs[rb] as i32))
                    }
                    16 => {
                        let take = match w & 0xffff0000 {
                            0x41820000 => compare.is_eq(),
                            0x40820000 => !compare.is_eq(),
                            0x41800000 => compare.is_lt(),
                            _ => panic!("unexpected branch {w:08x}"),
                        };
                        if take {
                            pc = (pc as isize + w as i16 as isize / 4) as usize;
                            continue;
                        }
                    }
                    18 => {
                        let offset = ((w & 0x03fffffc) << 6) as i32 >> 6;
                        let target = (ALLOCATION_CODE + pc as u32 * 4).wrapping_add_signed(offset);
                        if w & 1 != 0 {
                            assert_eq!(target, ORIGINAL_CODE);
                            calls.push((regs[3], regs[4], regs[5], regs[6]));
                            let success = if calls.len() == 1 {
                                first_ok
                            } else {
                                second_ok
                            };
                            lr = ALLOCATION_CODE + pc as u32 * 4 + 4;
                            for reg in [0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12] {
                                regs[reg] = 0xdead0000 + reg as u32;
                            }
                            regs[3] = if success { 0x81110000 } else { 0 };
                        } else if (ALLOCATION_CODE..ALLOCATION_CODE + words.len() as u32 * 4)
                            .contains(&target)
                        {
                            pc = ((target - ALLOCATION_CODE) / 4) as usize;
                            continue;
                        } else {
                            assert_eq!(target, 0x80085008);
                            tail = true;
                            returned = true;
                            break;
                        }
                    }
                    19 if w == 0x4e800020 => {
                        assert_eq!(lr, caller);
                        returned = true;
                        break;
                    }
                    _ => panic!("unexpected instruction {w:08x}"),
                }
                pc += 1;
            }
            assert!(returned);
            assert_eq!(&regs[13..32], &before[13..32]);
            assert_eq!(lr, caller);
            let eligible = old == 0 && size != 0 && size != u32::MAX && requested != custom;
            assert_eq!(tail, !eligible);
            if tail {
                assert!(calls.is_empty());
                assert_eq!(&regs[3..7], &before[3..7]);
                assert_eq!(regs[1], before[1] - 80);
            } else {
                assert_eq!(regs[1], before[1]);
                assert_eq!(calls[0], (requested, 0, size, 16));
                let retry = !first_ok && count >= 7 && present && same_class && exists;
                assert_eq!(calls.len(), if retry { 2 } else { 1 });
                if retry {
                    assert_eq!(calls[1], (alternate, 0, size, 16));
                }
                assert_eq!(regs[3] != 0, first_ok || retry && second_ok);
            }
        }
    }

    fn report(release: Release) -> DiscReport {
        DiscReport {
            size: 0,
            output_size: 0,
            release: Some(release),
            copies: [0; 3],
            rarity_nonzero: 0,
            already_modded: false,
            boot_elf: None,
            cnf: None,
            authored: crate::table::EnemyTable {
                hp: vec![],
                stat: vec![],
                crit: vec![],
                para: vec![],
                rarity: vec![],
            },
        }
    }

    #[test]
    fn ps2_does_not_receive_a_powerpc_patch() {
        for release in Release::ALL {
            assert!(regions(&[], &report(release)).unwrap().is_empty());
        }
    }

    #[test]
    fn optional_retail_hooks_match_both_regions_and_reject_changed_code() {
        for (key, release) in [
            ("DW4_RETAIL_GC_USA_ISO", Release::GcUsa),
            ("DW4_RETAIL_GC_JAPAN_ISO", Release::GcJapan),
        ] {
            let Some(path) = std::env::var_os(key) else {
                continue;
            };
            let file = std::fs::File::open(path).unwrap();
            let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let edits = regions(&iso, &report(release)).unwrap();
            assert_eq!(edits.len(), 6);
            assert_eq!(edits[0].bytes.len(), 4);
            assert_eq!(edits[1].bytes.len(), stub(0, 0).len());
            // Only the executable is needed for these refusal checks.
            let dol = word(&iso, 0x420).unwrap() as usize;
            let end = (0..7)
                .map(|i| {
                    dol + word(&iso, dol + i * 4).unwrap() as usize
                        + word(&iso, dol + 0x90 + i * 4).unwrap() as usize
                })
                .max()
                .unwrap();
            let mut changed = iso[..end].to_vec();
            let at = edits[0].offset as usize;
            changed[at] ^= 1;
            assert!(regions(&changed, &report(release)).is_err());
            changed[at] ^= 1;
            changed[edits[1].offset as usize] = 1;
            assert!(regions(&changed, &report(release)).is_err());
            eprintln!("verified {release:?} allocator hook");
        }
    }
}
