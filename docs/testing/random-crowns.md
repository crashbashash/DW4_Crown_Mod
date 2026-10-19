# Random crown colours

The optional **Random crown colours** setting starts disabled. When enabled,
each ordinary crowned enemy rolls its icon colour when it spawns: green, blue,
pink, white or yellow. Two enemies of the same type can have different colours;
colours may repeat and remain fixed for that enemy's lifetime. Boss families
keep yellow. Other uncrowned enemies and props stay uncrowned, including
the tutorial's otherwise unbuffed Kokatorimon.

Numenume River's eight missile turrets are also crowned. Their cosmetic colour
is chosen when each turret is generated and stays fixed until destruction.
They use a private Lua generator rather than the enemy icon hook, because
turrets have no enemy-table rarity row. It consumes no game RNG calls. See
[river turrets](river-turrets.md) for their damage and lifecycle checks.

This option changes the displayed icon only. It does not remap enemy rows,
change HP, stats, critical/stun values, or choose different drop tables. It works
independently of the spawn-count and custom-drop settings.

## Implementation

Ordinary crowned rows use an internal icon marker in the existing RARITY byte.
A hook at the icon-argument push converts that marker to a colour. Other rank
values pass through unchanged. The general rank getter and messaging functions
are left intact. Boss rows retain rank 5.

The hook uses a private 32-bit generator, seeded once from the operating system
when patching. It does not consume the game's random stream. The seed is shared
through the complete container pipeline, including compressed-output verification.
An explicit seed is available to core callers for reproducible checks.

PS2 uses an EE hook; GameCube uses a PowerPC hook. Both validate the original
call context and unused padding before writing. The feature supports all five
retail releases listed in the README. No executable growth is needed.

## Verification

- Both emitted instruction sequences are executed by small instruction models
  over 10,000 spawns. Their results match the private generator; ordinary ranks
  0–5 pass through without advancing it. Messenger, stack and return values are
  preserved, and memory access is confined to the private seed.
- Full colour-enabled images were created from PS2 USA, Europe and Japan and
  GameCube USA and Japan inputs. Independent audits found identical default
  Crown Mod HP, stats and critical/stun bytes, 413 ordinary marked rows, yellow
  boss rows, and no unrelated executable edits.
- A USA PS2 cold boot loaded the patched call and seed. A controlled private
  pre-spawn state then exercised three identical Goburimon with the hook and
  cosmetic table bytes applied, retaining the state's combat data. Three spawns
  advanced the private generator exactly three times and produced ranks 1, 5
  and 4. The game displayed separate colours on the identical enemies.
- After the gate sequence, the live Goblin Fortress boss retained its yellow
  crown with the hook active. Its private generator remained at the initial
  seed, and the live boss retained row 60 and 41,522 HP. An in-arena state was
  saved for subsequent tests.

The pre-spawn state check isolates runtime icon behaviour; it is not a cold-boot
stage-loading or complete playthrough claim. GameCube and the other PS2 regions
have full-image/static and instruction-model coverage, without a corresponding
live boss encounter.

The opt-in `tests/resource_options.rs` matrix includes all three settings enabled
together, in addition to the original spawn/drop combinations. Each output is
unpacked where needed and all 1,995 transformed table blocks are checked.
The combined-settings case passed through a USA GameCube RVZ input and output,
including container round-trip verification. With the explicit test seed
`0x72abcdef`, the output is 990,853,895 bytes with MD5
`11daa61c1ccf51e647697c6383be11b3`. Other compressed colour combinations have not
been separately exercised.
