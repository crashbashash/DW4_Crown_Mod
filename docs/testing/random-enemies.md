# Random enemies

The randomizer swaps ordinary generators between enemy families while preserving
the original encounter’s combat stats, speed, status rates/resistances and rewards.
Fire, ice, thunder and dark resistances follow the replacement within ±100 points
of the original encounter after selected Crown Mod settings. Imported models also carry their resident sound
selectors, embedded banks and action bindings. Sound filename indices are local
to each stage and are remapped when a bank is added. Shared banks are resolved
from the model's actual action bindings rather than guessed from its model name.

Generators with a model-specific spawn sequence keep their original enemy and
sequence on every difficulty. Normal `SQ_NENTR` and `SQ_SWON` overrides may
randomize only when the replacement implements that same named sequence on
every difficulty. The override is preserved; Minotaurmon uses its reviewed
`SQ_NENTR` bypass. These include the cannon Numemon in Goblin Pass (`SQ_WAIT`)
and MR01 falling onto the train (`SQ_FALL`). Their entrance motion belongs to the
original model's animation/controller; clearing the sequence or assigning another
model loses that motion. Other generators in those stages can still randomize.
Bosses, script-dependent generators and kill-count restrictions also apply.

The current randomizer uses rules version 15. The same seed is reproducible within
a rules version; older images need rebuilding to receive these fixes.

Rules 15 relaxes reviewed non-boss protections:

- Normal entrance/activation overrides may randomize only to models implementing
  the same named sequence on every difficulty; cannon waits, train falls,
  jumps, Pharaohmon attack entrances and Otamamon’s special entrance stay native.
- The Minotaurmon (`dng204b2`) and Raremon (`dng411b3`) tutorial introductions
  retarget their named waits, control commands and cleanup. Intro replacement
  families are reserved from ordinary patrols. SkullGreymon and MR05 remain native.
- Numenume River’s six erase/freeze scripts follow one exclusive replacement for
  all Numemon generators. Dry Land ruins’ two erase scripts follow separate,
  exclusive replacements for its Mummymon and Tyrannomon groups. All generators
  and difficulty tiers in a family commit together; insufficient local table
  slots retain the entire native group and its original script targets.
- Boss protections and the unsupported Anomalocarimon/Pharaohmon summon logic
  remain intact. Kill-count quest model restrictions are unchanged.

The policy audit on both USA platforms changes the count of protected enemy
spawn points from 450 to 173 (difficulty copies excluded). This makes 277 more
spawn points eligible; it does not guarantee that every seed changes each one.
The two-platform retail regression verifies the reviewed script bytes differ
only by their model-name constants, normal activation overrides use the expected
entrance, other generator fields stay intact, and native bosses/custom entrances
remain protected. The broader dependency audit passes on both USA platforms.

GameCube USA live checks on seed 67, Hard, Crown Mod off: a fresh boot using a
copied native level-316 save loaded the new image. Test-only hub portal destination
changes accessed the stages; loaded stage data came from that image, not an older
savestate. Minotaurmon and Raremon tutorial flags 301/303 changed from 0 to 1,
their introduction dialogs completed, and controller movement resumed afterward.
Numenume River ran through Otamamon waves, the bomb/freeze sequence set flag 489,
and the native transition returned to Abyss of Grief. Electro Mine B2 loaded and
allowed movement with the newly eligible activation generators. Both ruins
cleanup triggers (`mata00`/`mata01`) ran and started GENE-61/GENE-63 with the
replacement rows. The first activation trial exposed a hang when default entrance
fallback sent an SQ_NENTR encounter to MR02 (which lacks SQ_NENTR); the candidate
was tightened to require the same named sequence. A freshly booted rebuilt image
(MD5 `148aa992e93ed4b0e852f7175a92bcef`) passed the exact live trigger again:
Electro Mine B2 GENE-08 spawned Minotaurmon and GENE-10 spawned Raremon, with live
actors moving in subsequent frames. The original failed trial and its native
state are retained as evidence; it is not a passing validation case. Captures and
native runtime snapshots are under `outputs/scripted-spawn-relaxation/` in the
outer workspace. The public-build boss-death crash report is being investigated
separately against unmodified v0.14.0 source. Three initial controlled public-build runs
(two Normal boss phases and one Hard route through preceding Death Valley areas)
passed, but the player-supplied state reproduced the crash after both boss death
and boss victory. See the allocator regression below.

