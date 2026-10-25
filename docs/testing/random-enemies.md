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

The current randomizer uses rules version 10. The same seed is reproducible within
a rules version; older images need rebuilding to receive these fixes.

Rules 10 preserves the backing blocks of imported Lua script registries and the
initial crown instance, animation, scene-child and visibility lists. The IGB
loader resolves reference-bearing memory even when no list owns it. Replacing a
list's backing block therefore left its original references alive after unload.
The Lua registry now rewrites and grows its original block, preserving authored
script order and imported callback insertions. Crown lists and their existing
visibility mask grow in place. Verification permits only the planned Lua block
rewrite and block growth; other original memory remains byte-identical.

The ownership regression exercises GekoGeko Land and Mangrove Forest with seed
`2162128323` on PS2 USA/Europe/Japan and GameCube USA/Japan. It requires the
original crown/list/mask IDs and backing IDs, preserved script entries and order,
and no unowned object-reference blocks retaining objects. The reported seed's
complete PS2 USA build preserves its enemy-plan digest, all 2,003 generator
decisions and 1,473 reward variants. An independent full-archive audit finds no
orphaned object references in the corrected image, versus 15,881 previously.

```sh
cargo test --release --lib expanded_crowns_and_imported_scripts_keep_backing_blocks -- --nocapture
```

Rules 9 gives every generator assigned Minotaurmon (`e_minotaru`) the native
`SQ_NENTR` entrance. It keeps the half-HP switch on the normal combat branch,
bypassing the retreat to a stage-specific healing position that can strand an
imported enemy against a wall. This covers both imported and resident models,
all difficulties and crown rarities, including private EXP variants. Authored
entrances and protected encounters retain their original setup. Other enemy
choices, combat rows and rewards are unchanged from rules 8.

The Minotaurmon regression checks PS2 USA/Europe/Japan and GameCube USA/Japan.
It exercises a resident model in `dng205b1` and an import into Abyss of Grief,
verifies 180 selected generator rows across the three difficulties, follows
private EXP rows back to their combat family and original reward, and checks
the alternate entrance's normal-combat label. Reapplying the generator edits
is byte-identical; other entrance fields and protected generator records are
preserved. This is disc-data validation; the change has not been tested live.
The existing PS2 USA/GameCube USA scripted-encounter audit also passes, and
the PS2 USA full-disc regression confirms same-seed reproducibility, a distinct
plan for another seed, and byte-for-byte verification of the written image.

Rules 8 separates combat matching from EXP. A replacement still uses the closest
HP/attack variant of the same authored crown rarity. Its EXP comes from the
original generator's row on the selected difficulty, after the normal Crown Mod
transform, including configured multipliers, exclusions and caps. This preserves
the reward the original encounter would have supplied; it can raise or lower the
replacement's native reward. Party sharing and player EXP bonuses remain native.

Each localized AREA pack receives private variants in otherwise unfilled enemy
rows. A variant copies the replacement's HP, combat stats, chargen and MODEL
binding, then takes only EXP from the original spawn. Original occupied rows are
preserved. Distinct (combat row, original row) pairs get distinct variants, so two
spawns of the same model can pay different rewards. All three difficulty packs
share the area's allocation; original generator rows are excluded from the free
pool. If the pool cannot hold a generator's variants across all difficulties,
that generator keeps its original enemy and reward. Model capacity planning uses
the combat families rather than the private row numbers.

Full-block overrides merge with the existing stat writes. MODEL reference writes
are included in byte-for-byte output verification and recovery, so updating an
older signed image restores the original database before rebuilding. The data
regression resolves all seven localized copies on all five supported releases
and checks two simultaneous MR02 variants paying 146 and 496 while its original
row remains at 58. These checks exercise the default transformed rewards rather
than just changing the floating display.

Rules 7 preserves the native texture bindings and complete render-attribute
ancestry of imported particles and projectiles. The old child-index match could
render MR02's blast/puddle particles or MR03's mouth fire using an unrelated texture or blend
state. See [the particle regression](particle-effects.md). Existing images need
rebuilding; the extra render resources can affect donor fit at the object limit.

