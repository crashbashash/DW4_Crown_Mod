# Imported Imperialdramon height

Imperialdramon (`e_impdra`) carries a flying model origin and body collision
offset of 20.5 units. Imported into a ground encounter, its body and collision
sit above the player's ordinary attack height.

The rules-6 randomizer lowers the imported actor basis by 13 units and translates
its actor/skin bounds, bullet-reception boxes and body hitbox commands by the
same amount. Bone animation data, projectile models, enemy stats, generators,
terrain collision and movement parameters remain unchanged. Native encounters
are excluded using the stage's original resident-model inventory.

The native body box spans Y=13..28. A 20-unit trial placed too much of the model
inside the floor. The final 13-unit correction raises that trial by 7 units and
puts the body box at Y=0..15, centred at 7.5, while preserving its size.

A spawn-position adjustment alone is insufficient: a live test lowered two
actor roots from terrain height 50 to 30, and movement restored both to 50 within
154 video fields. The persistent correction therefore uses the actor's existing
model-space transform, keeping the actor root on the terrain.

The correction runs after enemy instance capacity is expanded. All imported
instances, including additional clones, receive the same adjustment. Every edit
is a four-byte float change in a recognized field. Object IDs, references,
memory blocks, hitbox dimensions and animation clips stay intact. Unexpected
layouts and repeated application are refused.

## Data regression

```sh
cargo test --release --lib crown::height -- --nocapture
```

The test accepts the five clean-image variables documented in the development
guide. On each release it imports Imperialdramon into Goblin Pass, expands its
pool from four to eight, applies the correction and checks every edited float.
It verifies unchanged bytes outside those fields, unchanged IDs and memory,
all eight actor transforms and refusal to apply the correction twice.

All five releases passed. A complete GameCube USA seed-7 build also passed image
verification. Comparing rules 5 and 6 preserved all 1,848 generator assignments,
the 69-stage plan, imported model counts and plan digest `5a42c74aa11e35b6`.
The controlled build prioritized Imperialdramon donors to reproduce the issue;
this test preference is not part of the shipped app.

## Runtime checks

The 13-unit model height was previewed in a live GameCube encounter and visually
approved after the 20-unit trial. All four allocated actor transforms received
the same correction. The terrain roots remained at Y=50, with the incoming body
box at Y=0..15 and sequence hitbox centres at Y=7.5 relative to those roots.

A controlled melee check at this height reduced both active Imperialdramon HP
values from 6,864 to 1,037 and 632. Enemy combat stats were temporarily reduced
and the player made invulnerable to isolate hit detection; these test changes
are absent from the patch. This verifies that ground attacks can connect, rather
than comparing damage balance against the unmodified encounter.

The final rebuilt image uses the approved 13-unit correction. Its archive layout
matches the rules-5 build: only the 50 imported Imperialdramon model packs change;
generator packs and the native Imperialdramon pack remain byte-identical.
