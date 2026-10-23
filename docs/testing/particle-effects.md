# Imported particle render state

MR02's blast/puddle chain (`y_014eblst01a`, `y_014eblst01b`,
`x_014eblst01b`) and MR03's mouth fire (`y_021eblst01b`) use inherited
render attributes. The particle switches inherit texture bindings from a parent
`PromotedAttr`; further ancestors provide alpha testing, blending and effect-pass
state. MR02's companion effect model also inherits its effect-pass settings.
These resources are not owned by the effect switches themselves.

The old importer copied the switch and attached it to a destination branch by
child index. It omitted the original texture-binding parent, and its comparison
of render state excluded referenced attributes. Even an apparently exact match
could therefore use an unrelated destination texture or blend function.

Rules 7 copies the selected effect's complete attribute ancestry below the
matching destination effect pass. Shared source branches are pruned to selected
children, preserving their original order; unused child-memory entries contain
null IDs. This avoids importing unselected siblings. Multiple effects share
their copied ancestry within one import, while later imports retain separate
native attribute stacks. Destination effects keep their existing resources.

Donor class discovery uses the same pruned source view, so required texture and
render-attribute classes are included in layout compatibility checks. Copies
still pass the existing object/reference/memory verification. Unsupported render
passes are reported rather than attached approximately.

## Regression

```sh
cargo test --release --lib imported_particles_keep_native -- --nocapture
```

The clean-image variables in the development guide enable the five supported
releases. Each case imports Scumon, MR03 and then MR02 into Goblin Pass and Abyss
of Grief. Scumon's projectile is an additional particle case. The test compares
each ancestor's complete attribute resource graph against the
native source, including texture pixels, sampler settings, alpha and blend
parameters, independent of local object IDs. It also verifies that native and
previously imported effects retain their render state, with no approximate or
missing attachments.

The regression fails on the original importer at the first texture-binding
ancestor. The correction passes on PS2 USA, Europe and Japan, and GameCube USA
and Japan. The broader PS2 USA/GameCube USA enemy audio, shared-capacity and
scripted-spawn regressions also passed.

## Runtime and complete-image checks

A GameCube USA test image was cold-booted and entered Goblin Pass, then Abyss of
Grief through its normal exit. In the imported MR02 encounter, its own AI selected
`SQ_FIRE` after 96 video fields at firing range. The resulting particle/puddle
sequence was captured and visually approved. Positions and generator activation
were controlled to reproduce the attack; the accepted capture uses native attack
selection, without changing sequence cursors or attack data.

MR03's native AI selected its missile attack (`SQ_ATCK1`) after 48 video fields
at firing range. The subsequent capture shows the orange mouth fire and emitted
missile. Player/camera positions were controlled for visibility; sequence cursors
and attack data were unchanged. Its resources also pass the native texture and
attribute comparison above.

Complete PS2 USA and GameCube USA images for seed `1278675365` built and passed
the patcher's output verification with the particle and Mangrove corrections.
The Mangrove regression covers every supported release; its full boss encounter
has not been replayed live. These are local validation builds, not release assets.