Rules 7 also narrows Mangrove Forest's cutscene guard. Its `mate00.lua` wait and
`tim_g_orbz03.lua` cleanup address Otamamon/Gekomon boss adds whose generators
are already pinned (`GENE-131`, `GENE-139`, `GENE-s01..s05`). Those groups and
their native model pools remain intact; unrelated patrols can now randomize.
Tortamon's separate introduction and authored spawn sequences remain protected.
The seed `1278675365` regression changes 480 ordinary rows across the three
difficulties on each of the five releases, preserves 12 protected groups and
checks every original stage script byte-for-byte. It reproduces the original
whole-area exclusion before the fix. This is a data regression; the complete
boss encounter has not been replayed live with the corrected patrols.

Rules 6 lowers imported Imperialdramon models and their collision volumes by 13
units without changing rules-5 generator choices. Native Imperialdramon encounters
retain their authored data. See [the height correction](imperialdramon-height.md).

Rules 5 corrects resource ownership and bank registration without changing
rules-4 enemy placements.
Embedded `beCriSfpData` banks belong in the data list of the stage's registered
`beStaticInfo`. Previously imported banks were appended directly to the root
`igInfoList`, which made them reachable in a graph audit but omitted them from
the static resources used by the audio loader. The audio regressions now require
the actual static-data registration and preserve native bank entries and root
registrations. The GameCube regression failed before the fix; the corrected
audit passes all 204 source combinations on each of PS2 USA and GameCube USA.
This is static validation; audible playback remains untested.

Static inspection of the USA GameCube executable confirms the lookup path:
`0x80314f14`, message case `0x14`, requests `STATICINFO / GETINFOLIST`, searches
each returned info's data list at offset `0x14` for the bank filename, and adds
matches to the sound loader. `0x802f6aa8` transfers those matched `beCriSfpData`
banks. A bank referenced only by the pack root is absent from that search.

## Audio and scripted encounter audit

The data audit covers all 20 importable enemy families in 204 native stage/model
combinations on each of PS2 USA and GameCube USA. It checks every original action
binding, including damage/death labels, selector cue numbers and parameters,
translated filename indexes, static-data registration, embedded bank payloads
and repeat imports.
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

The regression checks 1,509 protected generator rows per platform, including 615
custom enemy-entrance rows (205 distinct generators across three difficulties).
It covers `SQ_WAIT`, `SQ_FALL`, `SQ_JUMP`, `SQ_JUMP1`, `SQ_ENTRHS`, `SQ_NENTR`,
`SQ_ATCK1` and `SQ_SWON`. Native hit/summon script bytes remain intact, and 3,402
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

Instance arrays and scene child lists grow in their existing memory blocks.
Replacing a list's backing block left its old references in the IGB directory;
in the GameCube seed-1520 checkpoint, the expanded Goburimon pool survived
unloading while unexpanded imported families were freed. The regression now
requires the original backing-block IDs for model, action, animation-combiner,
collision and shadow pools, and for existing scene child lists. Original mask
objects and their backing blocks are also preserved as visibility capacity grows.

Candidate donors are still chosen before generator assignments. Afterwards the
pack is rebuilt without candidates unused across all three difficulty packs.
This avoids allocating unused models and sound banks while preserving the seed's
generator choices. The seed-1520 Humid Cave check keeps both MR04 generators and
the Infermon generator, their full authored pools and the native Numemon model,
and omits the unused MR03 import. Both floors' generator-pack bytes match rules 4.

A private GameCube seed-1520 build was cold booted with random crowns, random
enemies and four-player spawns, then taken through World 1, Humid Cave B1, B2,
B1 again and back to World 1 using the normal entrance triggers. All transitions
completed without an exception. Arena usage was approximately 7.38, 5.09, 6.24,
5.34 and 8.06 MB respectively. Returning to World 1 previously failed at about
11.22 MB. The remaining 0.68 MB increase over the initial World 1 visit has not
yet been distinguished from caches or position-dependent allocations; repeated
routes and other dungeons still need validation. The supplied public-release
savestate was preserved and was not used for this cold-boot test.

The subsequent Cliff Dungeon B1 load exposed a separate padding bug. Its sound
pack (AFS entry 194) places the root list's memory at ID 0. Three unowned padding
blocks contained zero, which the loader resolves as a reference to ID 0 even
without a list owning the block. Object-reference counting then incremented the
root list's second word, corrupting the sound-info pointer. A memory write
watchpoint caught the increment at `0x8006a5fc`, before the eventual resource-list
fault at `0x802f9f88`. Padding now contains the null ID `0xFFFFFFFF`. Replaying the
same checkpoint with only those three padding words corrected successfully
loaded Cliff Dungeon. A regression constructs padding using the retail pack's
object-reference memory template and requires the null value.

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

