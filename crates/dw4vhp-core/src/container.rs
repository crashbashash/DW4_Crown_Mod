//! Disc containers around a plain GameCube image.
//!
//! The patcher works on a plain image, where every byte sits at its disc
//! offset. `.iso` and NKit `.iso` files already are one (NKit only zeroes the
//! unused junk). CISO and RVZ files store the same bytes in blocks and groups,
//! so they are unpacked into a temporary plain image first and packed again
//! after patching.
//!
//! Unused "junk" padding that RVZ stores as a seed is written back as zeros.
//! No game reads it. Wii discs, WIA files and other compressors are refused.

use crate::error::{Error, Result};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A plain image: `.iso`, `.gcm` or NKit `.iso`.
    Raw,
    Ciso,
    Rvz,
}

impl Format {
    /// The file extension an output in this format gets.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Raw => "iso",
            Self::Ciso => "ciso",
            Self::Rvz => "rvz",
        }
    }
}

fn bad(why: impl Into<String>) -> Error {
    Error::Container(why.into())
}

pub fn detect(path: &Path) -> Result<Format> {
    let mut magic = [0u8; 4];
    let mut file = File::open(path)?;
    let got = file.read(&mut magic)?;
    Ok(match &magic[..got] {
        b"CISO" => Format::Ciso,
        b"RVZ\x01" => Format::Rvz,
        b"WIA\x01" => return Err(bad("WIA files are not supported, only RVZ")),
        _ => Format::Raw,
    })
}

/// Reports `(done, total)` bytes as a container is converted.
pub type Progress<'a> = &'a mut dyn FnMut(u64, u64);

/// Expands `src` into a plain image at `dest`.
pub fn unpack(format: Format, src: &Path, dest: &Path, progress: Progress) -> Result<()> {
    match format {
        Format::Raw => Err(bad("a plain image needs no unpacking")),
        Format::Ciso => ciso_unpack(src, dest, progress),
        Format::Rvz => rvz_unpack(src, dest, progress),
    }
}

/// Packs the plain image `raw` into a new `format` file at `dest`.
pub fn pack(format: Format, raw: &Path, dest: &Path, progress: Progress) -> Result<()> {
    match format {
        Format::Raw => Err(bad("a plain image needs no packing")),
        Format::Ciso => ciso_pack(raw, dest, progress),
        Format::Rvz => rvz_pack(raw, dest, progress),
    }
}

fn read_at(file: &mut File, at: u64, len: usize) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(at))?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)
        .map_err(|_| bad("the file is truncated"))?;
    Ok(buf)
}

fn is_zero(block: &[u8]) -> bool {
    block.iter().all(|&b| b == 0)
}

// ---------------------------------------------------------------- CISO ----

const CISO_HEADER: usize = 0x8000;
const CISO_BLOCK: usize = 0x20_0000;

fn ciso_unpack(src: &Path, dest: &Path, progress: Progress) -> Result<()> {
    let mut file = File::open(src)?;
    let len = file.metadata()?.len();
    let head = read_at(&mut file, 0, CISO_HEADER)?;
    let block = u32::from_le_bytes(head[4..8].try_into().unwrap()) as u64;
    if block == 0 || !block.is_multiple_of(2048) || block > 64 << 20 {
        return Err(bad("unexpected CISO block size"));
    }
    let map = &head[8..];
    let present: Vec<usize> = (0..map.len()).filter(|&i| map[i] == 1).collect();
    if map.iter().any(|&m| m > 1) || present.is_empty() {
        return Err(bad("unexpected CISO block map"));
    }
    let stored = len
        .checked_sub(CISO_HEADER as u64)
        .ok_or_else(|| bad("the file is truncated"))?;
    let last = *present.last().unwrap() as u64;
    // Most tools pad the last block, and some leave a short footer after it.
    let count = present.len() as u64;
    let tail = if stored >= count * block {
        block
    } else {
        stored - (count - 1) * block
    };
    if tail == 0 {
        return Err(bad("the CISO file length does not match its block map"));
    }
    let logical = last * block + tail;

    let mut out = File::create(dest)?;
    out.set_len(logical)?;
    file.seek(SeekFrom::Start(CISO_HEADER as u64))?;
    let mut buf = vec![0u8; block as usize];
    for (n, &index) in present.iter().enumerate() {
        let size = if n + 1 == present.len() {
            tail as usize
        } else {
            block as usize
        };
        file.read_exact(&mut buf[..size])
            .map_err(|_| bad("the file is truncated"))?;
        out.seek(SeekFrom::Start(index as u64 * block))?;
        out.write_all(&buf[..size])?;
        progress(n as u64 + 1, present.len() as u64);
    }
    Ok(())
}