Rules 14 replaces nearest-variant combat stats with original-encounter stats.
The donor row still supplies the model and bounded elemental resistances; its
native HP, attack, wisdom, defence, spirit, speed and status values cannot leak
into the replacement. Both archive-only and application writers use the same
balance function. Application writes read the final transformed original row,
so Crown Mod scaling, exclusions and caps apply once.

The opt-in `seed_67_abyss_inherits_encounter_stats_with_and_without_crowns`
regression passed on PS2 USA and GameCube USA retail images. On each platform it
checked 54 area-local variants across Abyss of Grief’s difficulty packs, with
Crown Mod off and on, and confirmed archive/application agreement with Crown Mod
off. On PS2 it also confirmed the reported MR04 donor rows (57, 58, 62 and 65).
Synthetic checks cover elemental bounds in both directions, unchanged native
rows, and signed-short limits. These are disc-data checks; damage per hit,
overlapping attacks and dodge opportunities still need gameplay validation.

Rules 13 raises the per-stage import ceiling from three to four families and
supports Humid Cave B2's Sukamon introduction (`GENE-01`). Its `first.lua`
appearance wait and enemy freeze/unfreeze commands retarget together. The
replacement is reserved from randomized patrols, while other Sukamon generators
on the floor become eligible. Native entrance positions, camera, tutorial text,
flag `300` and the `GENE-08` Blossomon boss remain intact.

The Sukamon regression covers seeds `1520` and `2162128323`, every difficulty
and all five supported releases. It checks the exact Lua constant rename,
exclusive intro assignment, unchanged spawn configuration and native boss
scripts. With seed `1520`, the introduction becomes MR03 on every release;
seed `2162128323` selects Ogremon. Unused candidate models are still removed,
and selected families retain their shared-capacity and resource-ownership checks.

For the fully enabled seed-1520 GameCube build, imported stage/family pairs
increase from 172 (rules 12) to 219, with no approximate or undrawn effects.
The PS2 build imports 220 pairs. These totals count repeated use of a family in
different stages, not distinct enemy types. Individual stages can use fewer
than four imports when compatibility, the format's object limit or seeded
selection leaves fewer used candidates. Authored entrances, kill-count quest
restrictions and unsupported boss dependencies remain protected.

The rules-13 USA PS2 and GameCube images cold-booted native-card fixtures on
Hard with seed `1520`, Crown Mod, four-player spawns and random crown colours.
Both ran at 100% speed with one joined player. The MR03 replacement completed
Sukamon's introduction, set flag `300` and restored player control. The native
camera and original Sukamon information card remained. Mangrove's first-visit
and boss branches loaded; both randomized waves and ShogunGekomon were defeated,
completion flag `371` was set, and movement worked afterward. Each platform
then completed three further Geko–Mangrove round trips.

Additional rules-13 loads and movement checks covered Power Plant, Mummy Desert
and Ancient Ruins on both platforms, plus Abyss of Grief and Cliff Dungeon
B1/B2 on PS2. These were stage/transition checks, not complete dungeon runs.
Testing used healing, positioning and enemy HP reduction followed by ordinary
attacks; PS2 also used increased defence. Native memory cards were unchanged.
The guest logs contained no invalid reads/writes, and saved allocator checks
passed for ownership, circularity and overlap. Final main-source builds were
byte-identical to the live-tested images (GameCube MD5
`afb71b16fcf8477e0738d5622c55010a`, PS2 MD5
`6effb53b5471eb3ec1ea37e34a09dc46`).

