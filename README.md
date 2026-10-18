# DW4 Crown Mod

[![CI Linux](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/ci-linux.yml?label=CI%20Linux&branch=main)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/ci-linux.yml)
[![CI Windows](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/ci-windows.yml?label=CI%20Windows&branch=main)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/ci-windows.yml)
[![Release Linux](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/release-linux.yml?label=Release%20Linux)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/release-linux.yml)
[![Release Windows](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/release-windows.yml?label=Release%20Windows)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/release-windows.yml)

**Crown Mod**, formerly **DW4 Very Hard Plus**, is a difficulty mod for **Digimon World 4** on PS2 and GameCube. It makes enemies much
tougher, and marks the strongest ones with a yellow crown so you can spot them.

You don't need any modding knowledge. Open the app, pick your own copy of the game,
and click one button. It creates a new, modded copy of the game for you.

## Quick start

1. [Download the app](#download) for Windows or Linux.
2. Open it and choose your game disc image (`.iso`, or `.ciso` / `.rvz` for GameCube).
3. Click **Patch ISO**.
4. Play the new patched file in your emulator. A `.ciso` or `.rvz` stays in the same format.

Your original file is never changed.

## What the mod does

- **Tougher enemies.** Enemy health and other stats are boosted. Attack values are
  already near the game's maximum, so most of the extra difficulty comes from health
  and the other stat boosts.
- **Yellow crowns.** The strongest enemy variants wear a yellow crown. The game
  normally shows crowns on only 4 characters at once. This mod raises that to 64, so
  every crowned enemy shows its crown, not just the first few. Enemy spawn counts are
  unchanged.

## What you need

- A clean copy of Digimon World 4, as an `.iso` file (PS2 or GameCube), or a GameCube `.ciso` or `.rvz`. These releases work:

  | Release | Game ID |
  | --- | --- |
  | USA | `SLUS_208.36` |
  | Europe (English, German, Spanish, Italian) | `SLES_534.05` |
  | Japan (*Digimon World X*) | `SLPS_254.53` |
  | GameCube USA | `GDJE` |
  | GameCube Japan (*Digimon World X*) | `GDJJ` |

  GameCube images can be a plain `.iso`, an NKit `.iso`, a `.ciso` or an `.rvz` (uncompressed or zstd). The Europe GameCube release is not supported yet.

  The app detects your release automatically. It refuses discs that are unknown or
  already modified.
- Windows or Linux. Nothing else to install.
- About **2.2 GB** of free disk space for the new file (more for `.ciso` and `.rvz`, which are unpacked while patching). The app shows the exact amount.

## Download

Get the latest version from the **Releases** page of this project:

- **Windows:** `.msi` installer or a plain `.zip`
- **Linux:** `.AppImage`, `.deb`, or a plain download

The downloads are unsigned, so Windows may show a warning the first time you run it.

## Contributing and building from source

See the [development guide](docs/DEVELOPMENT.md) for build steps, tests and project
layout. Release notes and checksums are in the [changelog](CHANGELOG.md).

This repository never contains game data. No disc images or extracted game files are
committed, and the app reads everything it needs from your own disc.

## Licence

GPL-3.0-or-later. See `LICENSE`.
