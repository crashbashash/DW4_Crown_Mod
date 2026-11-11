//! Balance scene-object arenas while retaining native buffer ownership.
//!
//! PS2 list helpers can pass the object's pool when resizing serialized data
//! allocated in another pool. The operation hook resolves an existing buffer's
//! native owner before changing its bookkeeping. New allocations retain their
//! requested pool. Enemy/resource selection is unchanged.
#[cfg(test)]
use crate::disc::DiscReport;
use crate::{
    elf::ElfInfo,
    error::{Error, Result},
    region::WriteRegion,
    release::Release,
};
use md5::{Digest, Md5};

/// RAM reserved after the original BSS for the hook code.
const RESERVE: u32 = 4096;
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
    /// `igAllocator_ResolveEncodedIndex`: encoded pool index to allocator.
    resolve: u32,
    /// `igArenaMemoryPool` vfunc 84: usable size of an allocation (pool, pointer).
    size_of: u32,
    memcpy: u32,
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
    let (resolve, size_of) = match release {
        Release::Usa => (0x2f0700, 0x31edb0),
        Release::Europe => (0x2f07f0, 0x31eea0),
        _ => (0x2f1dd0, 0x320480),
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
        resolve,
        size_of,
        memcpy: 0x127098,
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

/// Label-addressed MIPS assembler for the operation hook. Branch offsets
/// include the architectural delay slot; `jal`/`j` targets are absolute.
struct Asm {
    base: u32,
    w: Vec<u32>,
    labels: std::collections::BTreeMap<&'static str, usize>,
    fixups: Vec<(usize, &'static str, bool)>,
}
impl Asm {
    fn new(base: u32) -> Self {
        Self {
            base,
            w: Vec::new(),
            labels: Default::default(),
            fixups: Vec::new(),
        }
    }
    fn e(&mut self, w: u32) {
        self.w.push(w);
    }
    fn mark(&mut self, l: &'static str) {
        assert!(self.labels.insert(l, self.w.len()).is_none(), "{l}");
    }
    /// Conditional branch (op 4 beq, 5 bne) to a label, then its delay slot.
    fn b(&mut self, op: u32, rs: u32, rt: u32, l: &'static str, delay: u32) {
        self.fixups.push((self.w.len(), l, false));
        self.e(i(op, rs, rt, 0));
        self.e(delay);
    }
    /// `jal`/`j` to a label inside this hook, then its delay slot.
    fn call(&mut self, link: bool, l: &'static str, delay: u32) {
        self.fixups.push((self.w.len(), l, true));
        self.e(if link { 3 } else { 2 } << 26);
        self.e(delay);
    }
    fn finish(mut self) -> Vec<u32> {
        for (at, l, absolute) in self.fixups {
            let to = self.labels[l];
            self.w[at] |= if absolute {
                (self.base + to as u32 * 4) >> 2
            } else {
                ((to as isize - at as isize - 1) as u32) & 65535
            };
        }
        self.w
    }
}

const SP: u32 = 29;
const RA: u32 = 31;
const V0: u32 = 2;
const A: [u32; 4] = [4, 5, 6, 7];
const T: [u32; 4] = [8, 9, 10, 11];
fn mov(rd: u32, rs: u32) -> u32 {
    r(rs, 0, rd, 0x2d)
}

/// The allocator operation hook, at `base`: `operation(pool, buffer, size,
/// alignment)` allocates (null buffer), frees (size -1) or resizes.
///
/// - Existing buffers of the system (0), rendering (6), general (10) and stage
///   (16) pools are first given to the general/stage arena that really owns
///   them (buffers can be allocated elsewhere than their object's pool).
/// - A failed fresh allocation from those pools is retried in the general
///   arena, or the stage arena for a general request, when that arena has the
///   same allocator implementation. Without this a full system pool returns
///   null to callers that never check (the animation context constructor).
/// - A failed resize allocates the new block the same way, copies at most the
///   old usable size, then frees the old block; on failure the old block is
///   left untouched and null is returned, as natively.
/// - A guard keeps allocations made by the native operation itself (child
///   arenas taking backing memory) on the native path, so a retry never
///   re-enters an arena whose operation is in progress.
/// - The pool table must exist with the stage entry before any lookup.
fn operation_code(n: &Native, base: u32, table: u32, original: [u32; 2]) -> Vec<u32> {
    let mut a = Asm::new(base);
    let [t0, t1, t2, t3] = T;
    let [a0, a1, a2, a3] = A;
    // Entry: only the four arena pools participate.
    a.e(i(35, a0, t2, 180));
    a.b(4, t2, 0, "known", 0);
    for id in [6, 10] {
        a.e(i(9, 0, t3, id));
        a.b(4, t2, t3, "known", 0);
    }
    a.e(i(9, 0, t3, 16));
    a.b(5, t2, t3, "native", 0);
    a.mark("known");
    // Table ready (index 8 = stage pool 16), else native.
    a.e(i(15, 0, t0, hi(table)));
    a.e(i(35, t0, t0, table));
    a.b(4, t0, 0, "native", 0);
    a.e(i(35, t0, t1, 4));
    a.e(i(11, t1, t1, 9)); // sltiu t1, count, 9
    a.b(5, t1, 0, "native", 0);
    a.e(i(9, 0, t3, 0xffff)); // t3 = -1
    a.b(4, a1, 0, "fresh", 0);

    // Existing buffer: resolve its owner, then free/resize.
    a.e(i(9, SP, SP, (-112i32) as u32));
    for (k, reg) in A.iter().enumerate() {
        a.e(i(31, SP, *reg, k as u32 * 16)); // sq
    }
    a.e(i(63, SP, RA, 64));
    a.e(jump(n.owner, true));
    a.e(mov(a0, a1));
    a.e(i(35, SP, a0, 0)); // requested pool (low word)
    a.b(4, V0, 0, "keep", 0);
    a.e(i(35, V0, t0, 0));
    a.e(i(35, a0, t1, 0));
    a.b(5, t0, t1, "keep", 0); // another implementation keeps native behaviour
    a.e(i(35, V0, t0, 180));
    a.e(i(9, 0, t1, 10));
    a.b(4, t0, t1, "replace", 0);
    a.e(i(9, 0, t1, 16));
    a.b(5, t0, t1, "keep", 0);
    a.mark("replace");
    a.e(i(63, SP, V0, 0)); // sd: owner replaces the requested pool
    a.mark("keep");
    for (k, reg) in A.iter().enumerate() {
        a.e(i(30, SP, *reg, k as u32 * 16)); // lq
    }
    a.e(i(9, 0, t3, 0xffff));
    a.b(4, a2, t3, "tail", 0); // free
    a.b(4, a2, 0, "tail", 0); // zero size
    a.e(i(15, 0, t0, hi(base)));
    a.mark("guard_check");
    a.e(i(35, t0, t1, 0)); // patched below to the guard word
    a.b(5, t1, 0, "tail", 0); // nested operation: native only
    a.call(true, "guarded", 0);
    a.b(5, V0, 0, "return", 0);
    a.e(i(30, SP, a0, 0));
    a.e(mov(a1, 0));
    a.e(i(30, SP, a2, 32));
    a.call(true, "fallback", i(30, SP, a3, 48));
    a.b(4, V0, 0, "return", 0); // old block untouched
    a.e(i(63, SP, V0, 80)); // new block
    a.e(i(30, SP, a0, 0));
    a.e(jump(n.size_of, true));
    a.e(i(30, SP, a1, 16));
    a.e(i(30, SP, a2, 32));
    a.e(r(V0, a2, t0, 0x2b)); // t0 = usable < size
    a.b(4, t0, 0, "copy", 0);
    a.e(0);
    a.e(mov(a2, V0));
    a.mark("copy");
    a.e(i(55, SP, a0, 80));
    a.e(jump(n.memcpy, true));
    a.e(i(30, SP, a1, 16));
    a.e(i(30, SP, a0, 0));
    a.e(i(30, SP, a1, 16));
    a.e(i(9, 0, a2, 0xffff));
    a.call(true, "guarded", i(30, SP, a3, 48));
    a.e(i(55, SP, V0, 80));
    a.mark("return");
    a.e(i(55, SP, RA, 64));
    a.e(r(RA, 0, 0, 8)); // jr ra
    a.e(i(9, SP, SP, 112));
    a.mark("tail");
    a.e(i(55, SP, RA, 64));
    a.call(false, "native", i(9, SP, SP, 112));

    // Fresh allocation: zero size and free stay native.
    a.mark("fresh");
    a.b(4, a2, 0, "native", 0);
    a.b(4, a2, t3, "native", 0);
    a.e(i(15, 0, t0, hi(base)));
    a.mark("guard_check2");
    a.e(i(35, t0, t1, 0));
    a.b(5, t1, 0, "native", 0);
    a.call(false, "fallback", 0);

    // fallback(pool, 0, size, alignment): native, then the other arena.
    a.mark("fallback");
    a.e(i(9, SP, SP, (-80i32) as u32));
    for (k, reg) in A.iter().enumerate() {
        a.e(i(31, SP, *reg, k as u32 * 16));
    }
    a.e(i(63, SP, RA, 64));
    a.call(true, "guarded", 0);
    a.b(5, V0, 0, "fallback_return", 0);
    a.e(i(35, SP, t0, 0));
    a.e(i(35, t0, t0, 180));
    a.e(i(9, 0, t1, 10));
    a.b(5, t0, t1, "resolve", i(9, 0, a0, 10)); // others borrow from general
    a.e(i(9, 0, a0, 16)); // general borrows from stage
    a.mark("resolve");
    a.e(jump(n.resolve, true));
    a.e(0);
    a.b(4, V0, 0, "fallback_return", 0);
    a.e(i(35, SP, t0, 0));
    a.e(i(35, t0, t0, 0));
    a.e(i(35, V0, t1, 0));
    a.b(5, t0, t1, "fallback_fail", 0); // matching allocator implementation
    a.e(mov(a0, V0));
    a.e(i(30, SP, a1, 16));
    a.e(i(30, SP, a2, 32));
    a.call(true, "guarded", i(30, SP, a3, 48));
    a.b(4, 0, 0, "fallback_return", 0);
    a.mark("fallback_fail");
    a.e(mov(V0, 0));
    a.mark("fallback_return");
    a.e(i(55, SP, RA, 64));
    a.e(r(RA, 0, 0, 8));
    a.e(i(9, SP, SP, 80));

    // guarded(...): the native operation with the guard set.
    a.mark("guarded");
    a.e(i(9, SP, SP, (-16i32) as u32));
    a.e(i(63, SP, RA, 0));
    a.e(i(15, 0, t0, hi(base)));
    a.mark("guard_set");
    a.e(i(9, 0, t1, 1));
    a.e(i(43, t0, t1, 0)); // sw t1, guard
    a.call(true, "native", 0);
    a.e(i(15, 0, t0, hi(base)));
    a.mark("guard_clear");
    a.e(i(43, t0, 0, 0)); // sw zero, guard
    a.e(i(55, SP, RA, 0));
    a.e(r(RA, 0, 0, 8));
    a.e(i(9, SP, SP, 16));

    // The native operation's displaced prologue.
    a.mark("native");
    a.e(original[0]);
    a.e(original[1]);
    a.e(jump(n.operation + 8, false));
    a.e(0);
    a.mark("guard");
    a.e(0);
    let guard = base + a.labels["guard"] as u32 * 4;
    // Each guard access is `lui t0,hi(guard)` followed by an access at lo(guard).
    for (l, at) in [
        ("guard_check", 0),
        ("guard_check2", 0),
        ("guard_set", 1),
        ("guard_clear", 0),
    ] {
        let k = a.labels[l];
        a.w[k - 1] = i(15, 0, t0, hi(guard));
        a.w[k + at] = (a.w[k + at] & !65535) | (guard & 65535);
    }
    a.finish()
}

/// The address of the pool-table pointer that `resolve` reads for even
/// (pool-ID) indices: `lui at,H` at +0x0c and `lw v0,L(at)` at +0x30.
fn table_pointer(iso: &[u8], at: impl Fn(u32) -> Result<usize>, resolve: u32) -> Result<u32> {
    let lui = word(iso, at(resolve + 0x0c)?)?;
    let lw = word(iso, at(resolve + 0x30)?)?;
    need(lui >> 16 == 0x3c01 && lw >> 16 == 0x8c22)?;
    Ok(((lui & 65535) << 16).wrapping_add(lw as i16 as i32 as u32))
}

/// The IOP module stored right after the boot ELF on every supported disc.
/// Relocating it (the image appends a copy) frees its sectors for the ELF's
/// hook code; the game loads modules by path.
pub(crate) fn relocated_module(
    iso: &[u8],
    release: Release,
) -> Result<Option<crate::iso9660::IsoFile>> {
    if native(release).is_none() {
        return Ok(None);
    }
    let boot = crate::iso9660::find_file(iso, release.serial())?;
    let module = crate::iso9660::find_in_directory(iso, "MODULES", "CRI_ADXI.IRX")?;
    need(module.lba == boot.lba + boot.size.div_ceil(2048))?;
    Ok(Some(module))
}

#[cfg(test)]
pub(crate) fn regions(iso: &[u8], report: &DiscReport) -> Result<Vec<WriteRegion>> {
    match report.release {
        Some(release) => regions_for_release(
            iso,
            release,
            false,
            relocated_module(iso, release)?.as_ref(),
        ),
        None => Ok(Vec::new()),
    }
}

/// `module` is the relocated IOP module whose old sectors the ELF may grow into
/// (see [`relocated_module`]); without it the hook must fit the ELF's last sector.
pub(crate) fn regions_for_release(
    iso: &[u8],
    release: Release,
    difficulty: bool,
    module: Option<&crate::iso9660::IsoFile>,
) -> Result<Vec<WriteRegion>> {
    let Some(n) = native(release) else {
        return Ok(vec![]);
    };
    let boot = crate::iso9660::find_file(iso, release.serial())?;
    let elf = ElfInfo::read(iso, &boot)?;
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
    let table = table_pointer(iso, at, n.resolve)?;
    code.extend(operation_code(&n, end + owner_offset, table, original));
    let mut code = encoded(&code);
    let (extra, routing) = if difficulty {
        crate::difficulty_io::mips_regions(iso, release, &elf, end + code.len() as u32)?
    } else {
        (Vec::new(), Vec::new())
    };
    code.extend(extra);
    need(code.len() <= RESERVE as usize)?;
    let code_file = boot.size.checked_add(15).ok_or_else(bad)? & !15;
    let new_size = code_file.checked_add(code.len() as u32).ok_or_else(bad)?;
    let sector_end = boot.size.div_ceil(2048) * 2048;
    let room = match module {
        Some(m) => {
            need(m.lba == boot.lba + sector_end / 2048)?;
            sector_end + m.size.div_ceil(2048) * 2048
        }
        None => sector_end,
    };
    need(new_size <= room)?;
    // The ELF's own sector padding must be unused; a relocated module's old
    // sectors are no longer referenced by the image.
    let padding_start = base + boot.size as usize;
    let padding_end = base + new_size.min(sector_end) as usize;
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
    edits.extend(routing);
    edits.extend(crate::anim_pool::ps2_regions(iso, release, &elf)?);
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

    /// A small EE interpreter for the operation hook: real control flow with
    /// delay slots, and the native operation and helpers modelled in Rust.
    struct Ee {
        regs: [u128; 32],
        mem: BTreeMap<u32, u8>,
        // pool address -> (id, vtable, free bytes)
        pools: BTreeMap<u32, (u32, u32, u32)>,
        // block -> (owning pool, size)
        blocks: BTreeMap<u32, (u32, u32)>,
        next: u32,
        // (pool, buffer, size, guard set)
        natives: Vec<(u32, u32, u32, bool)>,
        copies: Vec<(u32, u32, u32)>,
        owner_calls: usize,
    }
    const BASE: u32 = 0x58e000;
    const TABLE: u32 = 0x586f58;
    const RET: u32 = 0x1234560;
    const VT: u32 = 0x531030;
    impl Ee {
        fn new(table_ready: bool) -> Self {
            let mut e = Self {
                regs: std::array::from_fn(|k| {
                    ((0x5a5a_0000 + k as u128) << 64) | (0x900000 + k as u128 * 16)
                }),
                mem: BTreeMap::new(),
                pools: BTreeMap::new(),
                blocks: BTreeMap::new(),
                next: 0x1000000,
                natives: vec![],
                copies: vec![],
                owner_calls: 0,
            };
            e.regs[0] = 0;
            e.regs[29] = 0x1fff000;
            e.regs[31] = RET as u128;
            for (addr, id, vt, free) in [
                (0x591000u32, 0u32, VT, 64u32),
                (0x592000, 6, VT, 64),
                (0x596760, 10, VT, 100_000),
                (0x596a48, 16, VT, 100_000),
                (0x597000, 20, VT, 0),
            ] {
                e.pools.insert(addr, (id, vt, free));
                e.w32(addr, vt);
                e.w32(addr + 180, id);
            }
            if table_ready {
                e.w32(TABLE, 0x591020);
                e.w32(0x591020, 0x591030);
                e.w32(0x591024, 12);
            } else {
                e.w32(TABLE, 0);
            }
            e
        }
        fn w32(&mut self, a: u32, v: u32) {
            for (k, b) in v.to_le_bytes().into_iter().enumerate() {
                self.mem.insert(a + k as u32, b);
            }
        }
        fn r(&self, a: u32, n: usize) -> u128 {
            let mut b = [0u8; 16];
            for (k, v) in b.iter_mut().take(n).enumerate() {
                *v = *self
                    .mem
                    .get(&(a + k as u32))
                    .unwrap_or_else(|| panic!("read {a:x}"));
            }
            u128::from_le_bytes(b)
        }
        fn st(&mut self, a: u32, v: u128, n: usize) {
            for (k, b) in v.to_le_bytes().into_iter().take(n).enumerate() {
                self.mem.insert(a + k as u32, b);
            }
        }
        fn lo(&self, k: usize) -> u32 {
            self.regs[k] as u32
        }
        fn set(&mut self, k: usize, v: u64) {
            if k != 0 {
                self.regs[k] = self.regs[k] & !u128::from(u64::MAX) | u128::from(v);
            }
        }
        fn guard(&self, guard: u32) -> u32 {
            self.r(guard, 4) as u32
        }
        /// The native operation, `guard` must be set while it runs.
        fn native(&mut self, guard: u32) -> u32 {
            let (pool, buffer, size) = (self.lo(4), self.lo(5), self.lo(6));
            let guarded = self.guard(guard) == 1;
            self.natives.push((pool, buffer, size, guarded));
            let free = self.pools[&pool].2;
            if buffer == 0 {
                if size > free {
                    return 0;
                }
                self.pools.get_mut(&pool).unwrap().2 -= size;
                let b = self.next;
                self.next += 0x1000;
                self.blocks.insert(b, (pool, size));
                b
            } else if size == u32::MAX {
                let (owner, n) = self.blocks.remove(&buffer).expect("double free");
                assert_eq!(
                    owner, pool,
                    "freed through a pool that does not own the block"
                );
                self.pools.get_mut(&pool).unwrap().2 += n;
                0
            } else {
                let (owner, n) = self.blocks[&buffer];
                assert_eq!(
                    owner, pool,
                    "resized through a pool that does not own the block"
                );
                if size > n && size - n > free {
                    return 0;
                }
                self.blocks.insert(buffer, (pool, size));
                buffer
            }
        }
        fn call(&mut self, target: u32, n: &Native) -> u32 {
            match target {
                t if t == n.owner => {
                    self.owner_calls += 1;
                    self.blocks.get(&self.lo(4)).map_or(0, |b| b.0)
                }
                t if t == n.resolve => {
                    let id = self.lo(4);
                    *self.pools.iter().find(|p| p.1 .0 == id).unwrap().0
                }
                t if t == n.size_of => self.blocks[&self.lo(5)].1,
                t if t == n.memcpy => {
                    self.copies.push((self.lo(4), self.lo(5), self.lo(6)));
                    self.lo(4)
                }
                _ => panic!("unexpected call {target:x}"),
            }
        }
        fn exec(&mut self, w: u32) {
            let (rs, rt, rd) = (
                ((w >> 21) & 31) as usize,
                ((w >> 16) & 31) as usize,
                ((w >> 11) & 31) as usize,
            );
            let a = (self.lo(rs)).wrapping_add(w as i16 as i32 as u32);
            match w >> 26 {
                9 => self.set(rt, u64::from(a) as i32 as i64 as u64),
                11 => self.set(
                    rt,
                    u64::from((self.regs[rs] as u64) < (w as i16 as i64 as u64)),
                ),
                15 => self.set(rt, ((w & 65535) << 16) as i32 as i64 as u64),
                35 => self.set(rt, self.r(a, 4) as u32 as i32 as i64 as u64),
                43 => self.st(a, self.regs[rt], 4),
                55 => self.set(rt, self.r(a, 8) as u64),
                63 => self.st(a, self.regs[rt], 8),
                30 => self.regs[rt] = self.r(a, 16),
                31 => self.st(a, self.regs[rt], 16),
                0 if w == 0 => {}
                0 => match w & 63 {
                    0x2d => self.set(
                        rd,
                        (self.regs[rs] as u64).wrapping_add(self.regs[rt] as u64),
                    ),
                    0x2b => self.set(
                        rd,
                        u64::from((self.regs[rs] as u64) < (self.regs[rt] as u64)),
                    ),
                    _ => panic!("instruction {w:08x}"),
                },
                _ => panic!("instruction {w:08x}"),
            }
        }
        /// Runs the hook from its entry until it returns to the caller.
        fn run(&mut self, code: &[u32], n: &Native) -> u32 {
            let guard = BASE + (code.len() as u32 - 1) * 4;
            for (k, w) in code.iter().enumerate() {
                let at = BASE + k as u32 * 4;
                if !self.mem.contains_key(&at) {
                    self.w32(at, *w);
                }
            }
            let mut pc = BASE;
            for _ in 0..2000 {
                if pc == RET {
                    return self.lo(2);
                }
                if pc == n.operation + 8 {
                    // The displaced prologue reserved 256 bytes; the native epilogue returns.
                    let v = self.native(guard);
                    self.set(2, u64::from(v));
                    for k in [1, 3, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                        self.regs[k] = u128::MAX - k as u128;
                    }
                    self.regs[29] += 256;
                    pc = self.lo(31);
                    continue;
                }
                let w = code[((pc - BASE) / 4) as usize];
                let (rs, rt) = (((w >> 21) & 31) as usize, ((w >> 16) & 31) as usize);
                let slot = |e: &mut Self| e.exec(code[((pc + 4 - BASE) / 4) as usize]);
                match w >> 26 {
                    4 | 5 => {
                        let eq = self.regs[rs] as u64 == self.regs[rt] as u64;
                        let take = (w >> 26 == 4) == eq;
                        slot(self);
                        pc = if take {
                            pc + 4 + ((w as i16 as i32) << 2) as u32
                        } else {
                            pc + 8
                        };
                    }
                    2 | 3 => {
                        let target = (w & 0x03ff_ffff) << 2;
                        if w >> 26 == 3 {
                            self.set(31, u64::from(pc + 8));
                        }
                        slot(self);
                        if (BASE..BASE + code.len() as u32 * 4).contains(&target)
                            || target == n.operation + 8
                        {
                            pc = target;
                        } else {
                            let v = self.call(target, n);
                            self.set(2, u64::from(v));
                            for k in [1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                                self.regs[k] = u128::MAX - k as u128;
                            }
                            pc = self.lo(31);
                        }
                    }
                    0 if w & 63 == 8 => {
                        let to = self.lo(rs);
                        slot(self);
                        pc = to;
                    }
                    _ => {
                        self.exec(w);
                        pc += 4;
                    }
                }
            }
            panic!("hook did not return");
        }
    }

    fn hook() -> (Vec<u32>, Native) {
        let n = native(Release::Usa).unwrap();
        (operation_code(&n, BASE, TABLE, [0x27bdff00, 0x3c010058]), n)
    }

    fn op(e: &mut Ee, pool: u32, buffer: u32, size: u32) -> u32 {
        let (code, n) = hook();
        e.set(4, u64::from(pool));
        e.set(5, u64::from(buffer));
        e.set(6, u64::from(size) as i32 as i64 as u64);
        e.set(7, 16);
        let before = e.regs;
        let v = e.run(&code, &n);
        for k in [16, 17, 18, 19, 20, 21, 22, 23, 28, 29, 30, 31] {
            assert_eq!(e.regs[k], before[k], "register {k}");
        }
        assert_eq!(
            e.guard(BASE + (code.len() as u32 - 1) * 4),
            0,
            "guard left set"
        );
        v
    }

    #[test]
    fn fresh_allocations_retry_in_the_general_or_stage_arena() {
        let (system, render, general, stage, other) =
            (0x591000, 0x592000, 0x596760, 0x596a48, 0x597000);
        // Native success: one native call, nothing borrowed.
        let mut e = Ee::new(true);
        let b = op(&mut e, system, 0, 32);
        assert_eq!(e.blocks[&b].0, system);
        assert_eq!(e.natives.len(), 1);
        // A full system or rendering pool borrows from the general arena,
        // a full general arena from the stage arena.
        for (pool, expected) in [
            (system, general),
            (render, general),
            (general, stage),
            (stage, general),
        ] {
            let mut e = Ee::new(true);
            e.pools.get_mut(&pool).unwrap().2 = 0;
            let b = op(&mut e, pool, 0, 4208);
            assert_eq!(e.blocks[&b].0, expected, "{pool:x}");
            assert_eq!(e.natives.len(), 2);
            assert!(
                e.natives.iter().all(|n| n.3),
                "retried allocations run guarded"
            );
        }
        // Unknown pools, zero size, a missing table or a mismatched
        // implementation stay native.
        let mut e = Ee::new(true);
        assert_eq!(op(&mut e, other, 0, 64), 0);
        assert_eq!(
            e.natives,
            vec![(other, 0, 64, false)],
            "unknown pool: untouched native path"
        );
        let mut e = Ee::new(false);
        e.pools.get_mut(&system).unwrap().2 = 0;
        assert_eq!(op(&mut e, system, 0, 64), 0);
        let mut e = Ee::new(true);
        e.pools.get_mut(&system).unwrap().2 = 0;
        e.w32(general, 0xabcdef);
        assert_eq!(op(&mut e, system, 0, 64), 0);
        assert_eq!(e.natives.len(), 1);
    }

    #[test]
    fn nested_operations_never_retry() {
        let (code, n) = hook();
        let guard = BASE + (code.len() as u32 - 1) * 4;
        let mut e = Ee::new(true);
        e.pools.get_mut(&0x591000).unwrap().2 = 0;
        e.w32(guard, 1);
        e.set(4, 0x591000);
        e.set(5, 0);
        e.set(6, 64);
        assert_eq!(e.run(&code, &n), 0);
        assert_eq!(e.natives.len(), 1);
    }

    #[test]
    fn frees_and_resizes_reach_the_owning_arena() {
        let (system, general) = (0x591000, 0x596760);
        // A system-pool buffer that was borrowed from the general arena.
        let mut e = Ee::new(true);
        e.pools.get_mut(&system).unwrap().2 = 0;
        let b = op(&mut e, system, 0, 100);
        assert_eq!(e.blocks[&b].0, general);
        assert_eq!(op(&mut e, system, b, u32::MAX), 0); // freed in general (model asserts)
        assert!(e.blocks.is_empty());
        // A failed in-place resize migrates: new block, bounded copy, old freed.
        let mut e = Ee::new(true);
        let old = op(&mut e, system, 0, 48);
        let new = op(&mut e, system, old, 400);
        assert_eq!(e.blocks[&new].0, general);
        // Native resize, fresh native, general retry, then the old block's free.
        let resize: Vec<_> = e.natives[1..].iter().map(|n| (n.0, n.2, n.3)).collect();
        assert_eq!(
            resize,
            vec![
                (system, 400, true),
                (system, 400, true),
                (general, 400, true),
                (system, u32::MAX, true)
            ]
        );
        assert!(!e.blocks.contains_key(&old));
        assert_eq!(e.copies, vec![(new, old, 48)]);
        // Shrinking copies at most the new size.
        let mut e = Ee::new(true);
        e.pools.get_mut(&system).unwrap().2 = 1000;
        let old = op(&mut e, system, 0, 900);
        e.pools.get_mut(&system).unwrap().2 = 0;
        e.blocks.get_mut(&old).unwrap().1 = 900;
        // Model: growth fails in place, so force a migration with a larger size.
        let new = op(&mut e, system, old, 1200);
        assert_eq!(e.copies.last(), Some(&(new, old, 900)));
        // If no arena can take the new size, the old block is untouched.
        let mut e = Ee::new(true);
        let old = op(&mut e, system, 0, 48);
        for p in [system, general, 0x596a48] {
            e.pools.get_mut(&p).unwrap().2 = 0;
        }
        assert_eq!(op(&mut e, system, old, 400), 0);
        assert_eq!(e.blocks[&old], (system, 48));
        assert!(e.copies.is_empty());
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
            // The hook grows into the relocated module's old sectors.
            let module = relocated_module(&iso, release).unwrap().unwrap();
            let stop = (module.lba as usize + module.size.div_ceil(2048) as usize) * 2048;
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
            // USA has 720 bytes of sector padding: the hook needs the module's sectors there.
            assert_eq!(
                regions_for_release(&iso, release, false, None).is_err(),
                release == Release::Usa
            );
            eprintln!(
                "verified PS2 {release:?} allocator hooks ({} bytes) and 4 KiB reservation",
                edits.iter().map(|e| e.bytes.len()).max().unwrap()
            );
        }
    }
}
