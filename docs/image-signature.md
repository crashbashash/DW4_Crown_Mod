# Crown Mod image signature (format 1)

New images contain a checksummed metadata record **inside the logical disc image**.
The record is appended after the rebuilt resources, outside NDMW.AFS and every
executable. No executable NOPs, boot code, original data or archive-relative
resource offsets are overwritten. PS2 volume lengths include the extra sector;
GameCube file extents keep their original rebuilt sizes. This adds one 2048-byte
sector plus, for GameCube, up to 2047 bytes of alignment padding.

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
`crownColourSeed`, multipliers and the other low-level PatchPlan settings.

Readers can check the final sector first. To allow zero padding introduced by
GameCube container conversion, inspect sector boundaries within the final
2 MiB plus one sector. CISO/RVZ readers must use logical, decompressed disc
bytes rather than the physical compressed file's end. The core APIs
`signature::read_raw`, `signature::from_image` and `signature::decode` implement
these rules. DigiAtlas uses the same format without depending on the patcher.

## Detection and older images

Crown Mod checks the marker before reading/scanning enemy tables and reports
an already-modded image with its version. An absent marker means **unknown**, not
clean: the existing game-data/crown scan remains the fallback for older builds.
A recognized but corrupt/unsupported record is refused rather than treated as a
clean image. Old builds are not retrospectively assigned a version.

The write is included in the normal temporary-output verification and final
checksum before atomic installation. The low-level explicit-layout fixture seam
continues to omit resource expansion and signatures; application/retail patching
always emits the marker.

## Validation

- Identity/options and malformed-record unit tests, including all five release IDs.
- A signature-only fixture refuses before table parsing; unmarked modded fixtures
  retain the legacy-scan refusal.
- Generated CISO and RVZ round trips retain the record and its options.
- Full-disc PS2 regression tests retain the pre-signature golden hash after
  removing only the footer and reversing the volume-size update in a hashing view.
- DigiAtlas imports a freshly produced signed USA image and labels the version and
  random crowns from its metadata.

Historical release checksums in the changelog describe the builds released then.
Newly signed outputs have different whole-image checksums.

All five supported releases also passed the [signed-image emulator load check](testing/signature-boots.md).
