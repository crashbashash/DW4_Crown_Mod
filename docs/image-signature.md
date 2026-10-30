# Crown Mod image signature (format 1)

New images contain a checksummed metadata record **inside the logical disc image**.
The record is appended after the rebuilt resources, outside NDMW.AFS and every
executable. No executable NOPs, boot code, original data or archive-relative
resource offsets are overwritten. PS2 volume lengths include the appended metadata;
GameCube file extents keep their original rebuilt sizes. New outputs also include
a compressed change manifest before the signature, plus alignment padding.

The marker identifies the mod version, base platform/release and complete patch
options, including the resolved random-colour seed. Renaming/copying the image
keeps it. CISO/RVZ conversion preserves it in the logical disc data. This is an
identification record with corruption detection, not cryptographic publisher
authentication or a guarantee that nobody edited the disc afterwards.

## Layout

The record starts at a 2048-byte-aligned offset near the end of the logical image.
Integers are little-endian; the block is exactly 2048 bytes:

| Offset | Length | Meaning |
| --- | --- | --- |
| 0 | 32 | ASCII `DW4_CROWN_MOD_SIGNATURE` followed by nine zero bytes |
| 32 | 4 | Format version, currently 1 |
| 36 | 4 | UTF-8 JSON payload length, 1–1984 bytes |
| 40 | 20 | SHA-1 of the JSON payload |
| 60 | 4 | Reserved, zero |
| 64 | payload length | JSON metadata |
| remainder | variable | Zero padding |

Required JSON identity fields are `formatVersion: 1`, `id: "dw4-crown-mod"`,
`version`, `platform` (`ps2` or `gc`), `release` (boot serial/game ID) and `options`.
The option keys include `betterLoot`, `fourPlayerSpawns`, `randomCrownColours`,
`crownColourSeed`, `randomEnemies`, `enemySeed`, multipliers and the other low-level PatchPlan settings.

### Enemy randomiser record

An image built with `randomEnemies` also carries a `randomEnemies` object. The options hold what is
needed to rebuild the image (`enemySeed` is the resolved seed, drawn at patch time when none was
given); the object holds what the build did:

| Key | Meaning |
| --- | --- |
| `rulesVersion` | Bumped whenever a change alters what a seed produces. A seed only reproduces an image with the same version. |
| `seed`, `donorsPerStage` | The settings the plan ran with. |
| `stages` | Stages whose enemies were randomised. |
| `generators` | Spawn generators given a new enemy model. |
| `keptOriginal` | Generators that ended up on their original model, because nothing else fit. |
| `modelsCopied` | Enemy models copied from one stage pack into another. |
| `approximateEffects` | Bullets and effects hung in the nearest matching node of a differently shaped effect tree. |
| `undrawnEffects` | Bullets and effects that could not be hung at all (expected 0). |
| `rewardVariants` | Area-local replacement variants preserving each original spawn’s combat stats and EXP, with bounded elemental resistance differences. |
| `planDigest` | First 8 bytes of a SHA-1 over every decision, hex. Equal seed, rules version and mod version give equal digests. |

All five releases support the randomiser. The three PS2 discs share game data and therefore a plan
for a given seed; the two GameCube discs share another. The GameCube data is slightly larger, so a
few donors overflow the 16-bit object index there and another model is chosen: the same seed gives
close to, but not exactly, the same game on the two platforms, and their `planDigest` values differ.

Readers can check the final sector first. To allow zero padding introduced by
GameCube container conversion, inspect sector boundaries within the final
2 MiB plus one sector. CISO/RVZ readers must use logical, decompressed disc
bytes rather than the physical compressed file's end. The core APIs
`signature::read_raw`, `signature::from_image` and `signature::decode` implement
these rules. DigiAtlas uses the same format without depending on the patcher.

## Detection and older images

Crown Mod checks the marker before reading/scanning enemy tables and offers
to update an already-modded image with its recorded version and options. An absent marker means **unknown**, not
clean: the existing game-data/crown scan remains the fallback for older builds.
A recognized but corrupt/unsupported record is refused rather than treated as a
clean image. Old builds are not retrospectively assigned a version.

## Change manifest and in-place updates

New outputs carry a `recovery` descriptor in the JSON signature. It records
`formatVersion: 1`, `offset`, compressed `length`, `sha1`, `originalSize`,
`regionCount`, and `archiveEnd` (the end of the rebuilt archive before metadata).
The compressed bytes are outside every game file. The descriptor's SHA-1 checks
the compressed data before decoding, with a 128 MiB decompressed limit.

