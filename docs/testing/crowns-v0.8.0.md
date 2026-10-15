# v0.8.0 crown expansion

The desktop application's default patch now expands crown display resources in
all supported PS2 releases: USA SLUS_208.36, Europe SLES_534.05 and Japan SLPS_254.53.
The base enemy-table transform remains the same as v0.7.0.

## Implementation

The four-crown limit is authored resource capacity, rather than a single draw-loop
constant. Each of 85 NDMW.AFS model packs contains four model RAM slots, four
animation RAM slots, four matching scene roots, and a four-bit switch mask.
All four capacities are expanded together to 64. Each new root receives private
mutable child/override lists and each animation slot private appearance/actor
lists; immutable billboard geometry remains shared. The crown switch receives a
private two-word mask because its original mask may also belong to other switches.

The largest authored sum of enemy pool slots found across these packs is 53.
64 covers that inventory plus four players. This is an inventory bound, not a
claim that the engine supports 64 simultaneous enemy spawns. The stress test is
ten enemies, as requested. Spawn counts and scripts in the output ISO are unchanged.

The Rust parser reads the pack's own class descriptors, object remapping table
and memory directory. It checks class/count/extent consistency and the linked
crown graph, then reparses the result and verifies original IDs, original memory,
unrelated objects, new reference bounds/kinds, and model/animation node pairing.
No Python, reflection database, proprietary blobs, or regional executable offsets
are shipped. Explicit-layout generated-fixture APIs continue to patch tables
without requiring a retail archive; the retail/application entry point adds crowns.

The rebuild recipe retains references into the read-only input mapping for bulk
data. Only inserted resource data is retained across packs. AFS entry order,
untouched allocations, trailing content and filename-table pointers are preserved.
The writer appends the rebuilt archive to the copied ISO, updates the archive's
both-endian directory extent/size and the volume sizes, and checks every byte of
the new archive before the atomic output rename. No separate full-size archive
temporary file is needed. Destination-space checks and the UI use the final ISO size.

## Offline and native application checks

All three clean-disc integration tests pass against independently generated Python
prototype outputs. The complete crown archives are byte-identical to that prototype.
The application's ISO writer preserves the terminator's reserved bytes; the
prototype wrote volume size there too, accounting for the reference checksum
normalization. Existing executable and archive bytes are retained in the original
part of the image. Only the 1,995 stat-table ranges and ISO extent/volume metadata
change there, with the rebuilt archive appended.

| Region | Final bytes | Final MD5 |
| --- | ---: | --- |
| USA | 2,179,143,680 | `9ba8432d2237330a687d4ac1a0203d68` |
| Europe | 2,170,652,672 | `0dfd365ae96ef0ab30e326b5096e0f46` |
| Japan | 2,179,084,288 | `4a6815c3c7e5e4ee7f793b04b82eda52` |

The real Linux Tauri application, with the opt-in agent-control feature, created
and verified each output through its rendered UI. All three passed invalid-disc
refusal, original-image preservation, checksum and exact output size, overwrite
cancellation, confirmed overwrite, same-input/output refusal and already-modded
refusal. No `.part` file remained after success. Writer failure-injection tests
also confirm that interrupted appends and failed verification preserve an existing
output and remove the partial copy. Frontend tests distinguish input size from
expanded output size.

Core tests, strict Clippy and formatting checks, frontend tests/lint/build and
native Tauri compilation/Clippy pass. Windows native execution and physical PS2
hardware are outside this validation.

## Runtime checks

Verified on 2026-10-15 in the private PCSX2 build, controlling the game through
PINE. Each disc was created by the application, cold-booted, and loaded from its
corrected regional level-999 World 1 memory-card fixture. The game entered the
hub without replaying the intro, displayed the World 1 gate option, and loaded
Goblin Pass. No earlier savestate was loaded.

| Region | Model / animation / scene / mask capacity | Active enemies / crowns | After movement |
| --- | --- | --- | --- |
| USA | 64 / 64 / 64 / 64 | 10 / 10 | 10 / 10 |
| Europe | 64 / 64 / 64 / 64 | 10 / 10 | 10 / 10 |
| Japan | 64 / 64 / 64 / 64 | 10 / 10 | 10 / 10 |

For each region the first Goburimon generator's count and maximum were changed
from 2 to 10 **in RAM only**, before walking across the bridge to the first
spawns. The ISO retains the original generator values. Savestate memory inspection
confirmed ten distinct enemy crown IDs 0–9, ten active model slots, ten non-null
animation works, visible child bits 0–9, and 64 unique roots matching the model
and animation lists. These checks passed again after movement. GS captures also
show the expanded crowns in game. The fresh-stage state, encounter state and
post-movement state are retained per region as slots 01, 02 and 03.

The final production build, without agent control, was separately launch-tested
and created its native window successfully. No physical PS2 or Windows runtime
claim is made by these emulator and Linux application checks.

Private reports, screenshots, generated test cards and savestates live outside
this repository in `../Decomp/local/testing/veryhard-v0.8.0-20261015/`.
Cleanup removed 21 generated ISO artifacts (28,280,520,844 bytes), including the
v0.7.0 regional tests, crown prototypes and v0.8.0 application tests. All three
clean source-disc MD5s were rechecked unchanged. `cleanup-isos.json` retains the
removal audit; reports, screenshots, cards and states are preserved. Reference
mod images and other projects' images were retained. No game data is included
in this repository.
