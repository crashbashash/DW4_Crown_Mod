# Changelog

## v0.15.0

- Fixed a GameCube crash that could happen when travelling to another area after fighting Apokarimon.
- Randomized enemies now match the strength and EXP rewards of the enemies they replace. This prevents late-game enemies from becoming unfairly powerful in earlier areas.
- More scripted encounters can now be randomized, including the Minotaurmon and Raremon introductions, Numenume River and Dry Land ruins.
- More enemies with special entrances can now change, while keeping entrances that need a specific enemy working correctly.
- Added enemy randomization to the practice stage while keeping the lessons, progression and practice settings working as before.

## v0.14.0

- Improve PS2 and GameCube memory handling to prevent the reproduced randomized-stage loading crashes, including Mangrove Forest.
- Increase enemy variety with up to four imported enemy families per stage, up from three.
- Randomize the Sukamon and Tortamon introductions while preserving their scripted progression.
- Randomize Mangrove Forest's two pre-boss enemy waves while preserving boss progression and quest completion.

## v0.13.2

- Fixed a stage-resource leak that could cause loading freezes after repeated area changes, including Mangrove Forest.

## v0.13.1

- Fixed randomised Minotaurmon retreating to heal below 50% HP and getting stuck against walls. They now continue fighting.

## v0.13.0

- Make Crown Mod optional, allowing Random enemies and other independent options to run with the original game balance.
- Make randomized enemies award the EXP of the enemies they replace, preserving each difficulty's rewards and applying Crown Mod's EXP scaling only when enabled. Replacement combat strength is matched separately.

## v0.12.4

- Fix MR02's puddle/blast effects and MR03's missile mouth fire when imported into other areas, preserving their original textures and blending.
- Correct imported Imperialdramon's height, moving its model and collision volumes together while retaining terrain following.
- Restore random enemies in Mangrove Forest while preserving its scripted boss/add groups and Tortamon introduction.
- Expand floating EXP rewards to five digits, displaying up to 99,999 without changing awarded EXP.
- Preserve the source image extension in suggested output filenames, and automatically add it when omitted in Save As or a typed output path.

## v0.12.3

- Register imported enemy sound banks in the stage's static resource list so the audio loader can find them, including death sounds.
- Grow enemy instance and scene lists in their original backing blocks, avoiding orphaned references when unloading a randomized stage.
- Use null references in generated padding blocks, preventing resource-pointer corruption when object ID 0 is a memory block, including Cliff Dungeon with seed 1520.
- Omit donor candidates unused by every difficulty's generators, reducing loaded resources without changing enemy assignments.

## v0.12.2

- Fix simultaneous spawn limits for randomized enemies, including Tyrannomon in Goblin Pass.
- Fix missing enemy and projectile sounds, including hit/death sounds and Gekomon’s song.
- Preserve scripted entrances. Cannon Numemon and falling MR01 on the Dry Lands train retain their original enemy types.
- Protect encounters that rely on specific enemies for cutscenes, cleanup, freezing or other scripted actions.
- Preserve timed-quest and kill-count progression when randomizing enemies.
- Update enemy randomizer rules to version 4. Existing seeds may produce different enemy placements.

## v0.12.1

- The suggested output file name now includes the Random enemies seed, e.g. `[CrownMod] [Seed 1234]`.

## v0.12.0

- Added an experimental Random enemies option for PS2 and GameCube. Regular enemies in every
  stage are swapped for other enemies with matching stats. Bosses stay as they are.

## v0.11.2

- The four-player spawn setting now spawns the Kokatorimon in the practice stage.

## v0.11.1

- Added the ability to update existing modded games.
- General UI fixes and improvements.

## v0.11.0

- Halve Better Loot plug drops to 1% / 2% / 3% on Normal / Hard / Very Hard.
  Other item-category rates stay unchanged; the difference becomes no drop.
- Crown Numenume River missile turrets and multiply their missile damage to the
  shared boat by three. Position crowns close to the turret barrels; other river
  hits retain their normal damage.
- Include river turret crowns in the random crown colours option.

## v0.10.0

