//! Verified PS2 retail releases. Only offsets, identities and fingerprints are
//! stored here; table bytes always come from the player's own disc.
use crate::error::{Error, Result};
use crate::iso9660;
use crate::layout::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Usa,
    Europe,
    Japan,
}

impl Release {
    pub const ALL: [Self; 3] = [Self::Usa, Self::Europe, Self::Japan];

    pub fn serial(self) -> &'static str {
        match self {
            Self::Usa => "SLUS_208.36",
            Self::Europe => "SLES_534.05",
            Self::Japan => "SLPS_254.53",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Usa => "Digimon World 4 (USA)",
            Self::Europe => "Digimon World 4 (Europe)",
            Self::Japan => "Digimon World X (Japan)",
        }
    }

    pub fn layout(self) -> Layout {
        let (hp_off, stat_off, byte_off) = match self {
            Self::Usa => (0x78D64C5, 0x78DACB9, 0x78DA51D),
            Self::Europe => (0x783DCC9, 0x78439A9, 0x784320D),
            Self::Japan => (0x78A7549, 0x78ABD3D, 0x78AB5A1),
        };
        Layout {
            hp_off,
            stat_off,
            byte_off,
            rows: 649,
            expected_copies: 665,
        }
    }
}

/// Identify the release from its boot configuration and matching root ELF.
/// Ambiguous, renamed and incomplete images are refused before table lookup.
pub fn detect(iso: &[u8]) -> Result<Release> {
    let cnf = iso9660::find_file(iso, "SYSTEM.CNF")?;
    let start = cnf.lba as usize * 2048;
    let end = start
        .checked_add(cnf.size as usize)
        .ok_or(Error::NotThisDisc)?;
    let config = std::str::from_utf8(iso.get(start..end).ok_or(Error::NotThisDisc)?)
        .map_err(|_| Error::NotThisDisc)?;
    let mut boot = config.lines().filter_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "BOOT2").then_some(value.trim())
    });
    let target = boot.next().ok_or(Error::NotThisDisc)?;
    if boot.next().is_some() {
        return Err(Error::NotThisDisc);
    }
    for release in Release::ALL {
        if target != format!("cdrom0:\\{};1", release.serial()) {
            continue;
        }
        let elf = iso9660::find_file(iso, release.serial())?;
        let start = elf.lba as usize * 2048;
        let end = start
            .checked_add(elf.size as usize)
            .ok_or(Error::NotThisDisc)?;
        let bytes = iso.get(start..end).ok_or(Error::NotThisDisc)?;
        if !bytes.starts_with(b"\x7fELF") {
            return Err(Error::NotThisDisc);
        }
        return Ok(release);
    }
    Err(Error::NotThisDisc)
}
