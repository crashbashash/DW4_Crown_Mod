//! Balance scene-object arenas while retaining native buffer ownership.
//!
//! PS2 list helpers can pass the object's pool when resizing serialized data
//! allocated in another pool. The operation hook resolves an existing buffer's
//! native owner before changing its bookkeeping. New allocations retain their
//! requested pool. Enemy/resource selection is unchanged.
use crate::{
    disc::DiscReport,
    elf::ElfInfo,
    error::{Error, Result},
    region::WriteRegion,
    release::Release,
};
use md5::{Digest, Md5};

const RESERVE: u32 = 1024;
const GENERAL_LIMIT: u32 = 3 * 1024 * 1024;

struct Native {
    site: u32,
    create: u32,
    operation: u32,
    owner: u32,
    heap_hi: u32,
    info_hi: u32,
    libc_heap: u32,
    fingerprints: [&'static str; 4],
}

fn native(release: Release) -> Option<Native> {
    let (site, create, operation, owner, heap_hi, info_hi, libc_heap, fingerprints) = match release
    {
        Release::Usa => (
            0x301b74,
            0x2fdb40,
            0x31f480,
            0x2f04f0,
            0x1793d4,
            0x32f3b0,
            0x481e54,
            [
                "d0286b3a024922e8ecbd4261f2fe4429",
                "09715a99fba8f3e7fa91324d75fdffce",
                "4049e175a24d2a81a358b7a0f6f4c2a9",
                "1dbdecdcfd69a5b2be9a108a946d6864",
            ],
        ),
        Release::Europe => (
            0x301c64,
            0x2fdc30,
            0x31f570,
            0x2f05e0,
            0x179444,
            0x32f4a0,
            0x482b54,
            [
                "3583898a0b23d04a74cc9f0154e94dc6",
                "09715a99fba8f3e7fa91324d75fdffce",
                "4049e175a24d2a81a358b7a0f6f4c2a9",
                "c8e788d8d4c990f8141bb0e9b8283438",
            ],
        ),
        Release::Japan => (
            0x303244,
            0x2ff210,
            0x320b50,
            0x2f1bc0,
            0x17aaa4,
            0x330a80,
            0x483554,
            [
                "73149f88fc107a83ddcc6319c4eaabee",
                "09715a99fba8f3e7fa91324d75fdffce",
                "f51bfc84d7f5dbd4ae88a2e792e97583",
                "6c4c0eafa8e6646009da5d535d705546",
            ],
        ),
        _ => return None,
    };
    Some(Native {
        site,
        create,
        operation,
        owner,
        heap_hi,
        info_hi,
        libc_heap,
        fingerprints,
    })
}

fn bad() -> Error {
    Error::CrownData("unrecognised PS2 arena allocation hook or ELF reservation".into())
}
fn need(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(bad())
    }
}
fn word(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(at..at + 4).ok_or_else(bad)?.try_into().unwrap(),
    ))
}
fn encoded(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}
fn i(op: u32, rs: u32, rt: u32, imm: u32) -> u32 {
    op << 26 | rs << 21 | rt << 16 | (imm & 65535)
}
fn r(rs: u32, rt: u32, rd: u32, fun: u32) -> u32 {
    rs << 21 | rt << 16 | rd << 11 | fun
}
fn jump(address: u32, link: bool) -> u32 {
    (if link { 3 } else { 2 }) << 26 | (address >> 2)
}
fn hi(address: u32) -> u32 {
    address.wrapping_add(0x8000) >> 16
}

// Conditional offsets include the architectural branch-delay slot.
struct Code {
    words: Vec<u32>,
    branches: Vec<(usize, usize)>,
}
impl Code {
    fn new() -> Self {
        Self {
            words: Vec::new(),
            branches: Vec::new(),
        }
    }
    fn emit(&mut self, w: u32) {
        self.words.push(w);
    }
    fn branch(&mut self, op: u32, rs: u32, rt: u32, label: usize) {
        self.branches.push((self.words.len(), label));
        self.emit(i(op, rs, rt, 0));
    }
    fn finish(mut self, labels: &[usize]) -> Vec<u32> {
        for (at, label) in self.branches {
            self.words[at] |= ((labels[label] as isize - at as isize - 1) as u32) & 65535;
        }
        self.words
    }
}

