//! Resolve generated `Normal|Hard|VeryHard` resource names before native IO.
//! Only stage manifests contain these selectors. Ordinary names retain the
//! original path; native bootstrap, sound transfer and cache ownership remain.
use crate::{
    elf::ElfInfo,
    error::{Error, Result},
    region::WriteRegion,
    release::Release,
};
use md5::{Digest, Md5};
use std::collections::BTreeMap;

struct Native {
    read: u32,
    preload: u32,
    difficulty: u32,
    hashes: [&'static str; 3],
    pool: u32,
    intern: u32,
    release_string: u32,
    string_hashes: [&'static str; 2],
}

fn native(release: Release) -> Native {
    let (read, preload, difficulty, hashes) = match release {
        Release::Usa => (
            0x3092f0,
            0x309750,
            0x371da0,
            [
                "a8e0887888cc1cdd80a12b64895594cd",
                "07ec8347678d474174066888080f4845",
                "58ee22edafe168c175f0f30d975c764a",
            ],
        ),
        Release::Europe => (
            0x3093e0,
            0x309840,
            0x371ed0,
            [
                "a8e0887888cc1cdd80a12b64895594cd",
                "a739ff9289b9b6dbdbf06630785ca0df",
                "889da12237a37fc2804da3290c18120a",
            ],
        ),
        Release::Japan => (
            0x30a9c0,
            0x30ae20,
            0x373470,
            [
                "a8e0887888cc1cdd80a12b64895594cd",
                "eba47fef1a85d50ae800b22828cf0b15",
                "96f714e7c2ca3385acffe05abd0e18b8",
            ],
        ),
        Release::GcUsa => (
            0x8006fd34,
            0x80070140,
            0x802ebb4c,
            [
                "2d8f1fc3c32051164eb6cc5d2c354b70",
                "c272d6133d60962e14f025b13cbd47ec",
                "07880e9a198f8ed804717e93b703734c",
            ],
        ),
        Release::GcJapan => (
            0x8006fcd4,
            0x800700e0,
            0x802ebb24,
            [
                "2d8f1fc3c32051164eb6cc5d2c354b70",
                "c272d6133d60962e14f025b13cbd47ec",
                "b42f3f87b4b811a6f0a41ca12fb46b8c",
            ],
        ),
    };
    let (pool, intern, release_string, string_hashes) = match release {
        Release::Usa => (
            0x586f20,
            0x2eaf60,
            0x30c940,
            [
                "d4e3af0e7fc571b2441589e41d57fdd1",
                "3284e18344689c614ba4ae71b45c453c",
            ],
        ),
        Release::Europe => (
            0x587d20,
            0x2eb050,
            0x30ca30,
            [
                "d4e3af0e7fc571b2441589e41d57fdd1",
                "d6d5a140b52155a2fa54cf899e43641f",
            ],
        ),
        Release::Japan => (
            0x588aa0,
            0x2ec630,
            0x30e010,
            [
                "d4e3af0e7fc571b2441589e41d57fdd1",
                "3bd0972c227370b1773325b32f3b0c4d",
            ],
        ),
        Release::GcUsa => (
            0xd240,
            0x80054094,
            0x8007381c,
            [
                "45fbf03f3c1a958b787cc08e370af0f2",
                "60380621599d28c935c744249700b87b",
            ],
        ),
        Release::GcJapan => (
            0xd240,
            0x80054034,
            0x800737bc,
            [
                "45fbf03f3c1a958b787cc08e370af0f2",
                "60380621599d28c935c744249700b87b",
            ],
        ),
    };
    Native {
        read,
        preload,
        difficulty,
        hashes,
        pool,
        intern,
        release_string,
        string_hashes,
    }
}

fn bad() -> Error {
    Error::CrownData("unrecognised difficulty resource loader hook".into())
}
fn need(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(bad())
    }
}
fn guard(
    iso: &[u8],
    n: &Native,
    map: &impl Fn(u32, usize) -> Result<usize>,
    gc: bool,
) -> Result<()> {
    for ((address, size), hash) in [
        (n.read, 32),
        (n.preload, 32),
        (n.difficulty, if gc { 52 } else { 48 }),
    ]
    .into_iter()
    .zip(n.hashes)
    {
        let at = map(address, size)?;
        need(format!("{:x}", Md5::digest(&iso[at..at + size])) == hash)?;
    }
    for (address, hash) in [n.intern, n.release_string]
        .into_iter()
        .zip(n.string_hashes)
    {
        let at = map(address, 32)?;
        need(format!("{:x}", Md5::digest(&iso[at..at + 32])) == hash)?;
    }
    Ok(())
}

fn mi(op: u32, rs: u32, rt: u32, imm: i32) -> u32 {
    op << 26 | rs << 21 | rt << 16 | imm as u32 & 65535
}
fn mr(rs: u32, rt: u32, rd: u32, op: u32) -> u32 {
    rs << 21 | rt << 16 | rd << 11 | op
}
fn mj(target: u32, link: bool) -> u32 {
    (if link { 3 } else { 2 }) << 26 | (target >> 2 & 0x03ffffff)
}

#[derive(Default)]
struct Code {
    words: Vec<u32>,
    labels: BTreeMap<&'static str, usize>,
    fixups: Vec<(usize, u32, &'static str)>,
}
impl Code {
    fn emit(&mut self, w: u32) {
        self.words.push(w);
    }
    fn mark(&mut self, name: &'static str) {
        self.labels.insert(name, self.words.len());
    }
    fn mb(&mut self, op: u32, rs: u32, rt: u32, label: &'static str) {
        self.fixups
            .push((self.words.len(), mi(op, rs, rt, 0), label));
        self.emit(0);
    }
    fn pb(&mut self, op: u32, label: &'static str) {
        self.fixups.push((self.words.len(), op, label));
        self.emit(0);
    }
    fn finish(mut self, gc: bool) -> Vec<u32> {
        for (at, op, label) in self.fixups {
            let distance = self.labels[label] as i32 - at as i32 - i32::from(!gc);
            self.words[at] = op
                | if gc {
                    (distance * 4) as u32 & if op >> 26 == 18 { 0x03fffffc } else { 0xfffc }
                } else {
                    distance as u32 & 65535
                };
        }
        self.words
    }
}

fn mips_code(difficulty: u32, pool: u32, intern: u32, release: u32) -> Vec<u32> {
    let mut c = Code::default();
    c.emit(mi(15, 0, 1, 0)); // trampoline high half, filled after assembly
    c.emit(mr(25, 1, 25, 0x21));
    c.emit(mi(36, 5, 8, 0));
    c.emit(mi(9, 0, 1, 91));
    c.mb(5, 8, 1, "native");
    c.emit(mr(5, 0, 8, 0x2d));
    c.mark("find");
    c.emit(mi(36, 8, 9, 0));
    c.mb(4, 9, 0, "native");
    c.emit(mi(9, 8, 8, 1));
    c.emit(mi(9, 0, 1, 124));
    c.mb(5, 9, 1, "find");
    c.emit(0);
    c.emit(mi(9, 29, 29, -64));
    c.emit(mi(31, 29, 4, 0));
    c.emit(mi(31, 29, 5, 16));
    c.emit(mi(63, 29, 31, 32));
    c.emit(mi(43, 29, 25, 40));
    c.emit(mj(difficulty, true));
    c.emit(0);
    c.emit(mi(11, 2, 3, 3));
    c.mb(5, 3, 0, "valid");
    c.emit(0);
    c.emit(mr(0, 0, 2, 0x2d));
    c.mark("valid");
    c.emit(mi(35, 29, 8, 16));
    c.mark("skip");
    c.mb(4, 2, 0, "copy_start");
    c.emit(mi(9, 0, 1, 124));
    c.emit(mi(36, 8, 9, 0));
    c.mb(4, 9, 0, "restore");
    c.emit(mi(9, 8, 8, 1));
    c.mb(5, 9, 1, "skip");
    c.emit(0);
    c.emit(mi(9, 2, 2, -1));
    c.mb(4, 0, 0, "skip");
    c.emit(0);
    c.mark("copy_start");
    c.emit(mi(9, 29, 9, 48));
    c.emit(mi(9, 29, 11, 63));
    c.mark("copy");
    c.mb(4, 9, 11, "restore");
    c.emit(mi(36, 8, 10, 0));
    c.mb(4, 10, 0, "copied");
    c.emit(mi(9, 0, 1, 124));
    c.mb(4, 10, 1, "copied");
    c.emit(mi(9, 0, 1, 47));
    c.mb(4, 10, 1, "copied");
    c.emit(mi(9, 8, 8, 1));
    c.emit(mi(40, 9, 10, 0));
    c.mb(4, 0, 0, "copy");
    c.emit(mi(9, 9, 9, 1));
    c.mark("copied");
    c.emit(mi(40, 9, 0, 0));
    // Native directories retain igString tokens, including their preceding
    // reference header. A stack C string cannot outlive an asynchronous read.
    c.emit(mi(15, 0, 4, ((pool + 0x8000) >> 16) as i32));
    c.emit(mi(35, 4, 4, pool as i32));
    c.emit(mj(intern, true));
    c.emit(mi(9, 29, 5, 48));
    c.emit(mi(43, 29, 2, 44));
    c.emit(mi(30, 29, 4, 0));
    c.emit(mr(2, 0, 5, 0x2d));
    c.emit(mi(35, 29, 25, 40));
    c.emit(mr(25, 0, 31, 9));
    c.emit(0);
    c.emit(mi(31, 29, 2, 0));
    c.emit(mi(35, 29, 8, 44));
    c.emit(mi(35, 8, 9, -4));
    c.emit(mi(9, 9, 9, -1));
    c.emit(mi(43, 8, 9, -4));
    c.mb(5, 9, 0, "released");
    c.emit(mi(35, 8, 4, -8));
    c.emit(mj(release, true));
    c.emit(0);
    c.mark("released");
    c.emit(mi(30, 29, 2, 0));
    c.emit(mi(55, 29, 31, 32));
    c.emit(mr(31, 0, 0, 8));
    c.emit(mi(9, 29, 29, 64));
    c.mark("restore");
    c.emit(mi(30, 29, 4, 0));
    c.emit(mi(30, 29, 5, 16));
    c.emit(mi(35, 29, 25, 40));
    c.emit(mi(55, 29, 31, 32));
    c.emit(mi(9, 29, 29, 64));
    c.mark("native");
    c.emit(mr(25, 0, 0, 8));
    c.emit(0);
    c.finish(false)
}

pub(crate) fn mips_regions(
    iso: &[u8],
    release: Release,
    elf: &ElfInfo,
    base: u32,
) -> Result<(Vec<u8>, Vec<WriteRegion>)> {
    let n = native(release);
    let map = |address: u32, size: usize| {
        let at = elf.vaddr_to_iso_offset(address).ok_or_else(bad)? as usize;
        need(iso.get(at..at + size).is_some())?;
        Ok(at)
    };
    guard(iso, &n, &map, false)?;
    let mut code = mips_code(n.difficulty, n.pool, n.intern, n.release_string);
    let start = base + code.len() as u32 * 4;
    let mut edits = Vec::new();
    for site in [n.read, n.preload] {
        let trampoline = base + code.len() as u32 * 4;
        need(((trampoline + 0x8000) >> 16) == ((start + 0x8000) >> 16))?;
        let at = map(site, 8)?;
        code.extend(
            iso[at..at + 8]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| u32::from_le_bytes(*b)),
        );
        code.extend([mj(site + 8, false), 0]);
        edits.push(WriteRegion {
            offset: at as u64,
            bytes: [mj(base, false), mi(9, 0, 25, trampoline as i32)]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        });
    }
    code[0] = mi(15, 0, 1, ((start + 0x8000) >> 16) as i32);
    Ok((code.into_iter().flat_map(u32::to_le_bytes).collect(), edits))
}

fn ppc_code(base: u32, difficulty: u32, pool: u32, intern: u32, release: u32) -> Vec<u32> {
    let mut c = Code::default();
    c.emit(0x3d8c8000); // addis r12,r12,0x8000: trampoline address
    c.emit(0x88040000);
    c.emit(0x2c00005b);
    c.pb(0x40820000, "native");
    c.emit(0x7c8b2378);
    c.mark("find");
    c.emit(0x880b0000);
    c.emit(0x2c000000);
    c.pb(0x41820000, "native");
    c.emit(0x396b0001);
    c.emit(0x2c00007c);
    c.pb(0x40820000, "find");
    c.emit(0x9421ffb0);
    c.emit(0x7c0802a6);
    c.emit(0x90010034);
    c.emit(0x90610028);
    c.emit(0x9081002c);
    c.emit(0x91810030);
    let call = base + c.words.len() as u32 * 4;
    c.emit(0x48000001 | difficulty.wrapping_sub(call) & 0x03fffffc);
    c.emit(0x28030002);
    c.pb(0x40810000, "valid");
    c.emit(0x38600000);
    c.mark("valid");
    c.emit(0x80a1002c);
    c.mark("skip");
    c.emit(0x2c030000);
    c.pb(0x41820000, "copy_start");
    c.emit(0x88050000);
    c.emit(0x38a50001);
    c.emit(0x2c000000);
    c.pb(0x41820000, "restore");
    c.emit(0x2c00007c);
    c.pb(0x40820000, "skip");
    c.emit(0x3863ffff);
    c.pb(0x48000000, "skip");
    c.mark("copy_start");
    c.emit(0x38c10040);
    c.emit(0x38e1004f);
    c.mark("copy");
    c.emit(0x7c063840);
    c.pb(0x41820000, "restore");
    c.emit(0x88050000);
    c.emit(0x2c000000);
    c.pb(0x41820000, "copied");
    c.emit(0x2c00007c);
    c.pb(0x41820000, "copied");
    c.emit(0x2c00002f);
    c.pb(0x41820000, "copied");
    c.emit(0x98060000);
    c.emit(0x38a50001);
    c.emit(0x38c60001);
    c.pb(0x48000000, "copy");
    c.mark("copied");
    c.emit(0x38000000);
    c.emit(0x98060000);
    c.emit(0x806d0000 | pool); // native global string pool
    c.emit(0x38810040);
    let at = base + c.words.len() as u32 * 4;
    c.emit(0x48000001 | intern.wrapping_sub(at) & 0x03fffffc);
    c.emit(0x9061002c);
    c.emit(0x7c641b78); // selected managed string -> r4
    c.emit(0x80610028);
    c.emit(0x81810030);
    c.emit(0x7d8903a6);
    c.emit(0x4e800421);
    c.emit(0x90610028); // retain native return across string release
    c.emit(0x8081002c);
    c.emit(0x8064fffc);
    c.emit(0x3403ffff); // addic. r0,r3,-1
    c.emit(0x9004fffc);
    c.pb(0x40820000, "released");
    c.emit(0x8064fff8);
    let at = base + c.words.len() as u32 * 4;
    c.emit(0x48000001 | release.wrapping_sub(at) & 0x03fffffc);
    c.mark("released");
    c.emit(0x80610028);
    c.emit(0x80010034);
    c.emit(0x7c0803a6);
    c.emit(0x38210050);
    c.emit(0x4e800020);
    c.mark("restore");
    c.emit(0x80610028);
    c.emit(0x8081002c);
    c.emit(0x81810030);
    c.emit(0x80010034);
    c.emit(0x7c0803a6);
    c.emit(0x38210050);
    c.mark("native");
    c.emit(0x7d8903a6);
    c.emit(0x4e800420);
    c.finish(true)
}

pub(crate) fn gamecube_regions(iso: &[u8], release: Release) -> Result<Vec<WriteRegion>> {
    const BASE: u32 = 0x800046c0;
    const SPACE: usize = 460;
    let n = native(release);
    let be = |at: usize| -> Result<u32> {
        Ok(u32::from_be_bytes(
            iso.get(at..at + 4)
                .ok_or_else(bad)?
                .try_into()
                .map_err(|_| bad())?,
        ))
    };
    let dol = be(0x420)? as usize;
    let map = |address: u32, length: usize| -> Result<usize> {
        for index in 0..7 {
            let start = be(dol + 0x48 + index * 4)?;
            let size = be(dol + 0x90 + index * 4)?;
            if address >= start && u64::from(address - start) + length as u64 <= u64::from(size) {
                let at = dol + be(dol + index * 4)? as usize + (address - start) as usize;
                need(iso.get(at..at + length).is_some())?;
                return Ok(at);
            }
        }
        Err(bad())
    };
    guard(iso, &n, &map, true)?;
    let at = map(BASE, SPACE)?;
    need(iso[at..at + SPACE].iter().all(|b| *b == 0))?;
    let mut code = ppc_code(BASE, n.difficulty, n.pool, n.intern, n.release_string);
    let mut edits = Vec::new();
    for site in [n.read, n.preload] {
        let trampoline = BASE + code.len() as u32 * 4;
        let source = map(site, 8)?;
        code.extend([be(source)?, be(source + 4)?]);
        code.push(0x48000000 | (site + 8).wrapping_sub(trampoline + 8) & 0x03fffffc);
        edits.push(WriteRegion {
            offset: source as u64,
            bytes: [
                0x39800000 | (trampoline & 65535),
                0x48000000 | BASE.wrapping_sub(site + 4) & 0x03fffffc,
            ]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect(),
        });
    }
    let bytes: Vec<_> = code.into_iter().flat_map(u32::to_be_bytes).collect();
    need(bytes.len() <= SPACE)?;
    edits.push(WriteRegion {
        offset: at as u64,
        bytes,
    });
    Ok(edits)
}

#[cfg(test)]
#[path = "difficulty_io/tests.rs"]
mod tests;