The Zstandard payload is little-endian:

| Length | Meaning |
| --- | --- |
| 8 | ASCII `DW4UNDO1` |
| 8 | Original logical image size |
| 20 | SHA-1 of the entire original logical image |
| 4 | Number of overwritten regions |
| repeated | 8-byte original offset, 4-byte length, original bytes |

The manifest captures every table copy, filesystem pointer and volume length,
and any executable hooks, difficulty or serial writes. Unchanged game data stays
in the image; appended resources are removed by truncating to the original size.
It contains the player's original bytes, never a bundled retail-data template.
This records exactly where bytes were overwritten; the signature's `options`
records the selected features and their settings.

Selecting a modded image shows its disc details and loads its saved optional
features into Settings. The primary action changes from Patch ISO to Update ISO,
which opens an in-place update confirmation. The player can enable or disable
each feature, including when already using the current version. Updating reconstructs a private clean base, checks its original
SHA-1 and normal retail validation, and applies the current patch from scratch.
This prevents compounded stats, leftover hooks, and repeated archive growth.
Keeping random crowns preserves the recorded seed. After normal output checks
and any container round trip, the updater checks the selected file has not
changed and replaces it at the same path. Temporary files are cleaned on failure.
The selected file remains intact until the final rename.

Older signed or scan-detected images lack original-byte manifests. Their first
update asks for the player's original disc, validates that it is a clean copy of
the same platform/release, and rebuilds from that. It then replaces the selected
modded image in place and includes a manifest for future updates. Older unknown
feature settings start disabled in the dialog; no version is invented.

The write is included in the normal temporary-output verification and final
checksum before atomic installation. The low-level explicit-layout fixture seam
continues to omit resource expansion and signatures; application/retail patching
always emits the marker.

## Validation

- Identity/options and malformed-record unit tests, including all five release IDs.
- A signature-only fixture refuses before table parsing; unmarked modded fixtures
  retain the legacy-scan refusal.
- Generated CISO and RVZ round trips retain the record and its options.
- Full-disc PS2 regression tests check unsigned default-build golden hashes after
  removing the appended metadata and reversing the volume-size update in a hashing view.
  The padding-reference correction intentionally updates these hashes: reversing
  only the 85 corrected padding words reproduces each region's previous golden hash.
- DigiAtlas imports a freshly produced signed USA image and labels the version and
  random crowns from its metadata.

Historical release checksums in the changelog describe the builds released then.
Newly signed outputs have different whole-image checksums.

All five supported releases also passed the [signed-image emulator load check](testing/signature-boots.md).


### In-place update regression results (2026-10-19)

- All five supported releases (PS2 USA, Europe and Japan; GameCube USA and Japan):
  add all optional features, repeat the same
  update, remove all features, refuse an older build without an original while
  preserving its checksum, then migrate signed and unsigned legacy images using
  the matching original. Recorded random-colour seeds are retained. Damaged
  recovered originals are refused without replacing the selected image. Every
  completed update matched a fresh build byte for byte, and original disc
  checksums remained unchanged.
- GameCube USA CISO and RVZ: remove all features in place, preserve the container,
  verify its decompressed round trip and compare to a fresh build. Update
  progress remained monotonic and finished at 10000/10000.
- PS2 USA retains the independent pre-signature golden checksum after excluding
  metadata and reversing volume-length changes.
- Original-byte recovery unit tests reject altered manifests and unexpected edits
  to the recovered base. Frontend tests cover recorded options and cancellation.

Run the opt-in retail checks with `DW4_UPDATE_PS2`, `DW4_UPDATE_PS2_EUROPE`,
`DW4_UPDATE_PS2_JAPAN`, `DW4_UPDATE_GC`, `DW4_UPDATE_GC_JAPAN` and/or
`DW4_UPDATE_COMPRESSED` pointing to the player's original images:

```sh
cargo test -p dw4vhp-core --release --test update -- --nocapture
```

The update matrix also verifies monotonic progress, including incremental
recovery preparation before copying. The full configured run passed all six
cases (five regional images and the compressed pipeline).

The native desktop check also loaded recorded feature selections, cancelled an
update without changing the file, applied selected features in place, and
compared that image with a fresh build through the same UI. The files matched
byte for byte and the original disc was unchanged. A fresh patch visibly
advanced through 1–9% while preparing recovery metadata, before copying.
