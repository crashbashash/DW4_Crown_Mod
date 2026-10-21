# Four-player spawn counts and practice Speed

The spawn option is off by default. It changes enemy generator totals and
concurrent-enemy caps to each generator's authored fourth-player-count profile.
It does not change the selected enemy row, HP, attack, defence, Speed, experience,
per-kill drops, real party size, stage scripts or destructible counts.
Scripted encounter choices still follow the actual party size, with one
exception: the practice stage's farewell enemies (below). This option does not
reproduce every other script branch of a real four-player party.

## Practice-stage farewell pair

After the rocks-and-bushes lesson, the practice script (`NDMW.AFS` entry 607,
the one Lua chunk containing `GENE-a05`, `GENE-b06`, `GBOX-02` and
`SYSgetPlayerNinzu`) chooses the last enemies from the real party size.
Retail compares `SYSgetPlayerNinzu() <= 2` (GENE-a03/a04 or b03/b04) and
`>= 3` (GENE-a05/a06 or b05/b06). The a06/b06 generators spawn the
Kokatorimon (rows 644/645), so with fewer than three joined players it never
appears, and the generator counts above cannot change that.

With the spawn option on, the four comparison operands become `<= 0` and `>= 1`
so any party takes the three-or-more branch. Nothing else in the chunk changes:
same length, same instructions, no new IDs. Flag 382 still picks the a or b
group. The build refuses to continue if the chunk is missing, appears more than
once or does not have the retail operands `2, 3, 2, 3`.

The ordinary generator spawn path passes the selected enemy row and that row's
HPMAX to the enemy model. The per-party profiles hold two count fields and two
drop-list references, rather than a universal four-player stat multiplier.
There is no custom stat-boost child setting.

The separate practice balance change halves stat column 4 (Speed) on rows
638–645 after the ordinary practice buffs, using the patcher's existing
ties-to-even integer rounding. It affects both Kokatorimon variants' Speed;
their other stats retain their original values. The low-level
`practice_speed_mult: 1.0` setting preserves historical output fingerprints.

| Practice row | Previous Speed | New Speed |
| --- | ---: | ---: |
| 638 | 310 | 155 |
| 639 | 335 | 168 |
| 640 | 341 | 170 |
| 641 | 372 | 186 |
| 642 | 316 | 158 |
| 643 | 322 | 161 |
| 644 | 65 | 32 |
| 645 | 65 | 32 |

## Resource implementation

`beGeneraterData._playerDataList` selects one of four
`beGeneraterPlayerData` records. Only `_geneCount` and `_geneMaxCount` are copied.
Modified profiles and lists are private clones because originals can be shared
with generators needing different four-player counts or with destructibles.
Both drop references stay attached to the source party-size profile.
The existing archive rebuild relocates the enlarged packs and validates their
original IDs, original memory pools, and every unchanged original object.

All five supported releases were inspected with the option off and on: PS2
USA/Europe/Japan and GameCube USA/Japan. Their generator inventories agree:
10,296 generators, including 7,567 enemy generators. The option changes 1,419
enemy generators, covering 3,717 party-size profiles in 132 packs. The rebuilt
archive grows by 196,608 bytes. The other 840 archive entries remain byte-identical.

Independent read-back of a PS2 USA test ISO checked every changed generator,
preserved drop references, unchanged enemy rows, unchanged destructibles, and
all 665 stat blocks. The only stat differences were the eight practice Speed
values. Executable and unrelated original disc bytes were unchanged.

Examples: the three opening practice enemy generators change from counts
`1 / 3 / 3 / 4` to `4 / 4 / 4 / 4`; Goblin Pass `GENE-00` changes from
`2 / 3 / 4 / 5` to `5 / 5 / 5 / 5`.

Generated-format tests cover shared profiles with conflicting target counts,
unchanged drops and crates, incomplete-profile refusal, and no-op reapplication.
Frontend tests cover option forwarding, overwrite handling, accurate reinspection,
preservation of the chosen output and rejection of stale analysis results.
The crown-drop control now independently applies the [custom loot rebalance](custom-drops.md).
The spawn-only audits and fingerprints on this page keep that option off.

The native GUI produced the expected spawn-only four-player ISO. Overwrite
cancellation and confirmation were exercised. The published v0.9 USA checksum
still reproduces with `practice_speed_mult: 1.0` and both resource switches off.

## Spawn-only PS2 USA output fingerprints

These include the new practice Speed adjustment, with custom drops off. Historical release checksums
in the changelog remain unchanged.

| Four-player spawns | Bytes | MD5 |
| --- | ---: | --- |
| Off | 2,179,188,736 | `73ac2e226a8a3fa504de9740ed9d446c` |
| On | 2,179,385,344 | `77575af67890466e297ca083b20d01ec` |

The common patched stat block has MD5 `c2cdbb0ca237b6619d55374bd4ed86e5`.

The [full release/container matrix](resource-options.md) also created and verified
all four option combinations for each supported raw release, plus compressed
GameCube builds with both switches enabled.
