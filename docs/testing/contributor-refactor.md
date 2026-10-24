# Contributor refactor validation

This is a structural refactor of the published `v0.13.0` source at `37c688f`.
The application version remains `0.13.0` and enemy randomizer rules remain `8`.

## Ordinary checks

`npm run check` passed through the complete sequence:

- Generated patch-selection TypeScript and encounter documentation match Rust.
- Engine formatting, Clippy with warnings denied, and workspace tests pass.
- Frontend type checking, ESLint, 78 tests and production build pass.
- Desktop formatting, Clippy with warnings denied, tests and build pass.
- The optional `devtools` feature also compiles and passes Clippy.

The normal checker reported zero configured retail-image inputs. The real-disc
comparisons and native UI checks below were run separately; ordinary green tests
do not imply these audits ran. Synthetic IGB fixtures now exercise parsing,
class addition, shared-memory growth, null padding, graph-reference remapping,
original-memory preservation and refusal of malformed references/edits.

## Entire-image comparisons

Reference binaries were built before editing the engine. Both reference and
refactored binaries patched the same read-only inputs, with explicit enemy and
crown-colour seeds. The entire resulting files were compared, including archive
contents, image signatures, recovery data and padding.

| Input | Options | Enemy seed | Crown-colour seed | Output bytes | Result |
| --- | --- | --- | --- | --- | --- |
| GameCube USA NKit ISO | Crown Mod, 4-player spawns, Crown Drops, random colours, random enemies | 1520 | `0x12345678` | 2,002,644,992 | Byte-for-byte identical |
| PS2 USA ISO | Random enemies alone | 1278675365 | `0x12345678` (option disabled) | 2,276,296,704 | Byte-for-byte identical |

Whole-output SHA-256 values:

- GameCube: `577d9f00dcf4814fcdf34b5b7e92bfc25e378752ef2933bdd34d696cc1e08d59`
- PS2: `07bb4bc35eefa237d9af19a3947585c71b5aea7925092fdf41a619db59fda3e8`

## Native desktop workflow

The repaired `tools/e2e.py` ran against the real Tauri app with a read-only
private copy of the PS2 USA input. It verified:

1. Empty options leave patching disabled.
2. Invalid input produces a dismissible refusal.
3. Explicitly enabling Crown Mod produces the reference image.
4. Cancelling overwrite preserves the output.
5. Confirming overwrite reproduces the image.
6. Selecting the input as output is refused and preserves the input.
7. Cancelling an existing-image update preserves the image.
8. Confirming an in-place update matches the fresh build and preserves the input.

For this Crown-Mod-only UI case, the output was 2,179,444,736 bytes with MD5
`9527d77a65fd4411598761dfa5468a26`. These are version-specific reference values;
regenerate them when image metadata or intended patch behaviour changes.

This evidence covers disc bytes and desktop workflows. Emulator and physical
console coverage is documented separately in the relevant feature reports.