fn ciso_pack(raw: &Path, dest: &Path, progress: Progress) -> Result<()> {
    let mut input = File::open(raw)?;
    let len = input.metadata()?.len();
    let blocks = len.div_ceil(CISO_BLOCK as u64) as usize;
    if blocks > CISO_HEADER - 8 {
        return Err(bad("the image is too large for the CISO format"));
    }
    let mut head = vec![0u8; CISO_HEADER];
    head[..4].copy_from_slice(b"CISO");
    head[4..8].copy_from_slice(&(CISO_BLOCK as u32).to_le_bytes());
    let mut out = BufWriter::new(File::create(dest)?);
    out.write_all(&head)?;
    let mut buf = vec![0u8; CISO_BLOCK];
    for index in 0..blocks {
        let size = (len - index as u64 * CISO_BLOCK as u64).min(CISO_BLOCK as u64) as usize;
        buf.fill(0);
        input.read_exact(&mut buf[..size])?;
        // The last block is kept even if zero and padded, so the length survives.
        if !is_zero(&buf) || index + 1 == blocks {
            head[8 + index] = 1;
            out.write_all(&buf)?;
        }
        progress(index as u64 + 1, blocks as u64);
    }
    let mut out = out.into_inner().map_err(|e| e.into_error())?;
    out.seek(SeekFrom::Start(0))?;
    out.write_all(&head)?;
    Ok(())
}

// ----------------------------------------------------------------- RVZ ----

const RVZ_HEAD1: usize = 0x48;
const RVZ_HEAD2: usize = 0xDC;
const RVZ_CHUNK: usize = 0x2_0000;
const RVZ_ZSTD: u32 = 5;
const RVZ_GROUP_ENTRY: usize = 12;
const RVZ_RAW_ENTRY: usize = 24;
const RVZ_SEED: usize = 68;

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}
fn be64(b: &[u8], at: usize) -> u64 {
    u64::from_be_bytes(b[at..at + 8].try_into().unwrap())
}

fn decompress(kind: u32, data: Vec<u8>, expected: usize) -> Result<Vec<u8>> {
    match kind {
        0 => Ok(data),
        RVZ_ZSTD => zstd::bulk::decompress(&data, expected + 1024)
            .map_err(|e| bad(format!("a compressed block is damaged ({e})"))),
        _ => Err(bad("only uncompressed and zstd RVZ files are supported")),
    }
}

/// Expands an RVZ-packed group: raw runs are copied and junk runs become zeros.
fn depack(packed: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(expected);
    let mut at = 0usize;
    while at < packed.len() {
        if at + 4 > packed.len() {
            return Err(bad("a packed block is damaged"));
        }
        let word = be32(packed, at);
        at += 4;
        let size = (word & 0x7FFF_FFFF) as usize;
        if out.len() + size > expected {
            return Err(bad("a packed block is damaged"));
        }
        if word >> 31 == 1 {
            at += RVZ_SEED;
            out.resize(out.len() + size, 0);
        } else {
            let run = packed
                .get(at..at + size)
                .ok_or_else(|| bad("a packed block is damaged"))?;
            out.extend_from_slice(run);
            at += size;
        }
    }
    if out.len() != expected {
        return Err(bad("a packed block has the wrong length"));
    }
    Ok(out)
}