fn object_code(create: u32) -> Vec<u32> {
    let mut c = Code::new();
    c.branch(4, 5, 0, 0);
    c.emit(i(35, 17, 2, 280)); // temp pool, delay slot
    c.branch(4, 2, 0, 0);
    c.emit(i(35, 17, 3, 284)); // data pool
    c.branch(5, 5, 3, 0);
    c.emit(i(35, 5, 1, 180));
    c.emit(i(9, 0, 3, 16));
    c.branch(5, 1, 3, 0);
    c.emit(i(35, 2, 1, 180));
    c.emit(i(9, 0, 3, 10));
    c.branch(5, 1, 3, 0);
    c.emit(i(35, 5, 1, 0));
    c.emit(i(35, 2, 3, 0));
    c.branch(5, 1, 3, 0);
    c.emit(i(55, 2, 3, 56));
    c.emit(i(15, 0, 1, GENERAL_LIMIT >> 16));
    c.emit(r(3, 1, 1, 0x2b));
    c.branch(4, 1, 0, 0);
    c.emit(0);
    c.emit(r(2, 0, 5, 0x2d)); // selected pool = general
    let tail = c.words.len();
    c.emit(jump(create, false));
    c.emit(0);
    c.finish(&[tail])
}

fn owner_code(owner: u32, operation: u32, original: [u32; 2]) -> Vec<u32> {
    // Only the stage/general arenas participate. Resolve ownership before the
    // native operation takes its locks. Preserve all four full EE argument
    // registers, the caller's return address and the original stack layout.
    let mut c = Code::new();
    c.branch(4, 5, 0, 0);
    c.emit(0); // new allocation: retain requested pool
    c.emit(i(35, 4, 2, 180));
    c.emit(i(9, 0, 3, 10));
    c.branch(4, 2, 3, 1);
    c.emit(0);
    c.emit(i(9, 0, 3, 16));
    c.branch(5, 2, 3, 0);
    c.emit(0);
    let lookup = c.words.len();
    c.emit(i(9, 29, 29, (-80i32) as u32));
    for (reg, off) in [(4, 0), (5, 16), (6, 32), (7, 48)] {
        c.emit(i(31, 29, reg, off));
    }
    c.emit(i(63, 29, 31, 64));
    c.emit(jump(owner, true));
    c.emit(r(5, 0, 4, 0x2d));
    c.branch(4, 2, 0, 2);
    c.emit(0);
    // An owner with another pool implementation must retain native behaviour.
    c.emit(i(35, 2, 3, 0));
    c.emit(i(35, 29, 1, 0));
    c.emit(i(35, 1, 1, 0));
    c.branch(5, 3, 1, 2);
    c.emit(0);
    c.emit(i(35, 2, 3, 180));
    c.emit(i(9, 0, 1, 10));
    c.branch(4, 3, 1, 3);
    c.emit(0);
    c.emit(i(9, 0, 1, 16));
    c.branch(5, 3, 1, 2);
    c.emit(0);
    let replace = c.words.len();
    c.emit(i(63, 29, 2, 0));
    let restore = c.words.len();
    for (reg, off) in [(4, 0), (5, 16), (6, 32), (7, 48)] {
        c.emit(i(30, 29, reg, off));
    }
    c.emit(i(55, 29, 31, 64));
    c.emit(i(9, 29, 29, 80));
    let unchanged = c.words.len();
    c.emit(original[0]);
    c.emit(original[1]);
    c.emit(jump(operation + 8, false));
    c.emit(0);
    c.finish(&[unchanged, lookup, restore, replace])
}

