# Changelog

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