One GameCube diagnostic initially reported a four-byte foreign block after a
return trip. The pointer was the native allocator's empty-top sentinel at
`core + 72`, not a free block. Native instruction inspection confirmed this
representation; correcting the private checker made the captured RAM and later
samples pass. A negative control still rejected an actual foreign free block.
No game change was needed for that diagnostic. A PS2 Mummy Desert movement
probe also initially hit a spawn obstruction; movement in the opposite direction
passed. The small repeated-load drift documented below remains unresolved.
These results do not establish physical-console or four-joined-player coverage.

Rules 12 randomizes Mangrove's two pre-boss waves (`GENE-139`, then
`GENE-131`). The opening `mate00.lua` wait and the timeout cleanup in
`tim_g_orbz03.lua` follow the final selected models, including native fallback
if reward slots are unavailable. Generator completion callbacks, spawn points,
counts, refill modes and drops stay authored. The same families are used on all
three difficulties, with the existing combat/crown matching and original EXP.
Wave replacements may also serve patrols, preserving the stage's choice pool;
only the Tortamon intro keeps its existing exclusive replacement. The five
ShogunGekomon variants and eight scene NPCs remain native.

Retail-data regressions cover seeds `1520` and `1278675365` on all five
supported releases and all difficulties. They verify wave choices, exact script
renames, unchanged wave configuration and callbacks, native boss variants and
other stage scripts. At rules 12, seed `1278675365` covered both waves choosing
the same replacement family; seed `1520` covered a replacement using the other
wave's native family, checking simultaneous name replacement without chained
renames. With rules 13 and four imports, seed `1520` assigns both waves Ogremon;
seed `1278675365` assigns MR03 and MR04 respectively. The current regional tests
check these updated choices and exact script renames across all difficulties.

USA PS2 and GameCube live tests with seed `1520`, Hard, Crown Mod, four-player
spawns and random crown colours completed both randomized waves, spawned and
defeated ShogunGekomon, set completion flag `371`, and restored movement. Both
emulators were explicitly set to 100% speed for these checks. One player was
joined; test assistance healed the player, reduced enemy HP before normal
attacks, and repositioned the player. Checkpoints came from native-card cold
boots of the corresponding rebuilt images; no memory-card changes were saved.
Final free-list ownership/overlap checks passed without logged memory faults.

The first PS2 attempt inherited an unlimited/3x profile and was superseded by
normal-speed tests. A later attempt waited outside the native boss activation
radius after clearing the waves, then timed out. Replaying the post-wave
checkpoint and entering the boss's 115-unit radius spawned it and completed the
encounter without further flag edits. The boss trigger remains authored. The
relevant pre-fight flags matched across platforms (`64=1`, `361=1`,
`371..373=0`); differing story flag `902` controls nearby NPC dialogue rather
than the boss-wave progression/completion scripts.

Rules 11 enables Mangrove's Tortamon introduction (`GENE-165`) to randomize.
The `first.lua` model-name constant changes with the selected enemy, so both
its appearance wait and its removal target the replacement. That model is
reserved from randomized patrols so intro cleanup cannot remove them. The
native camera, flags, information window and control restoration stay intact;
the separate Tortamon preload is retained. At rules 11, `GENE-131/139` and
`GENE-s01..s05` stayed protected; rules 12 enables the two waves. Retail-data regressions cover seeds `1520`
and `1278675365`, all three difficulties and all five supported releases,
checking the exact script rename, exclusive intro assignment, and unchanged
boss groups and other scripts.

USA GameCube and PS2 cold-boot tests also replayed the introduction with seed
`1520`, Hard, Crown Mod, four-player spawns and random crown colours, with one
joined player. Test-only RAM flags enabled the first-visit branch; native cards
were unchanged. Both completed the randomized Ogremon introduction and restored
movement, with clean allocator free lists and no logged guest memory faults.
The PS2's initial capture caught the stage still loading; allowing the load to
finish passed the check. The full boss fight was not replayed in this test.