- Added optional four-player enemy spawn counts for smaller parties.
- Added an optional custom loot rebalance, with rewards suited to the area and difficulty.
- Added optional random crown colours for enemy spawns. Bosses keep yellow.
- Halved practice-stage enemy Speed.
- Added automatic update checks with release notes.

## v0.9.0

- Support Digimon World 4 (USA) and Digimon World X (Japan) on GameCube, with the same
  enemy stats and 64 yellow crowns as PS2. Plain `.iso`, NKit `.iso`, `.ciso` and
  `.rvz` (uncompressed or zstd) images are accepted, and `.ciso` and `.rvz` inputs
  produce the same format. RVZ junk padding is written back as zeros.
- The GameCube rebuilt crown archive is appended to the image and its file table
  entry repointed.
- Fix the Browse and Save buttons freezing the window without showing a dialog on
  Linux, and make Save as a real save dialog. Linux now uses the desktop's own file
  picker through xdg-desktop-portal (KDE, GNOME).
- Show a "Patching complete" popup when a patch is written and verified.

## v0.8.0

- Expand simultaneous crown capacity from four to 64 in all 85 crown-bearing
  model packs on USA, European and Japanese PS2 discs. Model/animation RAM,
  private mutable scene nodes and the visibility mask are rebuilt together.
- Integrate the rebuild into the native application without Python or external
  game metadata. Enemy stats retain the v0.7.0 transform; spawn counts are unchanged.
- Stream the rebuilt archive onto the output copy, update its ISO extent, and
  verify the complete archive and enemy tables before replacing the destination.
- Show the exact larger output size in the free-space hint.
- Add expanded-output regression tests and interrupted-write coverage.

| PS2 release | Output bytes | Very Hard Plus ISO MD5 |
| --- | ---: | --- |
| USA | 2,179,143,680 | `9ba8432d2237330a687d4ac1a0203d68` |
| Europe | 2,170,652,672 | `0dfd365ae96ef0ab30e326b5096e0f46` |
| Japan | 2,179,084,288 | `4a6815c3c7e5e4ee7f793b04b82eda52` |

All three regions passed native application disc creation, PCSX2 cold boot,
World 1 gate entry, and ten simultaneous crowns persisting after movement.

Start the new disc from a cold boot and load a memory-card save: old emulator
savestates can restore the previous four-slot resources. See
[validation details](docs/testing/crowns-v0.8.0.md).

## v0.7.0

- Support the USA (`SLUS_208.36`), European (`SLES_534.05`) and Japanese
  (`SLPS_254.53`, Digimon World X) PS2 releases with automatic region detection.
- Validate the boot configuration, regional table offsets, original table
  fingerprints and all 665 copies of each table before patching.
- Preserve each release's boot serial, languages, video mode and save identity.
- Clear the selected native disc after a failed analysis, preventing a previous
  valid selection from remaining available to patch.
- Add generated regional detection tests and optional full-disc regression tests.
- Update desktop copy, package descriptions and UI automation for all regions.
- Exclude generated Rust build files from frontend linting.

Full-disc regression checks passed against local clean images. Each patched
output matches a reference checksum independently computed using the shipped
practice-buff mod's table bytes. Every byte outside the 1,995 table ranges is
unchanged, source images are unchanged, and already-patched inputs are refused.

| PS2 release | Clean ISO MD5 | Very Hard Plus ISO MD5 |
| --- | --- | --- |
| USA | `3185f04230b1dabd853db750e4b51108` | `df426b9ff17c9e8218779f8ff1ecf524` |
| Europe | `bd95f00a8efa2ab6b502b7aade37f143` | `f7f04d92915e7dc8b650d3d9faa1e55f` |
| Japan | `78543a26a08d82fb32f1f3b5035d7e3b` | `5352edd1f061be9c94c9eeb07807023a` |

The default USA output is unchanged from v0.6.0. The optional low-level
`force_very_hard` ELF edit remains USA-only and is refused for Europe/Japan;
the desktop application's base mode does not use it.

Follow-up verification on 2026-10-15 passed native application disc creation and
PCSX2 cold boot, World 1 gate entry, Goblin Pass loading, and complete live-table
equality for all three regions. See [the runtime validation report](docs/testing/regional-v0.7.0.md)
for evidence, test-save corrections and scope.
