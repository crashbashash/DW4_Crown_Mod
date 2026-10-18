# Supported-release resource-option matrix

All three current changes use the same platform-independent balance/resource
pipeline: half-Speed practice enemies, four-player enemy counts, and custom
crown drops. PS2 ISO9660 and GameCube FST relocation are selected only at the
outer image layer. Both compressed GameCube paths pass the complete patch plan
through unpacking, patching, repacking and round-trip verification.

The opt-in `tests/resource_options.rs` matrix completed 24 full disc builds from
nine pristine local inputs:

- Every combination of the spawn/drop switches on PS2 USA, Europe and Japan,
  plus GameCube USA and Japan ISO/NKit images: 20 builds.
- Both switches enabled through USA and Japanese GameCube CISO and RVZ inputs:
  four further builds. Output container format is preserved.
- Every build verifies all 665 copies of each of the three transformed enemy
  blocks (1,995 blocks), including the shared practice-Speed fingerprint
  `c2cdbb0ca237b6619d55374bd4ed86e5`. It checks release identity and predicted raw
  output size; the patch engine additionally verifies the rebuilt archive.
- All nine input files retain their original MD5. Outputs are temporary.

The existing generator audits verify that the spawn switch affects counts/caps,
while the drop switch preserves stats, enemy rows and protected reward tables.
See [spawn scope](four-player-spawns.md) and [drop rates/exceptions](custom-drops.md).
These are full disc-creation and data-verification checks, not emulator gameplay
or regional playthrough claims.

## Both switches enabled

| Input | Output bytes | Output MD5 |
| --- | ---: | --- |
| PS2 USA | 2,184,189,952 | `923cdc2a1893624b9290ed9c196a3242` |
| PS2 Europe | 2,175,698,944 | `7012439c09b51c1fa42617dc1ec9ced0` |
| PS2 Japan | 2,184,130,560 | `a0f3ecf3dd3ed824ccae7f56b397dbbe` |
| GameCube USA ISO/NKit | 1,909,897,216 | `75585c608239ce909514181f23353a33` |
| GameCube Japan ISO/NKit | 1,910,441,984 | `e93c5e03c192cea020dd86fd6becdb5f` |
| GameCube USA CISO | 1,914,732,544 | `7b8e66ca7704f04488841b9a0d202e5c` |
| GameCube Japan CISO | 1,916,829,696 | `1ff2dc22f770365fe19452bcd647b4c9` |
| GameCube USA RVZ | 990,811,005 | `7a669fb4148b782b7dd8f81b7996b989` |
| GameCube Japan RVZ | 991,219,709 | `459a6d67304f5bc50d72801acbd58d08` |

Compressed sizes/hashes refer to the compressed output file. Each output is also
unpacked and its release/current-balance blocks checked independently of the
container-level round trip inside the patch engine.

## Reproduce

Set any of the nine `DW4_RETAIL_*` variables listed in
[the development guide](../DEVELOPMENT.md#testing-against-real-discs), then run:

```sh
cargo test --release --test resource_options -- --nocapture
```

Unconfigured inputs are skipped; the test prints the configured-input count.
No game bytes or personal paths are included in the fixture or this report.
