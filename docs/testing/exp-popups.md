# Five-digit EXP popups

The base patch expands `expnm0` in all 85 stage model packs on every supported
PS2 and GameCube release. The display cap rises from 9,999 to 99,999. EXP awards,
level progression, damage/heal/bit displays and Lua scripts are unchanged.

Retail's shared display helper already accepts five-digit values and caps them
at 99,999. Its EXP sequence table deliberately uses `SQ_EXPNM04` for both four
and five digits. The patch retains that animation and extends the number to the
left using the spacing of the last two authored digits. The EXP label moves
left by one digit width so it cannot cover the leading digit. Smaller numbers
retain their original digit geometry and gain a small gap after the label;
unused digits use the number controller's existing hiding behavior.

Every EXP number instance gains a private fifth digit and six corner records.
Its triangle count grows from 8 to 10 and its vertex count from 24 to 30.
Position, optional normal and UV streams grow together. Existing list and vertex
memory block IDs stay fixed; original data remains at the start of each block.
The format parser verifies the result during patching.

The source-disc regression accepts `DW4_RETAIL_USA_ISO`,
`DW4_RETAIL_EUROPE_ISO`, `DW4_RETAIL_JAPAN_ISO`,
`DW4_RETAIL_GC_USA_ISO` and `DW4_RETAIL_GC_JAPAN_ISO`:

```sh
cargo test --release --lib crown::exp -- --nocapture
```

It passed all 85 packs on each of the five releases (425 packs). It checks
original digit references, private new corner indices, mesh/stream counts,
preserved backing-block IDs and original stream data. Every original object is
compared against its expected edits, including unrelated number displays,
scripts, textures and animation sequences. A generated-coordinate test checks
fifth-digit spacing and rejects truncated or nonfinite position data. The audit
also verifies the label position buffers against the intended translation.

The PS2 full-disc regression's unsigned fingerprints include this expansion:

| Release | Unsigned archive end | Unsigned image MD5 |
| --- | ---: | --- |
| USA | 2,179,418,112 | `25215189ba7f252e15beb9dd3ea27ced` |
| Europe | 2,170,929,152 | `e2c59ac2b1e98f266ae97d4cdf9a8a4e` |
| Japan | 2,179,346,432 | `df9b8f3a15fde1eade95f19fe7ca01f2` |

These hashes exclude the recovery footer and reverse only its volume-length
metadata, as in `tests/retail.rs`. They were independently calculated from the
completed base-patch images after the five-release resource audit.

## Runtime check

A final GameCube USA base-patch image was cold booted in Dolphin and entered
Goblin Pass from a copied memory-card fixture. All six loaded EXP controllers
reported five digits and a 99,999 cap. Captures confirmed `EXP10000` and
`EXP99999`, and the 9,999 check showed the unused fifth digit hidden. The label
remained clear of the leading digit.

These are controlled display-boundary checks: a temporary RAM-only replacement
of the native `MDLdspExp` wrapper supplied each display value. Enemy HP and player
position were adjusted to reproduce a kill; EXP award calculations were left
intact. The shipped patch contains only archive resource changes. This is
GameCube emulator validation; PS2 releases have the data audit and full-image
checks, and no physical-console result is claimed.

Updating an earlier signed base-patch test image through its embedded original
data produced a byte-identical result to the final fresh GameCube USA build
(`106eab1f5100eae919d360448ca6b679`). The final PS2 USA full-disc regression also
passed, including all 1,995 enemy table blocks, unchanged original archive and
executable bytes, and the refreshed unsigned fingerprint.

Existing modded images must be updated through the app, or rebuilt from a clean
source, to receive the expanded resources. Runtime checks must boot the rebuilt
disc and load its stage; old savestates can restore the four-digit resources.