fn rvz_unpack(src: &Path, dest: &Path, progress: Progress) -> Result<()> {
    let mut file = File::open(src)?;
    let file_len = file.metadata()?.len();
    let head = read_at(&mut file, 0, RVZ_HEAD1 + RVZ_HEAD2)?;
    if be32(&head, 8) > 0x0003_0000 {
        return Err(bad("this RVZ version is newer than the patcher supports"));
    }
    if be32(&head, 12) as usize != RVZ_HEAD2 || be64(&head, 0x2C) != file_len {
        return Err(bad("the RVZ header does not match the file"));
    }
    let iso_size = be64(&head, 0x24);
    let h2 = &head[RVZ_HEAD1..];
    if be32(h2, 0) != 1 {
        return Err(bad("only GameCube discs are supported, not Wii discs"));
    }
    let compression = be32(h2, 4);
    let chunk = be32(h2, 12) as usize;
    if chunk == 0 || !chunk.is_multiple_of(2048) {
        return Err(bad("unexpected RVZ chunk size"));
    }
    if be32(h2, 0x90) != 0 {
        return Err(bad(
            "this disc has partitions, so it is not a GameCube disc",
        ));
    }
    let raw_count = be32(h2, 0xB4) as usize;
    let raw_at = be64(h2, 0xB8);
    let raw_size = be32(h2, 0xC0) as usize;
    let group_count = be32(h2, 0xC4) as usize;
    let group_at = be64(h2, 0xC8);
    let group_size = be32(h2, 0xD0) as usize;

    let raw_table = decompress(
        compression,
        read_at(&mut file, raw_at, raw_size)?,
        raw_count * RVZ_RAW_ENTRY,
    )?;
    let group_table = decompress(
        compression,
        read_at(&mut file, group_at, group_size)?,
        group_count * RVZ_GROUP_ENTRY,
    )?;
    if raw_table.len() < raw_count * RVZ_RAW_ENTRY
        || group_table.len() < group_count * RVZ_GROUP_ENTRY
    {
        return Err(bad("the RVZ tables are truncated"));
    }

    let mut out = File::create(dest)?;
    out.set_len(iso_size)?;
    out.write_all(&h2[0x10..0x90])?;
    for r in 0..raw_count {
        let e = &raw_table[r * RVZ_RAW_ENTRY..];
        let (offset, size) = (be64(e, 0), be64(e, 8));
        let (first, groups) = (be32(e, 16) as usize, be32(e, 20) as usize);
        let skipped = offset % chunk as u64;
        let end = offset
            .checked_add(size)
            .filter(|&end| end <= iso_size)
            .ok_or_else(|| bad("an RVZ data range lies outside the disc"))?;
        for g in 0..groups {
            // Group 0 is stored from the chunk boundary, so it repeats the bytes
            // before the data offset.
            let start = offset - skipped + (g * chunk) as u64;
            let len = ((start + chunk as u64).min(end) - start) as usize;
            let entry = &group_table[(first + g) * RVZ_GROUP_ENTRY..];
            let (at, stored, packed) =
                ((be32(entry, 0) as u64) << 2, be32(entry, 4), be32(entry, 8));
            let stored_len = (stored & 0x7FFF_FFFF) as usize;
            if stored_len == 0 {
                continue;
            }
            let mut data = read_at(&mut file, at, stored_len)?;
            if stored >> 31 == 1 {
                let want = if packed != 0 { packed as usize } else { len };
                data = decompress(compression, data, want)?;
            }
            if packed != 0 {
                data = depack(&data, len)?;
            }
            if data.len() != len {
                return Err(bad("an RVZ block has the wrong length"));
            }
            if !is_zero(&data) {
                out.seek(SeekFrom::Start(start))?;
                out.write_all(&data)?;
            }
            progress(start + len as u64, iso_size);
        }
    }
    Ok(())
}