## Original-spawn EXP validation (rules 8)

Seed `1278675365` full images built and passed output verification on PS2 USA
and GameCube USA with random enemies, random crowns, Crown Drops and four-player
spawn counts. An independent disc audit checked 72,072 spawn rewards per platform
across 243 difficulty packs and 567 localized tables, including 42,042 randomized
spawn checks. Each replacement's HP, non-EXP stats and chargen matched its selected
combat variant. Every originally occupied row remained unchanged relative to the
normal Crown Mod table. Recovery covered all changed original bytes on both
images, including the additional MODEL bindings.

A fresh GameCube boot loaded the memory-card fixture and entered Goblin Pass
through the normal gate. Its first randomized MR02 group used a private row with
HP 1,300 and EXP 146, preserving the original Goburimon reward rather than MR02's
native 58. Two kills increased the player's actual EXP by 146 each (292 total).
For this payout check, active enemies' current HP was reduced to 1 and the player
was made invulnerable; reward tables and the payout code were unchanged.

The same seed also built and passed verification on PS2 USA and GameCube USA
with Crown Mod disabled and Random enemies selected alone. The independent audit
again checked 72,072 rewards per platform and the replacement combat variants
across all difficulties and localized copies. Rewards matched the original
unscaled spawn rows, and every originally occupied row retained native balance.

## PS2 Mangrove loading regression (rules 10)

The PS2 USA seed `2162128323` build was tested in PCSX2 with Crown Mod disabled,
random enemies and four-player spawn counts, Normal difficulty and one joined player.
The corrected image was cold-booted from a memory-card fixture; its opening
dialogue was completed before the route began. No earlier-build savestate was
loaded, since it would restore cached archive metadata and resources.

The route completed Humid Cave and Blossomon, Cliff Dungeon and Mammothmon,
then Goblin Fortress and Mecha Rogue after activating the room console. It used
the first two dungeons' native end gates, rescued Leomon, completed the Commander
Room Digi-Elf/Ophanimon dialogue and left through that room's gate. From the hub
it entered Venom Jungle/GekoGeko Land, then Mangrove Forest. Player positions
were adjusted to native triggers; bosses' current HP was set to zero before
controller attacks finished them. Health was restored at the native Heal Circle.
Boss-clear flags and stage arrivals were read from saved EE RAM.

The earlier rules-9 image faulted on the fifth Geko-to-Mangrove return after
this history. Replaying that final native boat prompt reproduced the guest TLB
read fault at PC `0x002d891c`, invalid address `0x3c1a805d`. The private emulator
then asserted; the original report described a loading freeze with PCSX2 open.

Rules 10 completed the same dungeon history and 30 further Geko–Mangrove round
trips (60 stage loads) without a failed load or the earlier fault. The table
compares Mangrove arrivals after each round, excluding the initial arrival.

| Build | Successful returns | First arrival | Last arrival | Growth per round |
| --- | ---: | ---: | ---: | ---: |
| Rules 9 | 4; fifth faulted | 15.762 MiB | 16.384 MiB | about 212 KiB |
| Rules 10 | 30 | 14.431 MiB | 14.613 MiB | about 6.4 KiB |
| Unmodified retail control | 10 | 11.307 MiB | 11.308 MiB | about 0.14 KiB |

These values sum tracked game-pool counters; they are neither total physical
RAM nor loading-time allocation peaks. Growth is fitted after the first round.
The earlier fault did not recur in this run, while a smaller drift remains.

The unmodified USA disc control cold-booted the same memory-card fixture,
completed its opening dialogue, healed and entered Venom Jungle/Mangrove
directly. It completed ten round trips without a failed load and stayed nearly
flat. It omitted the dungeon history and the four-player spawn patch, so this
control does not isolate expanded capacity from randomized resources. The
remaining 6.4 KiB growth cannot be dismissed as ordinary retail background drift;
further allocation tracing is needed to identify it.

The core checks, resource-ownership regressions on all five supported releases,
and full PS2 USA/Europe/Japan default-build verification passed. Independent
comparison of the reported seed's old and corrected images found identical AREA
data, all 243 generator packs, 7,640 Lua bytecodes, 98 Lua registry lookup orders
and model-pool counts in all 164 model packs. The correction changes resource
ownership without changing the seed's encounters or rewards.