Mangrove's boss sequence uses authored stage generators: `mate00.lua` starts
the Otamamon wave (`GENE-139`); its completion callback `gene139.lua` starts
the Gekomon wave (`GENE-131`); `gene131.lua` then selects one of the five
ShogunGekomon generators (`GENE-s01..s05`) and attaches the boss HP meter.
The eight `n_gekoa` surrounding the arena are separately scripted NPCs.
This differs from Pharaohmon's `cal_e_pharaoh.lua`, which directly generates
Mummymon from its model callback at the summoner's position. Mangrove's end
scripts addressed the native wave families until rules 12 added explicit
retargeting. ShogunGekomon's boss variants remain protected independently of
allocator fixes.

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
`tim_g_orbz03.lua` cleanup address the Otamamon/Gekomon waves; their generators
and the boss variants are already pinned (`GENE-131`, `GENE-139`, `GENE-s01..s05`). Those groups and
their native model pools remain intact; unrelated patrols can now randomize.
At rules 7, Tortamon's separate introduction also remained protected; rules 11
adds script retargeting for it. Authored spawn sequences remain protected.
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

## GameCube Mangrove allocator regression (rules 10)

Seed `1520`, Crown Mod, four-player spawn counts, random enemies and random
crown colours reproduced a first-load Mangrove failure on Hard in both
v0.13.1 and v0.13.2. The tests cold-booted their respective USA images and
loaded a native character save. GekoGeko Land loading successfully did not
establish that Mangrove Forest would load.

The native string-reference list requested an 8,192-byte replacement buffer
while growing from 1,024 to 2,048 references. That allocation returned null.
The resize caller nevertheless freed the original buffer and recorded the
larger capacity, so later appends overwrote freed heap metadata. At the
recorded failure, the stage arena had 10,344 free bytes in total but its largest
free block was only 5,192 bytes. The allocator's subsequent invalid access was
a consequence of the failed resize, rather than evidence that a different
previous stage was required to trigger the problem.

The runtime hooks redirect individual IGB object allocations from the stage
arena into the existing general arena while tracked general usage is below
4 MiB on GameCube or 3 MiB on PS2. Serialized data and explicitly selected
custom pools retain their existing allocation policy. These are soft object
allocation thresholds: later buffer growth and temporary allocations need
additional headroom.

Existing buffers require an independent ownership check. A serialized list
object can move to the general arena while its backing buffer remains in the
stage arena. Some native list helpers pass the object's pool when resizing or
freeing that buffer. Without correction, a stage block can enter the general
arena's free lists, even before an emulator logs an invalid access. Both hooks
therefore retain the verified native owner of existing stage/general buffers
before the arena operation changes their bookkeeping. New allocations retain
the requested pool; other allocator implementations retain native behaviour.

The PS2 implementation reserves 1 KiB after the original BSS end using an
additional ELF load segment. It advances the kernel heap setup, engine memory
start/layout references and libc's initialized break pointer beyond that
reservation. The original BSS-clearing boundary is preserved. The code occupies
verified unused padding in the boot file's final ISO sector, without relocating
another file. GameCube uses verified unused executable padding.

These changes preserve randomizer rules 10, imported families, generator choices
and resource archives. AREA and NDMW are byte-identical to their respective
pre-hook seed-1520 builds on both platforms. The GameCube seed-1520 plan remains
`fa32b8d9f637ac2c`, with 2,018 generator decisions, 172 copied models and three
donors per stage. Those initial memory-hook checks used rules 10. Rules 11 and
12 separately retarget Tortamon's introduction and Mangrove's two enemy waves;
allocation routing itself does not change script dependencies or enemy choices.

The PS2 USA prototype cold-booted a native save on Hard with seed `1520`, Crown
Mod, random enemies, random crown colours, four-player spawn counts and one
joined player. It completed ten Geko–Mangrove boat round trips, retained working
movement and logged no guest memory faults. The final patcher-generated PS2
image repeated the cold boot and three further round trips. At every saved
arrival, both arenas' free lists were circular, their blocks belonged to the
correct backing ranges and no free blocks overlapped. Stage-arena peak usage
was approximately 9.28 MiB during the ten-round prototype run, versus 9.83 MiB
in the earlier unpatched three-round run, out of a 10 MiB arena. These counters
are not total physical RAM. The smaller repeated-load drift remains unresolved.

