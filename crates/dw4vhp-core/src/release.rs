//! Verified PS2 and GameCube retail releases. Only offsets, identities and fingerprints are
//! stored here; table bytes always come from the player's own disc.
use crate::error::{Error, Result};
use crate::iso9660;
use crate::layout::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Usa,
    Europe,
    Japan,
    GcUsa,
    GcJapan,
}

impl Release {
    /// The PS2 releases, which are detected by their boot configuration.
    pub const ALL: [Self; 3] = [Self::Usa, Self::Europe, Self::Japan];
    /// The GameCube releases, which are detected by their disc game ID.
    pub const GAMECUBE: [Self; 2] = [Self::GcUsa, Self::GcJapan];

    pub fn is_gamecube(self) -> bool {
        Self::GAMECUBE.contains(&self)
    }

    pub fn serial(self) -> &'static str {
        match self {
            Self::Usa => "SLUS_208.36",
            Self::Europe => "SLES_534.05",
            Self::Japan => "SLPS_254.53",
            Self::GcUsa => "GDJE",
            Self::GcJapan => "GDJJ",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Usa => "Digimon World 4 (USA)",
            Self::Europe => "Digimon World 4 (Europe)",
            Self::Japan => "Digimon World X (Japan)",
            Self::GcUsa => "Digimon World 4 (USA, GameCube)",
            Self::GcJapan => "Digimon World X (Japan, GameCube)",
        }
    }

    pub fn layout(self) -> Layout {
        let (hp_off, stat_off, byte_off) = match self {
            Self::Usa => (0x78D64C5, 0x78DACB9, 0x78DA51D),
            Self::Europe => (0x783DCC9, 0x78439A9, 0x784320D),
            Self::Japan => (0x78A7549, 0x78ABD3D, 0x78AB5A1),
            Self::GcUsa => (0x77B737D, 0x77B737D + 0x47F4, 0x77B737D + 0x4058),
            Self::GcJapan => (0x780BAD9, 0x780BAD9 + 0x47F4, 0x780BAD9 + 0x4058),
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

impl Release {
    /// The block offsets inside this particular image. PS2 offsets are fixed by
    /// the retail layout. A GameCube image can place its files anywhere, since
    /// NKit and RVZ conversions move them, so the offsets follow `area.afs`,
    /// where the tables sit at a fixed distance from the file's start.
    pub fn layout_in(self, disc: &[u8]) -> Result<Layout> {
        let mut layout = self.layout();
        if !self.is_gamecube() {
            return Ok(layout);
        }
        let (retail_start, shift) = match self {
            Self::GcJapan => (5_327_464u64, 0x72F_7071u64),
            _ => (5_327_720u64, 0x72A_2815u64),
        };
        let start = crate::gcm::find_file(disc, "area.afs")?.offset;
        // `layout()` holds the offsets for a disc whose files sit at retail_start.
        let delta = start as i64 - retail_start as i64;
        debug_assert_eq!(layout.hp_off, retail_start + shift);
        for field in [
            &mut layout.hp_off,
            &mut layout.stat_off,
            &mut layout.byte_off,
        ] {
            *field = field.checked_add_signed(delta).ok_or(Error::NotThisDisc)?;
        }
        Ok(layout)
    }
}

/// Identify the release from a GameCube disc's game ID, or from a PS2 disc's
/// boot configuration and matching root ELF.
/// Ambiguous, renamed and incomplete images are refused before table lookup.
pub fn detect(iso: &[u8]) -> Result<Release> {
    if let Ok(id) = crate::gcm::game_id(iso) {
        let release = Release::GAMECUBE
            .into_iter()
            .find(|release| id.starts_with(release.serial().as_bytes()))
            .ok_or(Error::NotThisDisc)?;
        crate::gcm::find_file(iso, "ndmw.afs")?;
        return Ok(release);
    }
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
