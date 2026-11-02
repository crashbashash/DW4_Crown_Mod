# Arukenimon random enemy donor

Rules 18 adds Arukenimon (`e_arachne`) to the ordinary random enemy pool.
Its original Grief Wasteland encounter remains protected. It is not added to
the confirmed ranged river pool: the runtime probe has not yet captured a web
projectile hitting the player.

## Resource preparation

The home-stage model pack (AFS 649) contains the hit script, web projectile
`x_009eblst01a` and attack effects, but the body is loaded separately from AFS
650. The body alone lacks the home-stage attack dependencies and an ordinary
shared enemy group. Audio is in AFS 648, rather than immediately before the
standalone body.

The randomizer assembles a complete donor in memory: attach the body to a
compatible shared group, import its registered rendering graph into the home
stage, and let the normal importer copy the complete dependency closure into
each destination. The destination uses ordinary embedded resources and does
not run the home stage's separate-file loading callback. This also applies to
subsequent pruning, reimporting and companion-model audio discovery.

The adapter refuses incompatible class layouts, missing attack resources,
approximate rendering links and unattached rendering resources. It retains the
native hit script. Independent actor capacity expands to the required total
for the randomized assignments; the original standalone file remains at one
actor. Arukenimon provides `SQ_ENTR`, so randomized activations use that native
sequence instead of requesting an absent `SQ_NENTR` or `SQ_SWON`.

## Validation

Real-disc checks pass for PS2 USA, Europe and Japan, and GameCube USA and Japan.
They verify the complete prepared source, native hit-script preservation,
web-projectile companion discovery, audio selection, protection of the native
encounter, and independent expansion from one to 48 actor slots. Prototype
imports pass into five ordinary destination stages, including GekoGeko Land
and Mangrove, on all five releases. Resource compatibility alone does not
establish ranged combat eligibility.

The Dolphin USA ordinary-stage test cold-boots the focused image from a native
memory card, without loading an older savestate. Multiple Arukenimon render,
animate and move independently, with separate model nodes and HP cells. A
normal player attack kills one while another stays alive. Four subsequent
area loads, returning through Goblin Pass and Humid Cave, preserve gameplay and
pass the system, rendering, general and stage allocator invariants.

The Dolphin USA mixed Mangrove image uses seed 2 with Arukenimon, Pharaohmon,
Infermon and Mecha Rogue 03 donors across GekoGeko Land and Mangrove. It
allocates 74 independent Arukenimon actor slots. A fresh native-card boot on
Hard passes all 22 sampled Mangrove encounter areas, including the original
seed-151 failure location. Four native allocator invariants pass at every
checkpoint. The Arukenimon pre-boss wave clears and starts the next wave using
the normal death and generator callbacks. The following wave and native ShogunGekomon
also clear, setting the native completion flag 371. The boss initializes at
5,400/5,400 HP; its rendered health bar responds to unmodified attacks, which
reduce HP to 4,477. The initially grey bar becomes visible after approaching the boss spawn and
attacking; the native boss HP is initialized throughout the check. Combat testing uses player healing,
player/target positioning and reduced target HP followed by normal attacks to accelerate the
encounter; it is not an unassisted playthrough.

After the encounter, three complete GekoGeko–Mangrove return trips (six stage
loads) preserve movement and pass all four allocator checks after every load.
The allocation guard is zero at the final checkpoint, and the isolated Dolphin
instance closes successfully. Video-field input remains bounded; the test
profile's frame limiter is disabled.

The PS2 USA mixed Mangrove image independently cold-boots from a native card
on Normal, without loading a prior savestate. It passes all 22 encounter areas,
the Arukenimon wave, the following wave and the native boss sequence, reaching
completion flag 371 and preserving player movement. Combat uses native maximum
player HP and reduced target HP followed by normal attacks. The helper tops
player HP up to the card character's native maximum of 99,999.

After the boss, three complete GekoGeko–Mangrove return trips (six stage loads)
preserve movement and pass both relevant PS2 allocator checks. Re-entering the
first Arukenimon encounter after the final load renders two live Arukenimon
again, with native HP and separate model/health instances. The isolated PCSX2
process closes successfully with exit code zero. Both emulator test instances
are closed at the end of validation.

The PS2 allocator diagnostic explicitly excludes its inline, zero-tag empty-top
sentinel from free blocks. The first boat exit exposed that missing diagnostic
case, not a game fault: the pointer is the first list descriptor inside the
allocator core. Actual free blocks still pass circular-list, ownership-span and
non-overlap checks; the saved game snapshots were rechecked with that distinction.

Private evidence is in `outputs/arukenimon-donor/` in the outer workspace.
Focused QA images use a forced donor list and are not public seed reproductions.

The ordinary app pipeline also builds a complete, unforced GameCube USA
randomizer-only seed-151 image under rules 18: 2,279,747,584 bytes, MD5
`6f02c4cdc06d19ff42ad992c4c9b06bb`, plan digest `1c490134f4c5b685`.
It reports no approximate or undrawn effects. The allocator code and guard
match the focused image used for native Mangrove testing.
