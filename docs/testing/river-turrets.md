# Numenume River missile turrets

The default mod crowns the river's eight `g_numepodz01` turrets and multiplies
only their missile hits on the shared boat by three. A damaging, unguarded hit costs
15 / 12 / 9 / 6 boat HP with 1 / 2 / 3 / 4 actual players. The boat still starts
at 100 HP. Numemon contact and other attack IDs retain their original damage.
Guarding, failed hit checks and zero calculated damage still prevent damage.
The four-player spawn option does not change this actual-party-size rule.

The native missile hitbox's attack ID is validated as 70108 before patching.
The shared player hit script substitutes the threefold amount only for that ID.
Boat health and meter values clamp at zero. The boat's damage animation changes
when a hit crosses into a health band, and the failure branch accepts health
below its threshold, so multiplayer hits cannot skip game over by overshooting.

Turrets retain their original scripted durability and hit handling. A crown is
generated in the expanded `i_rareicon` pool after each authored turret spawn call.
Slots 56-63 are reserved for these eight crowns; the patch checks that the river's
enemy pools plus four player slots cannot reach that range. Native billboard
geometry and colour animations are reused. The crown is positioned 5 world
units above its turret and closes on both ordinary and charged destruction.
Ignored hits and nonlethal hits retain the crown. Stage unloading clears the
stage's model resources.

Crowns are yellow by default. With Random crown colours enabled, each turret
generation advances a private 16-bit Lua generator and chooses ranks 1-5.
It uses the patch's colour seed, persists in a separate global and makes no calls
to the game's RNG or math libraries. Disabling the option does not advance it.
Colour choice does not alter damage, durability, drops or encounter counts.
Both option settings emit identically sized scripts, preserving output-size
prediction during disc analysis.

Only the river model pack is affected. The patch finds its controllers and Lua
objects by authored names, validates instruction patterns and rewrites constants
and branches from the player's disc. All original object IDs and memory blocks
remain intact. Four Lua data objects are repointed to appended replacement
chunks; unrelated original records are verified unchanged. No game bytecode or
assets are stored in the repository.

## Validation

`cargo test --lib river` checks independent scalar execution of the emitted
generator over 10,000 generations, every boat HP from 1-100 across all four
party sizes and missile/non-missile IDs, and branch relocation around after-call
hooks. Synthetic fixtures contain test-owned instructions only.

Set `DW4_RIVER_PACK` to a clean river model-pack extract to enable the optional
real-pack preservation test. Set `DW4_RIVER_AUDIT_DIR` to an output directory
outside the repository to export both colour variants and their modified Lua
scripts. With a reference Lua 4.0.1 interpreter, run:

```sh
lua -e 'river_audit_root="<audit-directory>"; dofile("tools/verify-river.lua")'
```

That executes the complete modified scripts with mocked game API calls. It
checks all eight spawn slots, exact colour sequences, inactive-trigger behaviour,
nonlethal and lethal turret hits, charged and ignored hits, guarded/missed/zero
damage, all party sizes, unrelated attack IDs and lethal boat-health overshoot.
Native game rendering, projectile collision and scene cleanup require a separate
emulator check. A live GameCube USA comparison confirmed the initial +60 world
height placed the crown near the screen's top edge. Lowering it to +5 places it
beside the barrels, close to the turret. The selected random colour remains
visible. The same paused scene was restored between captures; the live session
retains the 3× missile multiplier. Later turret placements and destruction still
have reference-Lua coverage rather than native visual coverage.

The real river packs from PS2 USA, Europe and Japan and GameCube USA and Japan
passed both colour variants, preservation checks and reference-Lua execution.
Native status-icon code reads the model's world translation and generates the
same `i_rareicon` billboard used here. All eight inserted positions preserve the
authored turret X/Z and use turret Y + 5. Both colour variants verify this against
the original turret spawn calls in the reference-Lua harness.

Before the multiplier was reduced from 10× to 3×, the full-image resource-option matrix passed all five option combinations on
each of the five raw releases (25 outputs). Every output retained the expected
1,995 enemy-table blocks, exact output-size predictions and unchanged input
images. The current 3× build with the +5 crown placement has these three-region PS2 default
output fingerprints:

| Release | Output bytes | MD5 |
| --- | ---: | --- |
| PS2 USA | 2,179,162,112 | `dd1c73561a824c1a2512f9c3f4078e38` |
| PS2 Europe | 2,170,671,104 | `e6abfe8b81f2234a88228776e93f0186` |
| PS2 Japan | 2,179,102,720 | `f2cefb65e40ea022487f009a783c96e9` |

Before that multiplier change, USA GameCube RVZ round trips also passed with spawn counts and custom drops
enabled, both with yellow and random turret crowns. Each packed output was
unpacked and checked against the complete patched image. With seed `0x72abcdef`,
the random-colour RVZ is 990,857,858 bytes, MD5
`632a659bf019cea97094d70251e288bc`.