fn rvz_pack(raw: &Path, dest: &Path, progress: Progress) -> Result<()> {
    let mut input = File::open(raw)?;
    let iso_size = input.metadata()?.len();
    let mut disc_header = [0u8; 0x80];
    input.read_exact(&mut disc_header)?;
    if iso_size <= 0x80 {
        return Err(bad("the image is too small"));
    }

    let mut out = BufWriter::new(File::create(dest)?);
    out.write_all(&vec![0u8; RVZ_HEAD1 + RVZ_HEAD2])?;
    let mut pos = (RVZ_HEAD1 + RVZ_HEAD2) as u64;

    let data_len = iso_size - 0x80;
    let groups = (0x80 + data_len).div_ceil(RVZ_CHUNK as u64) as usize;
    let mut table = Vec::with_capacity(groups * RVZ_GROUP_ENTRY);
    input.seek(SeekFrom::Start(0))?;
    let mut buf = vec![0u8; RVZ_CHUNK];
    for g in 0..groups {
        // Group 0 repeats the header bytes, as Dolphin writes it.
        let from = (g * RVZ_CHUNK) as u64;
        let len = ((((g + 1) * RVZ_CHUNK) as u64).min(iso_size) - from) as usize;
        input.read_exact(&mut buf[..len])?;
        let block = &buf[..len];
        let (offset, stored) = if is_zero(block) {
            (0u32, 0u32)
        } else {
            let packed = zstd::bulk::compress(block, 5).map_err(|e| bad(e.to_string()))?;
            let (bytes, flag) = if packed.len() < block.len() {
                (&packed[..], 1u32 << 31)
            } else {
                (block, 0)
            };
            out.write_all(bytes)?;
            let at = pos;
            pos += bytes.len() as u64;
            let pad = (4 - pos % 4) % 4;
            out.write_all(&[0u8; 3][..pad as usize])?;
            pos += pad;
            ((at >> 2) as u32, bytes.len() as u32 | flag)
        };
        table.extend_from_slice(&offset.to_be_bytes());
        table.extend_from_slice(&stored.to_be_bytes());
        table.extend_from_slice(&0u32.to_be_bytes());
        progress(from + len as u64, iso_size);
    }

    let mut raw_entry = Vec::new();
    raw_entry.extend_from_slice(&0x80u64.to_be_bytes());
    raw_entry.extend_from_slice(&data_len.to_be_bytes());
    raw_entry.extend_from_slice(&0u32.to_be_bytes());
    raw_entry.extend_from_slice(&(groups as u32).to_be_bytes());
    let raw_blob = zstd::bulk::compress(&raw_entry, 5).map_err(|e| bad(e.to_string()))?;
    let group_blob = zstd::bulk::compress(&table, 5).map_err(|e| bad(e.to_string()))?;
    let raw_at = pos;
    out.write_all(&raw_blob)?;
    let group_at = raw_at + raw_blob.len() as u64;
    out.write_all(&group_blob)?;
    let total = group_at + group_blob.len() as u64;

    let mut h2 = vec![0u8; RVZ_HEAD2];
    h2[0..4].copy_from_slice(&1u32.to_be_bytes());
    h2[4..8].copy_from_slice(&RVZ_ZSTD.to_be_bytes());
    h2[8..12].copy_from_slice(&5i32.to_be_bytes());
    h2[12..16].copy_from_slice(&(RVZ_CHUNK as u32).to_be_bytes());
    h2[16..0x90].copy_from_slice(&disc_header);
    // No partitions: count 0, entry size 48, SHA-1 of the empty list.
    h2[0x94..0x98].copy_from_slice(&48u32.to_be_bytes());
    h2[0x98..0xA0].copy_from_slice(&((RVZ_HEAD1 + RVZ_HEAD2) as u64).to_be_bytes());
    h2[0xA0..0xB4].copy_from_slice(&Sha1::digest([]));
    h2[0xB4..0xB8].copy_from_slice(&1u32.to_be_bytes());
    h2[0xB8..0xC0].copy_from_slice(&raw_at.to_be_bytes());
    h2[0xC0..0xC4].copy_from_slice(&(raw_blob.len() as u32).to_be_bytes());
    h2[0xC4..0xC8].copy_from_slice(&(groups as u32).to_be_bytes());
    h2[0xC8..0xD0].copy_from_slice(&group_at.to_be_bytes());
    h2[0xD0..0xD4].copy_from_slice(&(group_blob.len() as u32).to_be_bytes());

    let mut h1 = vec![0u8; RVZ_HEAD1];
    h1[0..4].copy_from_slice(b"RVZ\x01");
    h1[4..8].copy_from_slice(&0x0100_0000u32.to_be_bytes());
    h1[8..12].copy_from_slice(&0x0003_0000u32.to_be_bytes());
    h1[12..16].copy_from_slice(&(RVZ_HEAD2 as u32).to_be_bytes());
    h1[16..36].copy_from_slice(&Sha1::digest(&h2));
    h1[0x24..0x2C].copy_from_slice(&iso_size.to_be_bytes());
    h1[0x2C..0x34].copy_from_slice(&total.to_be_bytes());
    let head1_hash = Sha1::digest(&h1[..0x34]);
    h1[0x34..0x48].copy_from_slice(&head1_hash);

    let mut out = out.into_inner().map_err(|e| e.into_error())?;
    out.seek(SeekFrom::Start(0))?;
    out.write_all(&h1)?;
    out.write_all(&h2)?;
    Ok(())
}
