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
randomize only when the replacement implements that same named sequence for
the selected difficulty. The override is preserved; Minotaurmon uses its reviewed
`SQ_NENTR` bypass. These include the cannon Numemon in Goblin Pass (`SQ_WAIT`)
and MR01 falling onto the train (`SQ_FALL`). Their entrance motion belongs to the
original model's animation/controller; clearing the sequence or assigning another
model loses that motion. Other generators in those stages can still randomize.
Bosses, script-dependent generators and kill-count restrictions also apply.

The current randomizer uses rules version 28. The same seed is reproducible within
a rules version; older images need rebuilding to receive these fixes.
Imports fill unused registry capacity before extending it, preserving the backing
block while keeping empty resource entries outside the active list. This also
applies to scene children and script registries after native models are retired.
Retirement also preserves a named scene when its draw tree contains another
registered model. A retained action/controller alone does not keep its scene
ancestors registered, so imported actors must remain reachable from a draw scene.

## Introduction previews (rules 28)

Rules 28 makes each enemy introduction preview its stage. Introductions now
choose after patrol planning, from the compatible families the floor's other
enemies use on that difficulty. Imports and families with a retail information
card are preferred. Families of protected encounters, such as MR05 in Mecha Nest
B3, are never chosen. Two introductions on one stage prefer different families.
An otherwise unused resident is used only if no stage family fits. This
applies to all 16 introduction sites the randomizer supports. Arukenimon's Grief
Wasteland introduction remains native.

Earlier rules kept each introduction family out of the patrols, because the
scripts freeze, wait for and erase enemies by model name. In retail, though, every
introduction family also patrols its own stage: the Minotaurmon floor has five
other Minotaurmon generators, Goblin Pass has 45 Goburimon generators, and so on.
Sharing a family therefore restores the retail arrangement.

The introduction's card follows its model. Rules 28 adds 19 retail family cards
to the AREA bank used for boss cards: the arenas' boss cards come first, then the
family cards. Introduction scripts select the new message rows, separately for
each difficulty. Destination card rows come from each stage's native card.
MR01–MR03 and MarineDevimon have no retail card, so those previews keep the
native text. Forest Entrance's Otamamon and Gekomon cards share a type-line row,
which changes only when both introductions agree.

Data checks on PS2 USA, Europe and Japan and GameCube USA and Japan covered seeds
7, 67, 1520, 1278675365 and 2162128323 and every difficulty of the 12 introduction
stages. They found 240 previews per release, no exclusive fallbacks, no protected
families and exact script renames, and every changed card line matched its
family's retail text in all language packs. Every family card was also checked
against every introduction and language. The GameCube USA seed-1520 image (Hard)
passed live tests at the Sukamon, Minotaurmon, Raremon, Tortamon and Goblin Pass
introductions. They showed Kokatorimon, Goburimon, Mummymon, Sukamon and Raremon,
each with that Digimon's card. During each introduction, only the introduction
actor was alive. The erasing introductions removed only that actor; control
returned afterwards. Later patrols of the previewed family spawned and moved on
the Minotaurmon, Sukamon, Raremon and Tortamon floors. Raremon patrols were not
reached at the three Goblin Pass generators visited. PS2 has data checks only.

## Repeat boss fights (rules 29)

Rules 29 adds the eleven repeat boss encounters to the Random bosses shuffle:
Abyss of Grief (Blossomon), Grief Wasteland and Mangrove Forest (SkullGreymon
and ShogunGekomon, five strength variants each, started by a random roll in
`mata05.lua` and `gene131.lua`), Quest 1 (Mammothmon), the three Quest 4
variants (Anomalocarimon), Quest 7 (ShogunGekomon) and the three Quest 8 variants
(one to three sequential LordKnightmon). Only the four world bosses and the
final-boss stages stay native. Each encounter is one slot in the per-difficulty
inventory, so the shuffle now covers 20 encounters per difficulty. All of an
encounter's generators change together. Quest 1's `SQ_WALKIN` uses the same
reviewed activation fallback as Cliff Dungeon B3. Every replacement must provide
`SQ_DDMG`, which Mangrove's orb timeout sends to the boss by name.