The PS2 fixture finished its opening dialogue and enabled gate-access progression
flags in test RAM. Player positions were adjusted to native transition triggers.
Its native memory card was unchanged, and no prior-build savestate was loaded.

The final GameCube USA image cold-booted its native save on Hard with the same
seed-1520 options, loaded Geko and Mangrove, and completed five boat round trips
with working movement, zero invalid reads/writes and clean ownership/overlap
checks at every saved arrival. Its stage-arena peak reached approximately
7.96 MiB out of 8.96 MiB. Earlier allocation-only results are superseded: zero
logged invalid accesses alone did not establish correct free-list ownership.

A second PS2 USA candidate used the original seed `2162128323`, Normal
difficulty, Crown Mod off, random enemies and four-player spawn counts. It
cold-booted and completed three direct Geko–Mangrove boat round trips with
working movement, no logged guest memory faults and clean free-list checks.
This second run did not replay the earlier Humid Cave/Cliff Dungeon/Fortress
completion history. Difficulty was verified from the saved runtime integer on
both PS2 cases.

The emitted-instruction tests exercise native arguments, EE register width,
stack/return preservation, pool eligibility, soft thresholds and existing-buffer
ownership in both directions. The optional retail checks validate executable
fingerprints and code reservations on PS2 USA/Europe/Japan and GameCube
USA/Japan, and reject changed code or occupied padding:

```sh
cargo test -p dw4vhp-core --lib _memory::tests -- --nocapture
```

Additional rules-12 checks used seed `1520`, Hard, Crown Mod, four-player
spawns and random crown colours, with one joined player and both emulators at
100% speed. Eighteen stage/transition checks passed: PS2 covered Humid Cave B1/B2,
hub returns, Mummy Desert, Ancient Ruins and Power Plant; GameCube additionally
covered Abyss of Grief and Cliff Dungeon B1/B2/B3. Loads and movement worked,
free-list ownership/circularity/non-overlap checks passed, and no guest memory
faults were logged. These were load/transition checks, not complete dungeon
combat runs. The small residual growth reported above remains unresolved.

Set the corresponding `DW4_RETAIL_USA_ISO`, `DW4_RETAIL_EUROPE_ISO`,
`DW4_RETAIL_JAPAN_ISO`, `DW4_RETAIL_GC_USA_ISO` and
`DW4_RETAIL_GC_JAPAN_ISO` variables for real-disc checks. Other regional gameplay,
other seeds, longer stage histories and physical hardware are not covered by these runs.


## GameCube fragmented-buffer allocation after Apokarimon

The player-supplied Hard-mode `GDJEB2.s02` (SHA-256
`2fb4fd5d4fab303d32819f6a700b29e4001b8c89a2a0fa4ede991b655911146f`)
starts in Doom Dome F2 before the boss portal, with public v0.14.0 seed 67,
random enemies and cosmetic crown colours, and no Crown Mod stat boost.
The original input is preserved in the outer workspace’s
`outputs/seed67-public-crash/player-input/`.

From that state, both native boss death and native boss victory, followed by
hub → Death Valley, reproduce a null read at PC `0x80042a28`. The loader’s
object-reference list has 65,605 entries and capacity 66,560, requiring a
260 KiB contiguous buffer. At the captured death-route fault, the general
arena has 2,747,756 free bytes in total but its largest block is only 138,288
bytes; the list’s buffer pointer is null. The scene arena still has a
1,353,064-byte free block, and both arenas’ free-list ownership and overlap
checks pass. This is allocation failure under fragmentation, rather than an
observed corrupted free list.

The allocation hook now retries **fresh** positive-size general/scene buffer
allocations in the other arena after native allocation failure. It validates
the alternate arena and allocator class, preserves the requested size/alignment,
and returns the native failure if neither arena can allocate. Existing buffers
never migrate on resize/free: the ownership hook first resolves their actual
arena. Zero-size, native `-1` free, custom-pool and existing-pointer operations
keep their native allocation behavior.

