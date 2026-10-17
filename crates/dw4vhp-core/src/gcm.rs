//! The minimum GameCube disc filesystem the patcher needs: identify the game
//! from the disc header and find a file in the FST (file system table).
//!
//! All GameCube disc fields are big-endian, and file offsets are plain byte
//! offsets into the disc. A file entry is 12 bytes: a type byte and 24-bit name
//! offset, then the file's disc offset and its length. The FST is only
//! read here; the patcher edits entries in place through [`GcmFile`].

use crate::error::{Error, Result};

const DVD_MAGIC: [u8; 4] = [0xC2, 0x33, 0x9F, 0x3D];
const MAGIC_OFFSET: usize = 0x1C;
const FST_OFFSET_FIELD: usize = 0x424;
const FST_SIZE_FIELD: usize = 0x428;
const ENTRY_LEN: usize = 12;

/// A file found in the FST, with the disc offsets of its own entry fields so a
/// patch can repoint it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcmFile {
    pub offset: u64,
    pub size: u64,
    /// Where the 4-byte big-endian file offset is stored in the image.
    pub offset_field: u64,
    /// Where the 4-byte big-endian file length is stored in the image.
    pub size_field: u64,
}

fn be_u32(bytes: &[u8], at: usize) -> Result<u32> {
    let word = bytes.get(at..at + 4).ok_or(Error::NotThisDisc)?;
    Ok(u32::from_be_bytes(word.try_into().unwrap()))
}

/// The 6-character game ID (for example `GDJEB2`) of a GameCube image, or
/// [`Error::NotThisDisc`] when the header lacks the GameCube magic word.
pub fn game_id(disc: &[u8]) -> Result<&[u8]> {
    if disc.get(MAGIC_OFFSET..MAGIC_OFFSET + 4) != Some(&DVD_MAGIC[..]) {
        return Err(Error::NotThisDisc);
    }
    disc.get(..6).ok_or(Error::NotThisDisc)
}

/// Looks `name` up anywhere in the FST, ignoring ASCII case, and returns the
/// file's location. Directories are skipped.
pub fn find_file(disc: &[u8], name: &str) -> Result<GcmFile> {
    game_id(disc)?;
    let fst = be_u32(disc, FST_OFFSET_FIELD)? as usize;
    let fst_size = be_u32(disc, FST_SIZE_FIELD)? as usize;
    let table = disc
        .get(fst..fst.checked_add(fst_size).ok_or(Error::NotThisDisc)?)
        .ok_or(Error::NotThisDisc)?;
    let count = be_u32(table, 8)? as usize;
    let names = count.checked_mul(ENTRY_LEN).ok_or(Error::NotThisDisc)?;
    if count == 0 || names > table.len() {
        return Err(Error::NotThisDisc);
    }
    for index in 1..count {
        let at = index * ENTRY_LEN;
        if table[at] != 0 {
            continue;
        }
        let name_at = u32::from_be_bytes([0, table[at + 1], table[at + 2], table[at + 3]]) as usize;
        let raw = table.get(names + name_at..).ok_or(Error::NotThisDisc)?;
        let end = raw.iter().position(|&b| b == 0).ok_or(Error::NotThisDisc)?;
        if raw[..end].eq_ignore_ascii_case(name.as_bytes()) {
            return Ok(GcmFile {
                offset: be_u32(table, at + 4)? as u64,
                size: be_u32(table, at + 8)? as u64,
                offset_field: (fst + at + 4) as u64,
                size_field: (fst + at + 8) as u64,
            });
        }
    }
    Err(Error::FileNotFound {
        name: name.to_string(),
    })
}