Mangrove Forest has no unfilled stat rows left after patrol planning. When a stage
cannot supply a new row, a boss row used only by that encounter (on that
difficulty, across all of the stage's packs) is rebound in the stage's own
database: it keeps its stats and EXP and takes the replacement's model and bounded
elemental resistances. Encounters with neither keep their native boss.

PS2 USA seed 1520 (vanilla balance) changed all 60 boss slots. The full-image
audit checked every slot's patrol exclusion against the written MODEL databases,
93 replaced boss generators (15 on rebound rows), 1,554 localized stat cases,
the 27 dungeon cards, 60 victory callbacks, the absence of any stage script still
addressing the original boss, health meters following the replacement, the 18
world-boss generator packs and byte-for-byte recovery. GameCube USA seed 1520 passed the
same audit with the same totals. On retail data, 151 of 160
site/family combinations fit; the rest are patrol-resident MR04/MR05 in Quest 8
and ShogunGekomon lacking a walk-in fallback.

Live checks on that image used Hard, one player and a level-999 save, with
healing, teleports and enemy HP zeroed before ordinary attacks. Mangrove Forest
(both waves, then Anomalocarimon), Abyss of Grief (LordKnightmon), Grief
Wasteland (MR05), Quest 1 (Blossomon walking in), Quest 7 (Diaboromon) and all
three rooms of Quest 8 Very Hard (ShogunGekomon) spawned, fought and set their
native defeat flags (371, 66, 368, 115, 187, then 208, 250, 251 and the quest
clear flag 452), with control restored and no logged guest memory faults. Quest
4's replacement spawned from its native trigger, but the test harness did not
complete that fight. Quests were reached through the developer stage menu;
redirecting a world gate straight into a quest stage crashes the game without
quest context and is not a valid test route. GameCube, Europe, Japan, Normal and
Very Hard were not tested live.

## Optional dungeon bosses

The app’s **Random bosses** setting appears under Random enemies and starts
disabled. It requires Random enemies, shares its seed, and is included in settings
files and image metadata. Supported releases are PS2 USA, Europe and Japan, and
GameCube USA and Japan. Turning off Random
enemies clears the dependent setting. The production builder shuffles dungeon
bosses after the final patrol selection. It preserves each difficulty's encounter
inventory and excludes the selected patrol families across every floor of the
dungeon. World bosses remain fixed. Boss imports retain the
arena's dormant original resources and callback names, and donor hit scripts use
the arena's health display. Reviewed entrance fallbacks prefer `SQ_ENTR`:
Blossomon's `SQ_NENTR` selects normal-enemy death labels and can prevent the
arena's victory callback from running.

Rules 27 adds Tortamon to the import pool, from its embedded body in `stag0301`.
Mangrove Forest cannot import it because it keeps the separately loaded native
resource. Rules 27 also hashes the seed and salt into each stream's start state;
XORing them let small seeds share one random stream between stages. A PS2 USA
image with Tortamon forced into Goblin Pass passed live spawn, rendering, combat,
defeat, EXP, drop and stage-exit checks. GameCube has static test coverage only.

The earlier USA prototype used literal English cards. Rules 26 retains native
Unicode rendering and supplies localized message banks in a rebuilt AREA archive;
difficulty-specific intro scripts select the donor’s four message indexes. All
original messages and the final local databases are preserved. The regional
validation below is static and does not add regional emulator coverage.

Rules 26 seed 1520 built through `PatchSelection` on all five supported releases:
PS2 USA, Europe and Japan, and GameCube USA and Japan. Each full-image audit
verified 27 boss choices against every dungeon floor’s written MODEL database,
189 localized cards across all seven native language packs, 580 native stat rows,
567 local boss-stat cases and 18 fixed world-boss generator packs. Drops and
complete destination victory callbacks match their originals. Recovery restored
each source disc byte-for-byte, including both archive extents. Japanese PS2 used
Crown Mod balance; the other four images used vanilla balance. These regional
checks used file-level audits without additional emulator runs. Rebuilding AREA
preserves its original messages and adds its archive size to the output; the app’s
size estimate includes this.

PS2 USA seed 1520 passed accelerated Normal, single-player live checks on the
final image with a level-99 test save. Teleports shortened travel; native
controller attacks defeated the bosses without enemy-HP or defeat-flag writes.
Blossomon in Cliff Dungeon B3 rendered with its matching info card, set the
original flag 150 and cleared 508, while its donor flag 119 stayed clear. Vine
Tunnel B3 spawned three Scorpiomon at the original encounter's 1675 HP each;
flag 157 remained clear while any boss survived and set after all three died.
Scorpiomon's donor flag 189 stayed clear.

GameCube USA seed 1520 passed live spawn, intro and victory checks for all nine
Normal boss encounters in Dolphin with one joined player and a copied native
level-316 save. Every arena's original defeat flag set after native attacks,
while the replacement's donor-arena flag stayed unchanged. The rebuilt image
cold-booted before creating its checkpoints. Hub transition arguments and player
positions shortened travel; enemy HP, defeat flags and boss scripts were not
edited. Cliff Blossomon set flag 150 while 119 stayed clear. Vine Tunnel spawned
three Scorpiomon at 1675 HP each; flag 157 stayed clear after the first and second
kills and set after the third, while 189 stayed clear. Both encounter segments
logged zero invalid memory reads or writes, as did the other seven encounters.
Mecha Nest's native Raremon entry introduction must finish before a test
shortcut enters the boss trigger. Overlapping those cutscenes misframes the
boss card; completing the entry intro restores the native camera and correctly
frames ShogunGekomon with its matching card.

All 27 boss/difficulty slots passed resource, draw-scene, activation and victory
bytecode checks on PS2 USA and GameCube USA. Boss generators and their
drop/callback dependencies remain unchanged except for the enemy row. The audit
also checked 580 native stat rows, 567 localized stat cases and 18 fixed
world-boss generator packs on each platform.
Hard/Very Hard gameplay, multiplayer and complete campaign routes still need
live coverage. Key-item pickup was not separately exercised live. Private
evidence is retained in the outer workspace's
`outputs/boss-randomizer/live-check/live-result.json` and
`outputs/boss-randomizer/gc-live-check/live-result.json` with adjacent captures.

The rules-25 production app path passed full-image checks on USA GameCube with vanilla
stats and USA PS2 with Crown Mod, four-player spawns, random crown colours and
custom drops. The boss-off control retained all native bosses. All 27 boss
selections were independently checked against the written MODEL databases on
every dungeon floor, alongside the intro, callback, count and stat checks.
Whole-dungeon exclusions change two Normal seed-1520 placements: MR04 replaces
SkullGreymon in Sand Labyrinth, and MR05 replaces Diaboromon in Vein. Both final
GameCube fights passed native attack/defeat-flag checks; their donor flags stayed
clear. The other seven Normal boss model, sound and generator packs match the
previously tested resources. Evidence is in `outputs/app-boss-integration/`.

DigiAtlas v0.5.0 completed imports but omitted spawn lists when appended
difficulty resources were referenced: only eight of 98 stages retained generator
data and all nine shuffled boss stages were missing. Its generator scan started
after the highest referenced model entry, beyond the native generator files.
The failure evidence remains in
`outputs/app-boss-integration/atlas-compatibility.json`.

The companion Atlas importer 1.28.0 resolves named generator files through
NDMW.IGB instead. It passed the complete spawn/model/stat/position and schema
regression on all five rules-26 images and both rules-25 USA test images. Each
new regional image exports all 27 dungeon bosses and 10,287 live spawn records;
all 81 stages with generators are restored. Tests also passed on unmodified discs
from every supported region, the Crown Drops image and the map UI’s overlapping
spawn groups. Importer versioning invalidates affected old caches. These changes
require a new Atlas build; v0.5.0 itself is unchanged. Evidence is in
`outputs/boss-regions/`.

## Train-side ranged encounters

Rules 23 applies the river's projectile-only family pool and firing policy to
Train Line's ordinary side generators. The train keeps its four-family budget
per difficulty; Numenume River retains its three-family budget. Ranged choices
use verified projectile actions instead of melee attacks across the gap. MR05
uses the same reviewed off-platform firing bands as the river. Native MR01
boarding encounters, including `SQ_FALL`, their AI and their positions, stay
unchanged.

The opt-in `retail_train_sides_are_ranged_on_every_difficulty_and_boarders_stay_native`
audit passed for seeds 7, 67 and 1520 on PS2 USA, Europe and Japan, and GameCube
USA and Japan. Each difficulty retained 38 native MR01 falling generators and
26 ranged side generators, with four selected side families and distinct
rosters across difficulties. The audit also compares native MR01 action records
and generator positions/counts, and checks firing-policy idempotence. The
shared firing-policy preservation audit passed on all five releases. The
moving-projectile dependency audit passed on PS2 USA and GameCube USA. These
are disc-data checks; gameplay results are recorded separately below.

Rebuilt rules-23 USA images cold-booted in PCSX2 and Dolphin with Vulkan. On
Hard, seed 1520 loaded MarineDevimon, MR05, Megadramon and Imperialdramon beside
the native MR01 pool. Live samples on each console observed all four families'
projectile actions and moving projectile instances: MarineDevimon `SQ_MAGIC`,
MR05 `SQ_FIRE`, Megadramon `SQ_MAGIC` (released projectile `SQ_BULT`), and
Imperialdramon `SQ_FIRE`. Captures were inspected with the player on the train
and the ranged enemies on the side platforms. Native MR01 instances were
tracked from `SQ_FALL` above the roof to roof height and ordinary combat.

These were assisted encounter samples with one joined player and four-player
spawn counts. The opening projectile sample used the moving train. Later
samples restored a checkpoint created from that same rebuilt image, parked and
relocated the train (including its culling bounds), and used player HP and
invulnerability assistance. Enemy AI and generator rows were not edited during
the live checks. This does not establish a full route playthrough, projectile
accuracy for every family/seed, physical-console behavior, or four joined
players. Temporary disc images, profiles, states, memory dumps and captures
were removed after verification. Both consoles returned through a native stage
transition to reloaded hub resources with objective/exit-destination assistance.
Captured allocator checks passed, with no logged guest memory faults.

## Different families on each difficulty

Rules 22 plans Normal, Hard and Very Hard separately. Each difficulty prefers
families that the earlier difficulties have not used, then reuses compatible
families when the available pool or encounter restrictions require it. The
four-import budget applies independently to each difficulty; the river keeps
its three-family ranged limit. Bosses and protected encounters retain their
existing restrictions. A small or constrained stage may still use fewer families.

Each difficulty has its own model and sound packs, with native bodies and pool
capacity reclaimed against that difficulty's generators. The original archive
entries hold Normal, and appended entries hold Hard and Very Hard. Stage
manifests select the current pack before native file loading and caching. The
selected filename uses the game's managed string pool so asynchronous loading
can retain it safely. Sound selectors preserve their native `/s` suffix and
loading order. The image grows, but a stage loads one difficulty's resources.

The expanded archive updates the root file count and retains a packed sector
layout. Native CRI caches file sector counts and sums them to locate data, so
the filename table moves after all appended resources. The planner rejects an
archive whose explicit offsets differ from those cached offsets. Executable
hooks, the file count and archive changes participate in output verification
and recovery.

Instruction tests execute the MIPS and PowerPC filename wrappers for all three
difficulties, passthrough names, sound suffixes, invalid selectors and managed
string ownership. The opt-in regional audit checks native instruction
fingerprints and executable space on all five supported releases. The opt-in
roster test checks distinct family sets, corresponding resource packs,
four-family coverage where supported, and protected generators on configured
discs.

The rules-22 seed-1520 roster and routing audits passed on PS2 USA, Europe and
Japan, and GameCube USA and Japan. Targeted USA emulation recorded nine resource
loads on each console: Goblin Pass on all three difficulties, Train Line on
Hard, Humid Cave B2 on Very Hard, Mangrove on Hard, and the river on all three
difficulties. Resource inspection found the expected replacement pools and
excluded the other difficulties' replacement sets in Goblin Pass and the river.
Train Line and Humid Cave samples found all four selected families marked active
in RAM on both consoles. Other samples covered fewer families; they do not
establish every encounter's progression. River samples checked loading and
boat activity rather than a complete run or projectile accuracy.

These checks used one joined player with four-player spawn counts, a copied
native card, the live named difficulty setting, portal redirects, positioning
and HP assistance. Train exit progression used its objective flag. River
branches restored a hub checkpoint from the same image. The six non-river
visits per console reloaded hub resources through native transitions. All captured allocator
checks passed circularity, ownership and overlap checks, with no guest memory
faults logged. This is targeted emulator coverage, not physical-console,
four-joined-player or complete-playthrough validation.

The final PS2 image cold-booted again, rendered Goblin Pass on Hard with its
replacement enemies, and returned to the hub. GameCube USA also passed rendered
Goblin Pass checks on Normal, Hard and Very Hard from a cold-booted rules-22
image. The first encounter visibly used Otamamon, MR02 and Pharaohmon,
respectively, with the expected four-family resource pools for each difficulty.
These samples used a copied native card, live difficulty and world-unlock
settings, player positioning and invulnerability assistance. Difficulty branches
restored a hub checkpoint created from that same rebuilt image.

Goblin Pass's blue arrival platform was initially misidentified as the hub;
there was no rendering mismatch. The unmodified game showed the same field-entry
platform. Scene checks now read the player's `_isMainLobby` flag, wait for
enabled, unfrozen control, and move off the arrival pad before inspecting the
terrain and enemies. The flag was set in the hub and clear in every verified
field sample. A loader message or a shared portal asset alone is insufficient
visual evidence. The three difficulty screenshots were inspected, and all
captured allocator checks passed with no logged guest memory faults.

Independent final-image checks confirmed every packed AFS sector offset, filename
table placement and recorded file length, plus byte-exact recovery of both
original inputs. All 1,261 GameCube and 1,260 PS2 resource payloads, and the native
executable changes, match the earlier nine-load builds after the filename-table
repair. Temporary images, extracted files, captures and test profiles are removed
after validation; only code, metadata and reports are retained.

## Four-family patrols and native allocation reclamation

Rules 21 prefers up to four compatible imports for ordinary patrols and waves.
An augmenting-path coverage pass assigns distinct compatible families before
the remaining groups are balanced by their largest authored party population.
The tie order is seeded. Native families are compatibility fallbacks; the usual
rarity, entrance, actor-capacity, variant-row and quest restrictions still apply.

Exclusive introductions use compatible resident families, excluding other
protected encounters' families unless retaining the original intro. Their
named waits and controls still follow the selected actor. This keeps the four
imports available for patrols rather than spending them on one-time cutscenes.
The river retains its reviewed three-family ranged policy. Small stages and
scripted family groups can support fewer than four replacements.

After each difficulty's assignments and script rewrites are final, the randomizer can
retire unused native enemy bodies and shrink retained native pools to remaining
generator demand. Unknown script dependencies preserve native headroom. The
reviewed introduction commands wait, freeze and clean up existing actors, so
those commands alone do not reserve an entire former patrol pool.

Retirement detaches the body's model, animation, physics/shadow registrations,
owned callbacks and scene branch. The reference audit follows every loader
reference, including spare words beyond list counts, and preserves shared
resources. Retained action-command string lookups protect their named models;
hit-region filters and reviewed scene-object labels do not require an absent
enemy body. Companion projectiles, effects and native audio banks remain
independently registered. Their presence does not require the original actor.

Surviving IDs and backing blocks stay fixed. Retired IDs become inert four-byte
raw buffers: leaving old objects or reference-bearing blocks orphaned would
still allocate them or retain their children. Serialized-byte savings reported
by the audit are not measurements of runtime RAM savings.

Synthetic tests cover four-family coverage across 100 seeds, constrained
compatibility, reproducibility, mixed group populations, shared resources,
stable IDs and stale references in spare list capacity. The opt-in
`retail_four_family_distribution_reclaims_unused_natives` regression checks
final resources, every difficulty, preserved spawn profiles and protected
generators on configured discs. It specifically requires four patrol families
in Goblin Pass independently of the introductions. `DW4_VARIETY_SEED` and
`DW4_VARIETY_ONLY` can restrict a development audit; `DW4_VARIETY_EXPORT` writes
private generator metadata outside the checkout for gameplay checks.

The rules-21 seed-1520 audit passed across every scanned stage and difficulty
on PS2 USA, GameCube USA and GameCube Japan:

| Release | Stages using four imports | Unused native bodies retired | Native pools below authored size | Serialized bytes reclaimed |
| --- | ---: | ---: | ---: | ---: |
| PS2 USA | 60 | 133 | 27 | 22,419,448 |
| GameCube USA | 57 | 131 | 27 | 21,667,928 |
| GameCube Japan | 57 | 131 | 27 | 21,667,928 |

Body and pool counts are stage/family pairs, not distinct species. Totals span
the archive, rather than representing memory freed in one loaded stage.
Goblin Pass's 66 ordinary groups use MR03, Infermon, Kokatorimon and Minotaurmon
in a 17/17/16/16 split. Its Goburimon and Ogremon introductions each retain one
native actor; the four required Numemon cannon actors remain. The signed PS2
USA and GameCube USA builds also passed full output verification with Crown
Mod, four-player spawns and cosmetic crowns enabled.

The final GameCube USA image cold-booted a copied native card on Hard with one
joined player and four-player counts. Goblin Pass, Humid Cave B2 and Mangrove
loaded, ran combat samples and returned to the hub; Goblin Pass was visited
again. All four Humid Cave replacement families were observed active. Portal
destination redirects, player positioning and HP assistance shortened the route.
Every captured allocator check passed ownership, circularity and overlap checks;
the guest log recorded no invalid reads or writes.

A separate check restored a hub state captured from that same final image,
reset Sukamon's introduction flag and loaded Humid Cave B2. The replacement
Numemon used one actor slot, appeared in the native introduction, opened and
closed the info card, and restored player movement through ordinary controller
input. No state from an older mod image was loaded. These are targeted gameplay
checks, not complete playthroughs or physical-console/four-joined-player coverage.

The final PS2 USA image also cold-booted a copied native card on Hard in PCSX2
with Vulkan, one joined player and four-player counts. Loading, combat samples,
native exit callbacks and allocator checks covered Goblin Pass, Humid Cave B2,
Mangrove Forest, Train Line and Numenume River. All four Humid Cave replacements
were observed active. Train Line ran Arukenimon, Mummymon, MR03 and MR05 while
its protected MR01 fall generators retained their native assignments. Its exit
used the native finish state with progress assistance.

The PS2 river ran Infermon, MR02 and Otamamon together, and each produced an
independently moving native projectile through its reviewed firing sequence.
All five bank cleanup triggers passed. With completion-flag assistance, the
native victory exit loaded `stag0104`. A checkpoint captured in this exact
image/run then exercised the native bomb/freeze trigger, set flag 489 and loaded
`stag0102`. No state from an older mod was used. Portal destinations, player
position/HP and objective progress were assisted; the enemy models, AI and
projectile positions were not edited at runtime. All captured allocator checks
passed and no guest TLB, address, invalid-read/write or allocation faults were
logged. This is targeted stage coverage rather than an unassisted playthrough.

A further GameCube USA cold boot in headless Vulkan covered the same five areas
on Hard, with one joined player and four-player counts. All four Humid Cave and
Train Line replacements were observed active; the train also ran native MR01.
The first four areas completed combat samples and returned through native exit
callbacks with destination/progress assistance where needed.

The GameCube river ran Infermon, MR02 and Otamamon together, with each firing an
independently moving native projectile through its reviewed sequence. All five
bank cleanup triggers passed. Completion-flag assistance exercised the native
victory exit to `stag0104`; a checkpoint from this exact image/run then exercised
the bomb/freeze trigger, flag 489 and native exit to `stag0102`. All 77 captured
allocator checks passed, with no invalid reads/writes or allocation faults.
Player position/HP, boat health and objective progress were assisted; enemy AI,
models and projectile positions were unchanged at runtime. The emulator was
stopped after verification. Temporary validation images, captures, RAM dumps,+savestates and copied profiles were removed; results, logs and build metadata
were retained. These remain targeted emulator checks, not full playthroughs.

Rules 17 limits Numenume River replacements to the thirteen projectile-capable
families below. The restriction applies to both resident
replacement candidates and imports. Incompatible donors are skipped in shuffled
order; the river exhausts its available ranged candidates without the ordinary
failed-import attempt cap before falling back. Its banks use up to three compatible
ranged families, with cleanup and bomb-freeze commands covering every selected
family. Native scripted entrances and whole-group fallback when local variant
slots run out remain in place. See [mixed river encounters](river-mixed-enemies.md).

The pool requires native projectile spawn evidence, rather than an effect name:

| Family | Firing action | Spawned model | Projectile sequence |
| --- | --- | --- | --- |
| Numemon | `BLST01` | `x_003eblst01a` | `SQ_ATCK` |
| Sukamon | `BLST01` | `x_005eblst01a` | `SQ_BLST` |
| Minotaurmon | `NATK01` | `x_012eblst01a` | `SQ_ATCK` |
| MR02 | `BLST01` | `x_014eblst01b` | `SQ_ATCK01` |
| Otamamon | `BLST01` | `x_018eblst01a` | `SQ_ATCK` |
| MR03 | `BLST01` | `x_021eblst01a` | `SQ_ATCK` |
| MR04 | `BLST01` | `x_023enatk01a` | `SQ_BLST01a` |
| MarineDevimon | `MAGC01` | `x_020emagc01c` | `SQ_ATCK` |
| Megadramon | `BLST01` | `x_027emagc01a` | `SQ_ATCK` |
| Infermon | `BLST01` | `x_026eblst01a` | `SQ_ATCK` |
| Raremon | `BLST01` | `x_028eblst01a` | `SQ_ATCK` |
| MR05 | `BLST01` | `x_029eblst01a` | `SQ_BLST01a` |
| Imperialdramon | `BLST02` | `x_025eblst02a` | `SQ_BLST02` |

`river_pool_has_native_moving_projectile_spawns` checks that each enemy plays
the listed firing action, a registered action starter binds that action to the separate model,
and the spawned sequence moves forward and enables a damaging hitbox.
Mummymon's companion effects do not establish a moving ranged projectile;
it is excluded. Compatibility checks may reject an import even when its
projectile evidence passes.

`river_replacements_use_only_ranged_families` exercises multiple seeds and individual
donor supplies, checks each difficulty’s selected families and resources, and checks
that a melee-only donor supply leaves native enemies in place. The same regression checks every selected family's cleanup/freeze commands and
shared actor capacity on all five supported releases. Data checks establish projectile capability; the live checks
below additionally exercise rendering, collision and stage unloading. Natural
hit accuracy throughout the route is not exhaustively measured.

The initial river compatibility audit (`probe_river_donors`, devtools) finds the
same source-layout exclusions on PS2 USA and GameCube USA: seven of the twelve
nonresident projectile families have no compatible source scene. Sukamon,
Minotaurmon, MR03, MR04, Raremon, MR05 and Imperialdramon require the missing
shared `EM01` enemy group. MR02, Otamamon, MarineDevimon, Megadramon and Infermon
each have a standalone-scene donor source. Numemon is already resident. These
are scene-registration exclusions, not measured runtime RAM exhaustion; actual
imports, actor capacity, entrance compatibility and local variant allocation
still run afterward. Adapting grouped models to the river's standalone scenes
is now implemented for the river: its native scene gains a plain shared enemy
group while preserving the original model switch, objects and backing blocks.
All twelve nonresident ranged families now have a compatible grouped donor
source. The seven previously blocked families also have explicit import and
cross-difficulty selection coverage. See [river scene validation](river-enemy-scenes.md)
for rendering, collision and stage-unload evidence and test assistance.

Rules 16 adds Pharaohmon to the import pool. Its native `cal_e_pharaoh.lua`
control flow stays in place: each `gen` call fills one of twelve logical summon
slots. A fresh runtime `SYSgetRandom` roll chooses among compatible families
actually selected for that stage, excluding the previous summon family when
there is another choice. Build seeds determine the stage pool; gameplay RNG
and timing determine the individual summons.

Each eligible family has twelve reserved actors above its ordinary generator
capacity; live checks follow the family/actor recorded for each logical summon.
All Pharaohmon in the stage share the twelve-summon limit, as in retail.
Summons inherit Mummymon's original summon encounters (rows 116/120/124), using
the existing local-variant balance writer so selected Crown Mod settings apply
once. When a crowded stage has no free variant rows, summons instead reuse
that family’s selected encounter row for each difficulty. The original second
GENERATE parameter, summoner position and rotation
remain intact. Families use an available normal `SQ_NENTR`, `SQ_ENTR` or
`SQ_SWON` entrance. Pharaohmon itself uses its native `SQ_ENTR` when replacing
an ordinary activation; its authored attack/fall entrances remain protected.

Intro and family-cleanup replacements stay exclusive and cannot be summoned.
Pharaohmon cannot summon itself, bosses or unselected imports. Counter-aware
quests and stages controlling Pharaohmon by name keep their native callbacks.
Mummymon's hit and magic-request callbacks translate only the new summon actor
indices back to logical indices 0..11; ordinary actors and later physical actor
lookups keep their original indices. A stage change resets the summon tracking.

The opt-in `retail_pharaoh_summons_follow_selected_families` regression exports
scripts and pool/row metadata when `DW4_PHARAOH_AUDIT_DIR` is set. Run
`tools/verify-pharaoh.lua` in a Lua 4.0.1 runtime after setting
`pharaoh_audit_dir` to an exported stage directory. This checks per-summon rolls,
no consecutive repeats, the twelve-enemy limit, partial refills, positions,
difficulty rows and Mummymon's request/reset coordination. The focused disc
regression passed for Abyss of Grief and dungeon pack 249 on all five releases.
Their ten exported callback sets passed the Lua 4.0.1 runtime checks across all
three difficulties, including Mummymon's hit/healing coordination and stage
tracking reset. A full PS2 USA image with seed 67 and four donor families was
written and verified. Core checks (bindings, generated rules, formatting,
Clippy and workspace tests) passed.

GameCube USA live validation then exposed a missing behavioral initialization:
Pharaohmon's `SQ_NENTR` branch allows ordinary combat but omits the native
`CHECK` label setup needed for summoning. Imported/replaced Pharaohmon now
starts through `SQ_ENTR`; regression checks enforce that entrance on every
difficulty. A diagnostic RAM change first established the cause. The final
validation cold-booted a newly rebuilt disc and loaded the dungeon afresh,
without that RAM change or any forced summon command/RNG.

The focused live image randomizes only `dng205b1` (pack 249), seed 1520, with
one import candidate prioritized to Pharaohmon. Crown Mod and four-player
spawn overrides are off. A copied native level-316 character entered Hard mode;
only the hub portal destination, player position and player health were edited
for setup. `GENE-02` loaded `SQ_ENTR` from the new image. Pharaohmon naturally
summoned Minotaurmon in reserved slot 15 and then Mummymon in reserved slot 17
(row 120, HP 896). Mummymon remained alive and moved over the next four seconds;
the player still had 99,754 HP. Evidence and native states are in the outer
workspace's `outputs/pharaoh-live/`, particularly `fresh-live-result.json`,
`fixed-summon-confirmed.sav` and the final encounter captures. Dolphin was left
paused at the encounter.

Release validation reproduced a broad GameCube seed-1520 build failure at pack
335 (`memory block outside pool`). Growing a memory block could resize a
previously unique directory descriptor which newly copied blocks also reused.
The builder now gives the grown block a private descriptor when copies share
it, and invalidates stale descriptor-cache entries before later copies.
A synthetic regression reproduces the failure and checks copies both before
and after growth. The same build then exposed exhausted variant space at pack
709; summons now reuse selected encounter rows when additional variants do not
fit. The five-release regression also repeats that stage with no spare variant
rows and verifies the fallback.

The final v0.15.1 GameCube USA build with seed 1520 and forced Pharaohmon/Otamamon
import priority writes and verifies all 2,288,283,648 bytes (MD5
`148b3eaae402e9a0cfc00c292aea3951`). The build log is in the outer workspace's
`outputs/pharaoh-release/full-build-verified.log`. The complete engine,
frontend and desktop checks pass. Wider gameplay, Crown Mod scaling of
scripted GENERATE parameters, and live resource pressure in other stages
remain to be checked.

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
counts, refill modes and drops stay authored. In this earlier rules-17 implementation, the same families were used on all
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

## GameCube Mangrove seed 151 encounter crash

The public v0.15.1 randomizer-only report fails during encounters deeper in
Mangrove, after a successful stage entrance. The system and rendering pools
can fail fresh buffer allocations while other arenas retain free space. The
extended allocator fallback, ownership handling and validation are documented
in [the seed-151 regression](mangrove-seed151.md).

## River-only attack behavior

Randomized river enemies now use their verified firing choices in place of
melee attacks, while retaining movement, idle and lifecycle behavior. See
[river attack policy and validation](river-ranged-ai.md).

## Arukenimon donor

Rules 18 assembles Arukenimon's separately loaded body with its native stage's
attack resources and audio. See [resource preparation and platform validation](arukenimon-donor.md).

## Animation decoder contexts (Mangrove crowd crash)

A GameCube USA v0.19.0 report (seed `1520`, Hard, one player, four-player spawns, Crown Mod, random bosses and crown colours) crashed with "Invalid write to 0x000000a9, PC = 0x803b4260" after a dozen or more start-area enemies stayed alive for minutes. The PC is the constructor of an Enbaya animation decoder context. `igEnbayaContextPool` grows by one context, `(tracks - 1) * 96 + 272` bytes, whenever every context is in use, and its null path still writes the context header; skipping a failed context is not possible, since the pool then returns the element past its list. On GameCube the pool belongs to the system pool: with the start-area crowd 35 contexts held about 140 KB of it. On PS2 the live pool belongs to the general arena (encoded allocator index 10) in the hub and in Mangrove.

The grow routine and the pool destructor (the only allocation and free paths; the constructor has one caller) now resolve encoded index 10 directly (`src/anim_pool.rs`: two words at each site, GC USA/Japan and PS2 USA/Europe/Japan, refused unless every original word matches). PS2 behaviour is unchanged by construction.

Live GameCube USA validation (Dolphin, "Speed Up Disc Transfer Rate" on, cold boot from the native card, Hard): with 25-28 enemies alive all 40 contexts were in the general arena, the system pool kept 230 KB free (about 146 KB unpatched), an eight-minute fight logged no invalid accesses, and six Geko/Mangrove stage loads passed movement and four-allocator free-list checks; contexts were freed and recreated in the general arena across loads. Forced-starvation experiments on the unpatched image did not reproduce the report: blocks freed later refill emptied lists, and re-emptying them every frame breaks allocator bookkeeping. The reporter's own savestate was not available.

### PS2 allocation fallback parity

PS2 contexts already live in the general arena, but a failed general allocation reached the same null constructor write, and PS2 had no fresh-allocation fallback. The PS2 operation hook (`src/ps2_memory.rs`, `operation_code`) now matches GameCube: existing system/rendering/general/stage buffers resolve their owning arena; failed fresh allocations from those pools retry in the general arena (the stage arena for a general request) when the allocator implementation matches; a failed resize allocates the same way, copies at most the old usable size (`igArenaMemoryPool` vfunc 84) with `memcpy`, then frees the old block, leaving it untouched on failure; a guard keeps nested native operations native; nothing runs before the pool table holds the stage entry. The 728-byte hook (plus the optional 344-byte difficulty code) exceeds USA's 720 bytes of ELF sector padding, so random-enemy images append `MODULES/CRI_ADXI.IRX` (34 sectors, loaded by path) after the archives and grow the ELF into its old sectors on all three regions; the RAM reservation is 4 KiB.

Emulated-EE tests cover native success, system/rendering/general/stage retries, unknown pools, a missing table, mismatched implementations, nested guards, borrowed-buffer frees, resize migration with bounded copies and failed migration. Retail checks pass on PS2 USA/Europe/Japan. A headless PCSX2 cold boot of a seed-1520 USA image (Crown Mod, four-player spawns, random bosses and colours) reached the title, loaded the native card save, and crossed Central/Terminal twice with control: 95,188 operations ran through the hook (no retries needed), and all four arenas passed free-list, ownership and overlap checks at every checkpoint. A forced live retry and a PS2 Mangrove crowd were not run.