Instruction-level regressions exercise success, fallback in both directions,
both-arena failure, absent/incompatible alternate arenas, free/zero-size calls,
existing pointers, and stack/return-address/callee-saved-register preservation.
Retail patch sites and unused code space pass checks on GameCube USA and Japan.
The core gate passes. A built public-rules control plus allocator fix retains an
identical NDMW.AFS offset, size and SHA-256 to the unfixed control, isolating this
change from the stat and scripted-encounter work.

Both captured fragmented hub states (death and victory) load Death Valley with
the fix, allow movement, and log no invalid accesses with healthy free-list checks.
The death-route replay additionally passes a Humid Cave → Death Valley unload/reload
cycle, exercising cleanup of the fallback-allocated buffers. Native test states
and detailed results are in `outputs/seed67-public-crash/` outside the repository.
Boss victory was accelerated with pillar/boss HP edits followed by normal attacks,
retaining native victory callbacks and the return gate. Death was accelerated
by lowering player HP before a normal boss hit. No story flags were changed for
the supplied-state reproduction or fixed replay. Old savestates contain the old
executable patch; testing a new disc through an old state therefore also requires
applying the new code to a separate state copy.

## Practice-stage eligibility (v0.15.0, rules 15)

The practice model pack (607, `stag0001`) is now a destination for ordinary
seeded enemy replacements. It remains forbidden as a donor. Generator packs
608–610 each contain 14 enemy generators (`GENE-a00`–`a06`, `GENE-b00`–`b06`)
and four box generators. Enemy families are Goburimon, Ogremon, Numemon and
Kokatorimon. Special technique/charge damage handlers belong to lesson rocks
and boxes; the enemies use normal combat hit scripts. Progression is attached
to generator callbacks rather than hard-coded enemy family names.

`practice_randomisation_preserves_lessons_and_party_branches` was run against
clean USA PS2 and GameCube discs with seed 67, normal and four-player spawns.
For each release/option pair, all 42 enemy generator records across three
difficulties changed, while box rows, counts, positions, callback references,
drops and all original Lua payloads remained intact. The four-player check
compares against the already-transformed graduation script, retaining its
party-size threshold edits. Real practice-local stat/model databases resolve;
variant writes preserve the original practice row's HP, non-elemental stats,
status rates, rarity and EXP after each configuration's single transform.
The matrix covers Crown Mod off, Crown Mod on with practice excluded, and
practice buffs/speed enabled. Elemental differences use the shared ±100 bound.

Rules version 15 covers this eligibility addition together with the other
v0.15.0 changes. Other stage seeds use
stage-local RNGs; practice models remain outside the donor pool.

Live GameCube USA validation cold-booted a fully rebuilt seed-67 image with
random enemies, Crown Mod off and normal party counts. In an isolated Dolphin
profile, a copied save and a temporary hub-portal target entered `stag0001`;
practice entry flag 503 was reset for the test. Teleports shortened travel
between targets. Native controller attacks defeated the three lesson targets:
MR01 at 20 HP, MR01 at 25 HP, and Goburimon at 30 HP. Each advanced the original
lesson callback. A charged attack on the original box set flag 514 and started
the native graduation wave: one MR01 at 30 HP and two Ogremon at 20 HP each.
After native kills, the exit offered **Go to Death Valley? / Practice again?**,
confirming that the graduation clear reached the native completion condition.
Confirming the exit loaded Death Valley (`stag0101`) successfully, with player
movement verified afterwards. Rocks and teaching scripts were not modified. This is an accelerated progression
check, not a complete manual lesson walkthrough. PS2 gameplay, other seeds and
multiplayer gameplay were not exercised by this live run; the disc audit above
covers the PS2 data and four-player option composition.

Private evidence: `outputs/practice-randomization/` in the outer workspace;
`retail-check.log`, `core-check.log`, native checkpoints, screenshots and
`live-result.json`. The verified temporary image was 2,271,340,544 bytes,
MD5 `3d5b4a587cfd0f183845a241656d4d90`. The isolated emulator was stopped
after validation; the temporary image, redundant decoded RAM dumps and copied
test-card data were removed (about 2.80 GB reclaimed). Original player inputs
and the separate public crash-fix playtest image were retained.
