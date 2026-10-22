# Random enemies

The randomizer swaps ordinary generators between enemy families while matching
HP, attack and crown rarity. Imported models also carry their resident sound
selectors, embedded banks and action bindings. Sound filename indices are local
to each stage and are remapped when a bank is added. Shared banks are resolved
from the model's actual action bindings rather than guessed from its model name.

Generators with an authored spawn sequence keep their original enemy and sequence
on every difficulty. These include the cannon Numemon in Goblin Pass (`SQ_WAIT`)
and MR01 falling onto the train (`SQ_FALL`). Their entrance motion belongs to the
original model's animation/controller; clearing the sequence or assigning another
model loses that motion. Other generators in those stages can still randomize.
Bosses, script-dependent generators and kill-count restrictions also apply.

These changes use randomizer rules version 4. The same seed is reproducible within
a rules version; older images need rebuilding to receive these fixes.

## Audio and scripted encounter audit

The data audit covers all 20 importable enemy families in 204 native stage/model
combinations on each of PS2 USA and GameCube USA. It checks every original action
binding, including damage/death labels, selector cue numbers and parameters,
translated filename indexes, registered embedded bank payloads and repeat imports.
Bank bytes match the retail sources. Projectile/effect models are checked too;
their own bindings must be present even when they only use shared sound banks.

The audit exposed missing shared-bank projectile bindings for Mummymon, MR02 and
Imperialdramon, plus missing retail selectors in one MarineDevimon and one MR04
pack. Imports now carry those projectile bindings and supplement missing selectors
from a complete native source, without guessing cue numbers or changing the
original sound choice. Conflicting embedded banks are refused. MarineDevimon’s
retail `otDEAD01` reference resolves to Otamamon’s bank.

Gekomon is explicitly checked for `NATK01 -> geNATK01` (song), `DEAD01 -> geDEAD01`,
`LDMG01 -> geLDMG01`, `SDMG01/MDMG01 -> geSDMG01`, and the shared
`MAGC01 -> tgVOICE` binding. This verifies configuration, not audible playback or
the gameplay effect of its song; those still need a listening/gameplay test.

All 73 candidate stage packs are scanned for explicit generator sequences across
all difficulties, and for model names controlled by stage Lua. Supported intro
rewrites remain eligible; other model-specific waits, freeze, removal and damage
scripts retain their original enemy families. Model-owned callbacks are excluded
from this stage-level restriction. The timed quest in entry 449 now trades only
among its native counter-aware models, like the two existing kill-count floors.

The regression checks 2,019 protected generator rows per platform, including 615
custom enemy-entrance rows (205 distinct generators across three difficulties).
It covers `SQ_WAIT`, `SQ_FALL`, `SQ_JUMP`, `SQ_JUMP1`, `SQ_ENTRHS`, `SQ_NENTR`,
`SQ_ATCK1` and `SQ_SWON`. Native hit/summon script bytes remain intact, and 2,922
ordinary rows still change in the zero-extra-donor regression. The full default
randomizer reports 69 stages with changes; wholly scripted stages stay authored.

## Shared enemy capacity

A stage has one instance pool per enemy model, shared by every generator that
uses that model. The randomizer sums the largest authored party maximum of those
generators, taking the maximum over the three difficulty packs. Native models
also retain their original pool as headroom for scripts and summons when other
families are assigned to them. Existing generator counts and maxima are unchanged.

Each added slot receives private model RAM, actor/animation RAM, animation
combiners and their bone/state buffers, ground-collision RAM, scene roots and
mutable lists. Nonempty per-instance shadow lists grow with them. Meshes, textures, skeleton definitions and animation clips remain
shared. Model/animation/combiner/collision lists, scene children and the switch visibility mask grow
together; masks support more than 32 or 64 slots. Crown inventory grows to cover
the resulting enemy pools plus four players, with a minimum of 64.

Identical directory descriptors are reused when adding instances. The pack’s
remap entries contain 16-bit directory indexes; object references are 32-bit.
This avoids exhausting the descriptor table when expanding a large stage.
Every expansion reparses the graph and verifies translated instance references,
original IDs, original memory, untouched records and model/animation pairing.
Unknown private instance layouts are refused rather than partially cloned.

The seed 1520 GameCube Goblin Pass regression checks all three difficulty packs,
the imported Tyrannomon pool growing beyond four slots, unchanged generator
maxima and sufficient crown inventory. The family audit exercises PS2 and
GameCube model graphs, disjoint mutable state, repeated expansion, masks spanning
multiple words and an object count beyond 65535 with valid directory indexes.

## Data checks

With `DW4_RETAIL_USA_ISO` pointing at a clean PS2 USA image:

```sh
cargo test --lib imported_gekomon -- --nocapture
cargo test --lib audit_every_enemy_audio -- --nocapture
cargo test --lib every_scripted_spawn -- --nocapture
cargo test --lib retail_instance_pools -- --nocapture
cargo test --lib seed_1520_gamecube -- --nocapture
cargo test --lib cannon_and_train -- --nocapture
cargo test --test random randomised_image_is_deterministic_and_verifies -- --nocapture
```

The song regression also accepts `DW4_RETAIL_GC_USA_ISO` for a plain/NKit GameCube
USA image. It checks `geNATK01`, its action binding, the destination filename
index and byte-identical embedded bank data. It also checks existing sound records
and repeat imports. The entrance regression covers all three difficulty packs and
confirms ordinary generators still change. Every copied graph is reparsed and
verified during patching, including references, class layouts and memory data.

The release test `every_release_builds_and_records_its_plan` accepts
`DW4_UPDATE_PS2`, `DW4_UPDATE_PS2_EUROPE`, `DW4_UPDATE_PS2_JAPAN`,
`DW4_UPDATE_GC` and `DW4_UPDATE_GC_JAPAN`. It builds and verifies a signed output for
each configured image; unconfigured images are skipped.

The PS2 USA and GameCube USA full-image checks passed with seed 1234, including
CISO output for GameCube. The song regression passed on both platforms, and the
entrance regression passed on PS2 across all three difficulties.

These are disc-data checks. Audible song playback and the full cannon/train
encounters still require gameplay verification after loading the rebuilt stage.

## GameCube runtime check (shared-capacity build, 2026-10-22)

This earlier rules-3 runtime check predates the rules-4 audio/script audit and
does not verify audible playback.

Seed 1520 full images built and verified on PS2 USA and GameCube USA with
four-player spawn counts. The GameCube image was
cold-booted in Dolphin and loaded from a memory-card fixture. Goblin Pass loaded
19 Tyrannomon model slots, 19 animation slots, 19 animation combiners with private
buffers, 19 ground-collision slots, and 145 crown slots.

Activating the first two bridge groups produced eight simultaneous Tyrannomon,
eight active animation slots/combiners and 13 total crowns. All eight remained
active after movement and continued attacking. Player position and health were
adjusted for the test; enemy counts, model resources and spawn state were read
from the rebuilt disc without RAM edits. The fixture starts on Normal difficulty;
the other difficulty packs are covered by the data regression, not a live claim.
No physical GameCube validation is claimed.

An old hub savestate can restore the original cached archive index as well as
stage resources. Verification must cold-boot the rebuilt image and load a memory
card; reloading a stage after loading an old savestate is insufficient.
