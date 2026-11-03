# Numenume River enemy scenes

The later [river-only ranged attack policy](river-ranged-ai.md) restricts attack
choices as well as enemy eligibility. The resource-adapter checks below describe
the earlier behavior with unmodified enemy AI.

Rules 17 restricts river replacements to projectile-capable families. A separate
scene-layout limitation previously excluded seven of those families: Sukamon,
Minotaurmon, MR03, MR04, Raremon, MR05 and Imperialdramon.

## Adapter

The river stores Numemon's model switch directly in its own `igSceneInfo`.
Ordinary donor stages instead attach their enemy switches beneath an `EM01`
group. The importer correctly refused those models when the destination lacked
that group; removing the refusal alone would leave their models undrawn.

The river adapter copies a plain `EM01` group with an empty child list from the
native Humid Cave B2 pack. It wraps the existing Numemon switch in that group,
keeping the same native scene. Ordinary grouped donors can then use the existing
import, rendering, audio and capacity paths. Standalone-only donor variants are
still refused; all twelve nonresident river families have a grouped source.

The adapter adds four directory IDs and 172 serialized bytes on PS2 USA, Europe
and Japan, and GameCube USA and Japan. It copies no donor actors, geometry,
textures or world scene. The regression verifies all original object fields and
memory blocks remain byte-identical except for the native scene's root pointer,
and that applying the adapter twice changes nothing.

The later [mixed river encounter policy](river-mixed-enemies.md) distributes
banks across up to three ranged families and extends all six erase/freeze
scripts to those families. Entrances, encounter strength, rarity, rewards and
party counts keep their existing compatibility checks. The ranged policy applies
to both resident replacement candidates and donor imports.

## Disc-data checks

- `river_scene_adapter_preserves_native_resources_on_every_release` passes on
  all five supported releases.
- `river_replacements_use_only_ranged_families` forces each of the seven formerly
  blocked families independently on PS2 USA and GameCube USA, checking 378
  generator records across difficulties for each replacement. Native entrances,
  atomic family selection and incompatible-donor fallback are also checked.
- `river_pool_has_native_moving_projectile_spawns` verifies a native firing
  action, registered spawn binding, projectile movement and damaging hitbox for
  every allowed family. It distinguishes projectiles from companion effects.

## GameCube runtime checks

Each formerly blocked family was loaded into the river in Dolphin from its own
verified test image. Tests used a copied native level-316 Hard save. Initial
runs cold-booted; diagnostic reruns used a hub checkpoint from the exact same
image, with the river loaded afterward. Hub portal destination changes provided
stage access. The test did not replace enemy AI or attack sequences.

All seven rendered, fired a native projectile, registered a projectile collision
against the boat player, and returned through the game's river exit to Abyss of
Grief with a controllable player and valid allocator free lists:

| Family | Collision observation |
| --- | --- |
| Sukamon | Boat meter decreased from 50 to 47.5 |
| Minotaurmon | Native `SQ_GUARDR` reaction |
| MR03 | Native `SQ_GUARDR` reaction |
| MR04 | Moving `SQ_BLST01a` shot decreased boat meter from 50 to 47.5 |
| Raremon | Native `SQ_GUARDR` reaction |
| MR05 | Moving `SQ_BLST01a` shot produced native `SQ_SDMG` reaction |
| Imperialdramon | Moving `SQ_BLST02` shot decreased boat meter from 50 to 47.5 |

Collision isolation positioned an already fired projectile at the player on the
boat. This verifies the real collision/reaction path, not natural hit accuracy
throughout the river. MR04/MR05 cosmetic branches were excluded from that check.
The high-level test character can guard attacks; a guarded reaction is recorded
separately from boat damage. No attack data, sequence cursor or projectile
hitbox was patched. Normal boat damage was also observed before isolation in
the MR05 run.

Full-route checks exercised the bomb/freeze flag 489 with Sukamon, Minotaurmon,
MR03 and Raremon. Other runs also exercised the native boat-failure exit. Three
consecutive MR04 river visits completed without rewinding a savestate. The two
warmed returns changed allocator usage by -42,216 and +43,496 bytes in the two
arenas (1,280 bytes combined), rather than retaining an imported model pack.
Circular free lists, owning-arena bounds and non-overlapping free blocks passed.
Dolphin logs reported no invalid reads or writes.

An older PS2 debug savestate retained the original archive lookup data even
when the target river was loaded afterward. That was detected by inspecting the
loaded model names. Cross-image debug states are not accepted as proof of this
change; cold boot or use a before-river checkpoint from the exact same image.

Private images, native snapshots, captures and detailed logs are in the outer
workspace's `outputs/river-scene-adapter/`. They are not repository assets.

## Complete app-pipeline image

A GameCube USA image built with the normal `patch_file` pipeline, Crown Mod
and random enemies enabled, and seed 67 verified all 2,279,456,768 output bytes
(MD5 `f7298c4fbda708a4696bfb17db103e1e`). Its audit records 72 stages, 225 copied
models, and zero approximate or undrawn effects. A cold boot of this complete
image loaded the river's Sukamon replacement, produced a native projectile and
boat damage, and returned to Abyss of Grief with valid allocator free lists.
This exercises the scene adapter together with Crown Mod resources and balance,
in addition to the isolated per-family images above.

## PS2 runtime smoke test

A PS2 USA test image cold-booted in PCSX2 using a copied native level-999 Normal
save. A test-only hub portal destination led to the river. The running stage
contained the imported Sukamon pool (126 instance slots) and its projectile
pool (six slots), rather than the original Numemon-only resources. Sukamon and
its projectile rendered, isolated projectile contact produced `SQ_GUARDR`, and
the game returned to Abyss of Grief with a controllable player. Both allocator
arenas had valid circular free lists after the exit. This is a PS2 smoke test;
individual live checks of all seven imported families used GameCube USA.

Both emulators were closed after testing. The complete core, frontend and
desktop checker and the devtools Clippy check pass. Generated encounter rules
include the ranged pool and shared-scene adaptation.