pub(crate) fn regions(iso: &[u8], report: &DiscReport) -> Result<Vec<WriteRegion>> {
    let Some(n) = report.release.and_then(native) else {
        return Ok(vec![]);
    };
    let boot = report.boot_elf.as_ref().ok_or_else(bad)?;
    let elf = ElfInfo::read(iso, boot)?;
    let at = |address| {
        elf.vaddr_to_iso_offset(address)
            .map(|v| v as usize)
            .ok_or_else(bad)
    };
    for ((address, len), fingerprint) in [
        (n.site - 24, 64),
        (n.create, 32),
        (n.operation, 32),
        (n.owner, 32),
    ]
    .into_iter()
    .zip(n.fingerprints)
    {
        let offset = at(address)?;
        need(
            format!(
                "{:x}",
                Md5::digest(iso.get(offset..offset + len).ok_or_else(bad)?)
            ) == fingerprint,
        )?;
    }
    let base = elf.iso_offset as usize;
    let phoff = word(iso, base + 28)? as usize;
    let packed = word(iso, base + 40)?;
    let phsize = (packed >> 16) as usize;
    let phnum = u16::from_le_bytes(
        iso.get(base + 44..base + 46)
            .ok_or_else(bad)?
            .try_into()
            .unwrap(),
    );
    need(phsize == 32 && phnum == 2 && word(iso, base + 24)? == 0x100008)?;
    let ph0 = base + phoff;
    let ph1 = ph0 + phsize;
    need(word(iso, ph0)? == 1 && word(iso, ph0 + 8)? == 0x100000)?;
    let end = word(iso, ph0 + 8)?
        .checked_add(word(iso, ph0 + 20)?)
        .ok_or_else(bad)?;
    let heap_start = end.checked_add(RESERVE).ok_or_else(bad)?;
    need(heap_start < 0x02000000 && end.is_multiple_of(16))?;
    need(
        word(iso, ph1)? == 1
            && word(iso, ph1 + 8)? == end
            && word(iso, ph1 + 12)? == end
            && word(iso, ph1 + 16)? == 0
            && word(iso, ph1 + 20)? == 0
            && word(iso, ph1 + 24)? == 6
            && word(iso, ph1 + 28)? == 16,
    )?;

    let original = [
        word(iso, at(n.operation)?)?,
        word(iso, at(n.operation + 4)?)?,
    ];
    need(original[0] == 0x27bdff00 && original[1] & 0xffff0000 == 0x3c010000)?;
    let mut code = object_code(n.create);
    while !code.len().is_multiple_of(4) {
        code.push(0);
    }
    let owner_offset = code.len() as u32 * 4;
    code.extend(owner_code(n.owner, n.operation, original));
    let code = encoded(&code);
    need(code.len() <= RESERVE as usize)?;
    let code_file = boot.size.checked_add(15).ok_or_else(bad)? & !15;
    let new_size = code_file.checked_add(code.len() as u32).ok_or_else(bad)?;
    need(new_size <= boot.size.div_ceil(2048) * 2048)?;
    let padding_start = base + boot.size as usize;
    let padding_end = base + new_size as usize;
    need(
        iso.get(padding_start..padding_end)
            .is_some_and(|v| v.iter().all(|b| *b == 0)),
    )?;
    let size_at = boot.record_offset as usize + 10;
    need(
        word(iso, size_at)? == boot.size
            && iso.get(size_at + 4..size_at + 8) == Some(boot.size.to_be_bytes().as_slice()),
    )?;

    let mut edits = vec![];
    let mut put = |offset: usize, bytes: Vec<u8>| {
        if iso[offset..offset + bytes.len()] != bytes {
            edits.push(WriteRegion {
                offset: offset as u64,
                bytes,
            });
        }
    };
    // BSS clearing retains the original exclusive end, so it cannot erase the
    // extra load segment. Advance all three heap/layout paths and libc's
    // initialized break pointer beyond the reserved segment.
    need(
        word(iso, at(0x100120)?)? == i(15, 0, 3, hi(end))
            && word(iso, at(0x100128)?)? == i(9, 3, 3, end),
    )?;
    for (high, low, reg) in [
        (0x1001d0, 0x1001d8, 4),
        (n.heap_hi, n.heap_hi + 16, 19),
        (n.info_hi, n.info_hi + 8, 18),
    ] {
        need(
            word(iso, at(high)?)? == i(15, 0, reg, hi(end))
                && word(iso, at(low)?)? == i(9, reg, reg, end),
        )?;
        put(at(high)?, encoded(&[i(15, 0, reg, hi(heap_start))]));
        put(at(low)?, encoded(&[i(9, reg, reg, heap_start)]));
    }
    let libc = at(n.libc_heap)?;
    need(
        iso.get(libc - 16..libc) == Some(b"libkernl2800\0\0\0\0".as_slice())
            && word(iso, libc)? == end,
    )?;
    put(libc, encoded(&[heap_start]));
    put(base + code_file as usize, code.clone());
    put(
        ph1,
        encoded(&[1, code_file, end, end, code.len() as u32, RESERVE, 7, 16]),
    );
    let mut size = new_size.to_le_bytes().to_vec();
    size.extend(new_size.to_be_bytes());
    put(size_at, size);
    need(word(iso, at(n.site)?)? == jump(n.create, true))?;
    put(at(n.site)?, encoded(&[jump(end, true)]));
    put(
        at(n.operation)?,
        encoded(&[jump(end + owner_offset, false), 0]),
    );
    Ok(edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Cpu {
        regs: [u128; 32],
        memory: BTreeMap<u64, u8>,
        calls: usize,
    }
    impl Cpu {
        fn new() -> Self {
            let mut regs = std::array::from_fn(|i| {
                ((0xaabbcc00 + i as u128) << 64) | (0x810000 + i as u128 * 256)
            });
            regs[0] = 0;
            regs[29] = 0x1fff000;
            Self {
                regs,
                memory: BTreeMap::new(),
                calls: 0,
            }
        }
        fn low(&mut self, reg: usize, value: u64) {
            if reg != 0 {
                self.regs[reg] = self.regs[reg] & !u128::from(u64::MAX) | u128::from(value);
            }
        }
        fn store(&mut self, at: u64, value: u128, n: usize) {
            for (i, b) in value.to_le_bytes().into_iter().take(n).enumerate() {
                self.memory.insert(at + i as u64, b);
            }
        }
        fn load(&self, at: u64, n: usize) -> u128 {
            let mut b = [0; 16];
            for (i, value) in b.iter_mut().take(n).enumerate() {
                *value = *self
                    .memory
                    .get(&(at + i as u64))
                    .unwrap_or_else(|| panic!("unexpected read {at:x}, size {n}"));
            }
            u128::from_le_bytes(b)
        }
        fn op(&mut self, w: u32) {
            let rs = ((w >> 21) & 31) as usize;
            let rt = ((w >> 16) & 31) as usize;
            let rd = ((w >> 11) & 31) as usize;
            let imm = w as i16 as i64 as u64;
            let address = (self.regs[rs] as u64).wrapping_add(imm);
            match w >> 26 {
                9 => self.low(rt, address as i32 as i64 as u64),
                15 => self.low(rt, ((w & 65535) << 16) as i32 as i64 as u64),
                35 => self.low(rt, self.load(address, 4) as i32 as i64 as u64),
                55 => self.low(rt, self.load(address, 8) as u64),
                63 => self.store(address, self.regs[rt], 8),
                31 => self.store(address, self.regs[rt], 16),
                30 => self.regs[rt] = self.load(address, 16),
                0 if w == 0 => (),
                0 => match w & 63 {
                    0x2d => self.low(
                        rd,
                        (self.regs[rs] as u64).wrapping_add(self.regs[rt] as u64),
                    ),
                    0x2b => self.low(rd, u64::from((self.regs[rs] as u64) < self.regs[rt] as u64)),
                    _ => panic!("unexpected instruction {w:08x}"),
                },
                _ => panic!("unexpected instruction {w:08x}"),
            }
        }
        fn run(&mut self, code: &[u32], base: u32, native_owner: Option<(u32, u32)>) -> u32 {
            let mut pc = 0usize;
            for _ in 0..100 {
                let w = code[pc];
                let rs = ((w >> 21) & 31) as usize;
                let rt = ((w >> 16) & 31) as usize;
                match w >> 26 {
                    4 | 5 => {
                        let equal = self.regs[rs] as u64 == self.regs[rt] as u64;
                        let take = if w >> 26 == 4 { equal } else { !equal };
                        self.op(code[pc + 1]);
                        pc = if take {
                            (pc as isize + 1 + w as i16 as isize) as usize
                        } else {
                            pc + 2
                        };
                    }
                    2 => {
                        self.op(code[pc + 1]);
                        return (w & 0x03ffffff) << 2;
                    }
                    3 => {
                        let (callee, owner) = native_owner.expect("unexpected native call");
                        assert_eq!((w & 0x03ffffff) << 2, callee);
                        self.low(31, u64::from(base + pc as u32 * 4 + 8));
                        self.op(code[pc + 1]);
                        self.calls += 1;
                        // Exercise restoration after the normal native ABI clobbers.
                        for reg in 1..16 {
                            self.regs[reg] = u128::MAX - reg as u128;
                        }
                        self.low(2, u64::from(owner));
                        pc += 2;
                    }
                    _ => {
                        self.op(w);
                        pc += 1;
                    }
                }
            }
            panic!("native hook did not tail-call");
        }
    }

    #[test]
    fn object_hook_respects_pool_identity_and_general_reserve() {
        for (used, selected, temp, expected) in [
            (0, 0x111000u32, 0x112000u32, 0x112000u32),
            (u64::from(GENERAL_LIMIT) - 1, 0x111000, 0x112000, 0x112000),
            (u64::from(GENERAL_LIMIT), 0x111000, 0x112000, 0x111000),
            (1u64 << 32, 0x111000, 0x112000, 0x111000),
            (0, 0x113000, 0x112000, 0x113000),
            (0, 0x111000, 0, 0x111000),
            (0, 0, 0x112000, 0),
        ] {
            let mut c = Cpu::new();
            c.low(5, selected.into());
            c.low(17, 0x120000);
            c.store(0x120000 + 280, temp.into(), 4);
            c.store(0x120000 + 284, 0x111000, 4);
            for (pool, id) in [(0x111000, 16), (0x112000, 10), (0x113000, 20)] {
                c.store(pool, 0x531030, 4);
                c.store(pool + 180, id, 4);
            }
            c.store(0x112000 + 56, used.into(), 8);
            let before = c.regs;
            assert_eq!(c.run(&object_code(0x2fdb40), 0x58d280, None), 0x2fdb40);
            assert_eq!(c.regs[5] as u64, u64::from(expected));
            for (reg, saved) in before.iter().enumerate() {
                if ![1, 2, 3, 5].contains(&reg) {
                    assert_eq!(c.regs[reg], *saved);
                }
            }
        }
    }

    #[test]
    fn existing_buffers_use_their_native_owner_with_ee_arguments_and_return_preserved() {
        let main = 0x111000u32;
        let general = 0x112000u32;
        let foreign = 0x113000u32;
        for (requested, owner, buffer, calls, expected) in [
            (general, main, 0x160000u32, 1, main),
            (main, general, 0x170000, 1, general),
            (main, main, 0x160000, 1, main),
            (general, 0, 0x160000, 1, general),
            (general, foreign, 0x160000, 1, general),
            (foreign, main, 0x160000, 0, foreign),
            (main, main, 0, 0, main),
        ] {
            let mut c = Cpu::new();
            c.low(4, requested.into());
            c.low(5, buffer.into());
            c.low(6, u64::MAX);
            c.low(7, 16);
            for (pool, id) in [(main, 16), (general, 10), (foreign, 20)] {
                c.store(
                    pool.into(),
                    if pool == foreign { 0xabcdef } else { 0x531030 },
                    4,
                );
                c.store(u64::from(pool) + 180, id, 4);
            }
            let before = c.regs;
            let code = owner_code(0x2f04f0, 0x31f480, [0x27bdff00, 0x3c010058]);
            assert_eq!(c.run(&code, 0x58d2e0, Some((0x2f04f0, owner))), 0x31f488);
            assert_eq!(c.calls, calls);
            assert_eq!(c.regs[4] as u64, u64::from(expected));
            assert_eq!(c.regs[4] >> 64, before[4] >> 64);
            for reg in [5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23, 28, 30, 31] {
                assert_eq!(c.regs[reg], before[reg], "register {reg}");
            }
            assert_eq!(c.regs[29], before[29] - 256);
        }
    }

    fn report(release: Release, boot: crate::iso9660::IsoFile) -> DiscReport {
        DiscReport {
            size: 0,
            output_size: 0,
            release: Some(release),
            copies: [0; 3],
            rarity_nonzero: 0,
            already_modded: false,
            authored: crate::table::EnemyTable {
                hp: vec![],
                stat: vec![],
                crit: vec![],
                para: vec![],
                rarity: vec![],
            },
            boot_elf: Some(boot),
            cnf: None,
        }
    }

    #[test]
    fn optional_retail_reservations_map_code_without_erasing_bss_or_reusing_heap_space() {
        for (key, release) in [
            ("DW4_RETAIL_USA_ISO", Release::Usa),
            ("DW4_RETAIL_EUROPE_ISO", Release::Europe),
            ("DW4_RETAIL_JAPAN_ISO", Release::Japan),
        ] {
            let Some(path) = std::env::var_os(key) else {
                continue;
            };
            let file = std::fs::File::open(path).unwrap();
            let iso = unsafe { memmap2::Mmap::map(&file).unwrap() };
            let boot = crate::iso9660::find_file(&iso, release.serial()).unwrap();
            let report = report(release, boot);
            let edits = regions(&iso, &report).unwrap();
            let elf = ElfInfo::read(&iso, &boot).unwrap();
            let end = elf.phdrs[0].p_vaddr + {
                let base = elf.iso_offset as usize;
                let ph = word(&iso, base + 28).unwrap() as usize;
                word(&iso, base + ph + 20).unwrap()
            };
            let stop = elf.iso_offset as usize + boot.size.div_ceil(2048) as usize * 2048;
            let mut changed = iso[..stop].to_vec();
            for edit in &edits {
                changed[edit.offset as usize..edit.offset as usize + edit.bytes.len()]
                    .copy_from_slice(&edit.bytes);
            }
            let n = native(release).unwrap();
            let new_size = word(&changed, boot.record_offset as usize + 10).unwrap();
            let updated = ElfInfo::read(
                &changed,
                &crate::iso9660::IsoFile {
                    size: new_size,
                    ..boot
                },
            )
            .unwrap();
            let code = updated.vaddr_to_iso_offset(end).unwrap() as usize;
            assert_eq!(word(&changed, code).unwrap(), object_code(n.create)[0]);
            assert_eq!(
                word(
                    &changed,
                    elf.vaddr_to_iso_offset(0x100128).unwrap() as usize
                )
                .unwrap(),
                i(9, 3, 3, end)
            );
            assert_eq!(
                word(
                    &changed,
                    elf.vaddr_to_iso_offset(n.libc_heap).unwrap() as usize
                )
                .unwrap(),
                end + RESERVE
            );
            assert!(regions(&changed, &report).is_err(), "cannot apply twice");
            let mut bad_code = iso[..stop].to_vec();
            bad_code[elf.vaddr_to_iso_offset(n.site).unwrap() as usize] ^= 1;
            assert!(regions(&bad_code, &report).is_err());
            let mut bad_padding = iso[..stop].to_vec();
            bad_padding[elf.iso_offset as usize + boot.size as usize] = 1;
            assert!(regions(&bad_padding, &report).is_err());
            eprintln!("verified PS2 {release:?} allocator hooks and 1 KiB reservation");
        }
    }
}
