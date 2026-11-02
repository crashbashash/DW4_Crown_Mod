# River-only ranged attack choices

When enemy randomization is enabled, Numenume River restricts the offensive
choices of its thirteen projectile-capable enemy families to verified firing
sequences. Ordinary movement and idle behavior remain available. Required
activation, damage/status reactions, AI-map transitions, hiding and death
continue to use native behavior.

The change applies to the final river model pack (NDMW entry 628), after donor
imports, unused-donor pruning and capacity expansion. It does not edit donor
sources or models in any other stage. It also covers retained Numemon when a
replacement cannot be imported. With enemy randomization disabled, this policy
is not applied.

## Implementation

Each family has an explicit native firing-sequence policy. AI decision records
that selected melee or other unsuitable offensive sequences now select one of
that family's verified firing sequences. Their movement parameters and selection
weights retain their original bytes. MR05 additionally uses wider river-only
range bands: its active/firing band reaches 80 units instead of 30, and its
shield mode releases within the same 80-unit band. Both maps use a 120-unit
outer band while retaining the native 190-unit perception limit and turn rate.
This prevents the boat/shore gap from keeping it in shield mode until the boat
is extremely close. Other families and other stages retain their original
range settings. Infermon's voluntary
bind/part checks select idle rather than invoking its part/transform callback.

The original unsuitable offensive sequence entry points also reference the
native firing command list. This prevents a nested transition or callback from
bypassing the decision-table restriction. Entry labels must remain valid; for
example, Infermon retains both its `NORM` and `BIND` firing branches. Projectile
models, action bindings, animation/event timing, hitboxes, sounds and hit/death
scripts remain unchanged. Applying the policy twice changes nothing further.

This changes attack behavior within the existing one-family river selection.
It does not introduce a mixed-family river encounter or alter enemy strength,
rewards, spawn counts or turret behavior.

## Disc-data validation

Checks pass for all thirteen families on PS2 USA, Europe and Japan, and
GameCube USA and Japan. They verify:

- No restricted offensive choice remains in each enemy's AI tables.
- Every restricted sequence entry redirects to its native firing command list.
- Every retained offensive animation action has a registered spawn binding to an
  independently moving projectile with a damaging hitbox, including additional
  Minotaurmon and shielded Raremon firing variants.
- Allowed firing and lifecycle sequences retain their original records.
- Other fields, scripts, geometry/animation resources and memory blocks remain unchanged.
- MR05's two AI maps have the widened near/outer bands; their perception and
  turn fields and all other models' range fields remain unchanged.
- Non-river packs remain byte-identical, and the operation is idempotent.

The complete application checker and devtools Clippy pass separately from the
configured five-release data checks. Initial PS2 runtime coverage is recorded
below.

## Dolphin validation

Focused GameCube USA images test Sukamon, Minotaurmon, MR04 and Infermon. Each
image cold-boots from a copied native Hard memory card, with no old savestate
loaded. The running model's AI tables and offensive-entry command-list pointers
are inspected after loading the river, verifying that the image's restrictions
are present in the actual running game.

The observation helper samples active enemy sequences every six video fields
and observes native projectile spawning and movement. Player HP is refilled;
earlier runs retain the normal scripted boat damage and use native failure
exits to check unloading. The final protected MR04 check raises the real Lua
boat-health counter during observation, then uses completed bomb flags and
player positioning to exercise the native victory exit. These are assisted
checks, not unassisted river playthroughs. Projectile positions and sequence
cursors are not changed. Native spawn, hiding and movement states are recorded
separately from attacks.

Boat durability is stored in the Lua `g_boathp` global. Refilling the displayed
meter alone does not restore that durability.

Private evidence is in `outputs/river-ranged-ai/` in the outer workspace. Test
images use a forced donor for coverage and are not normal public seed builds.

All four final observation cases pass: Sukamon, Minotaurmon, MR04 and Infermon
each fire a moving native projectile without an observed melee sequence. Each
case samples 5,400 video fields. Enemy firing choices, movement, idle and normal
lifecycle states remain active, and subsequent stage unloading preserves
movement and allocator invariants. No guarded fallback alias is needed during
these observation traces.

The final protected MR04 run retains the real boat-health counter above
999,000 through observation and exits through the native victory transition
to Cliff (`stag0104`) with the assisted objective/position setup described
above. The four system, rendering, general and stage allocator checks pass
after that exit, and all test Dolphin instances close successfully. Earlier
MR04 diagnostic runs with incomplete stage-transition sampling are retained
privately; `mecha4-protected-result.json` records the final protected run.

## Remaining-family Dolphin coverage and MR05 range correction

The remaining nine families also receive focused GameCube USA cold-boot checks:
Numemon, MR02, Otamamon, MR03, MarineDevimon, Megadramon, Raremon, MR05 and
Imperialdramon. Numemon uses a native-family image with the river AI transform;
the other eight use separately verified single-donor randomized river images.
The observation validates firing bindings from each written image, including
Megadramon's `SQ_BULT` flight and the separate moving MR05 projectile branch.

Each family produces an independently moving native projectile, records no
melee sequence, and reaches the assisted native victory exit to Cliff with
working player movement and valid system, rendering, general and stage arenas.
Each additional observation spans at least 1,800 video fields. Player and real
boat health are protected from river entry; bomb-objective flags and positioning
assist the exit. No projectile positions or attack cursors are edited. These
checks establish firing and lifecycle behavior, not unassisted river balance.

The MR05 baseline exposes a gameplay limitation: it repeatedly changes shield
state and fires only when the boat is very close. Its native active map has a
30-unit firing band, while the shield map releases only within 40 units. The
river-specific correction widens both near bands to 80 units and both outer
bands to 120, retaining the 190-unit perception limit, native choice weights,
turn rates, firing animation and projectile data. MR05's normal stages retain
their original bands. All five release data checks verify this distinction.

The corrected live MR05 run verifies both maps at squared distances
`6400 / 14400 / 36100`, fires within the first observation interval, and records
repeated firing starts roughly 60–78 units from the boat. It produces moving
shots without melee and completes the assisted Cliff exit with valid allocators.
`mecha5-range-result.json` supersedes the earlier narrow-range baseline.

All thirteen river families now have live Dolphin firing and exit coverage for
the ranged-only policy. The private `all-thirteen-result.json` indexes the final
case records. Complete application checks, devtools Clippy and the five-release
projectile/range audit pass after the MR05 correction. All test Dolphin
processes close successfully.

## PCSX2 validation

PS2 USA Numemon and imported Sukamon images also pass native Hard-card
cold-boot checks without loading an old savestate. Both verify the loaded AI
choices and offensive entry guards, produce moving native projectiles without
an observed melee sequence, and reach the assisted native victory exit to
Cliff. Both PS2 allocator arenas retain valid circular, bounded and
non-overlapping free lists, and player movement works after unloading.

Observations span at least 1,446 video fields per family. Player and real boat
health protection and the objective/position assistance match the later
Dolphin checks; projectile positions and sequence cursors remain native. These
are assisted firing/lifecycle checks rather than unassisted balance tests.
Each case closes its test emulator after validation.

Private PS2 evidence is under `outputs/river-ranged-ai/ps2/` in the outer
workspace. The test driver automates the native menus and stage route, and
subsequent launches use PCSX2 fast-forward with bounded controller frame counts.
