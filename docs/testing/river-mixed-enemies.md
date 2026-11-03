# Mixed Numenume River enemies

Randomized river banks use up to three compatible projectile-capable families.
Selection shuffles compatible imported families using the stage's seeded RNG,
then distributes the original Numemon generators evenly across the selected
families in shuffled order. A generator retains its selected family across
all difficulties. A two-family donor supply produces two families; a single
compatible donor still produces a single-family river. Numemon remains the
fallback when replacement or local reward-row allocation cannot complete.

The change affects NDMW model pack 628 and its generator plans. Other stages
retain their existing selection order and script behavior. Original encounter
stats, rewards, rarity, player counts, entrance checks and spawn counts retain
their existing rules. Every selected family receives the
[river-only ranged attack policy](river-ranged-ai.md), including MR05's wider
firing bands. Turret AI and boat damage scripts are not edited by this change.

## Cleanup and freeze

The five bank-transition scripts previously erased every instance of the one
replacement family. They now issue the same native `ENEMY / SQ_ERAS` command
once for each family actually assigned to river banks. The bomb script issues
`MODELCTRL / ONFREEZE` once for each selected family. These are the six existing
reviewed scripts: `matb03`, `matb06`, `matb08`, `matb09`, `matb12` and `matf00`.
All commands retain the native all-instance selector, timing, conditions and
surrounding control flow. Generator starts and scenery commands remain native.

The Lua editor accepts only the reviewed native command shapes. Added calls
relocate existing branches so an original skip still bypasses the complete
cleanup/freeze block. Single-family assignments keep the original constant-only
rename path. Original pack IDs and memory blocks remain intact; replacement
script blocks are appended through the existing pack builder.

Reward-row allocation remains atomic across the whole river family group and
all difficulties. Scripts are rewritten after that allocation succeeds or falls
back, so they cannot refer to a discarded assignment. Actor capacity sums all
generators assigned to each model and takes the maximum over difficulties.
Unused speculative imports are removed before the game loads the pack.

## Validation

The complete application checker and devtools Clippy pass. The real-disc
regression passes on PS2 USA, Europe and Japan and GameCube USA and Japan.
Each release exercises three normal seeds, a two-donor supply, eight individual
ranged donors and an incompatible melee-only supply. It checks final families
across all difficulty generators, every reviewed cleanup/freeze target and
sufficient shared actor capacity. A retained native Numemon pool keeps its
authored capacity. The native data checks are separate from gameplay coverage.

Synthetic Lua tests verify two- and three-family commands, relocated skips and
rejection of unexpected command shapes or families. The reviewed entrance/script
regression also passes for PS2 USA and GameCube USA.

### Dolphin

All cases cold-boot GameCube USA with a copied native Hard memory card. No old
savestate is loaded. Each verifies the running AI choices and entry guards,
observes all selected families active together and producing independently
moving native projectiles, and checks movement and allocator invariants after
the native stage exit.

| Image | Families | Runtime coverage |
| --- | --- | --- |
| Two-family focused image | Sukamon, MR05 | Native bomb freeze and Abyss exit |
| Three-family focused image | Sukamon, MR05, Infermon | All five bank transitions and Cliff exit |
| Normal app pipeline, seed 67, Crown Mod + random enemies | Imperialdramon, Infermon, Raremon | All five bank transitions and Cliff exit |

The focused three-family observation spans more than 3,000 video fields; the
other two span at least 1,800. Every family fires a moving projectile without
an observed melee sequence. MR05's loaded range maps retain the widened river
bands. The normal pipeline image verifies all output bytes and records zero
approximate or undrawn effects.

Test helpers protect player and real Lua boat health during observation.
Positioning and objective flags assist bank/exit coverage. Projectile positions
and attack cursors remain native. These checks establish firing, script scope,
rendering and lifecycle behavior rather than unassisted river balance. The
Abyss exit helper waits for a controllable player after the loading screen;
an early diagnostic sample taken during loading is not treated as a failure
of the stage unload.

Private captures, native snapshots, image-verification reports and logs are
recorded under `outputs/river-variety/` in the outer workspace.

### PCSX2

The same three image cases pass on PS2 USA from copied native Hard cards,
without loading old savestates. All selected families are active together and
fire independently moving native projectiles. The running AI choice tables,
offensive-entry guards and MR05 range bands match the written image data.
Each observation spans 1,806 video fields without an observed melee sequence.

All three PS2 cases exercise all five bank cleanup/start transitions. The
two-family case additionally exercises the native bomb freeze and returns to
Abyss; the focused and normal-pipeline three-family cases reach Cliff. Each
checkpoint and completed unload retains valid allocator free lists in both PS2
arenas, with bounded, non-overlapping free blocks. Player movement works after
each exit. Captures verify the imported models and projectiles render.

The normal PS2 image also uses the complete app pipeline with Crown Mod and
random enemies enabled at seed 67. Its output bytes verify successfully, with
zero approximate or undrawn effects. It uses Imperialdramon, Infermon and
Raremon, matching the normal GameCube case. Health, positioning and progression
assistance have the same limits described above; these are not unassisted
balance tests.
