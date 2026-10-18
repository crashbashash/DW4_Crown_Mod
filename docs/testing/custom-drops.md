# Optional custom crown drops

The **Match drops to crowns** switch applies a custom category rebalance when
building a disc. It starts off. Crown display and enemy stats do not select loot;
the patch rewrites the ordinary and final drop references on enemy generators.
It is independent of the four-player enemy-count switch and applies to all four
party-size profiles.

| Per-kill category | Normal | Hard | Very Hard |
| --- | ---: | ---: | ---: |
| Equipment or ability chips | 50% | 50% | 50% |
| x-data | 15% | 20% | 25% |
| Stat / elemental plugs | 2% | 4% | 6% |
| BIT or consumables | 23% | 16% | 14% |
| Nothing | 10% | 10% | 5% |

These are category probabilities, not independent rolls. One ordinary loot roll
selects at most one item. More enemy spawns naturally produce more total rewards.

## Item pools and first-version exceptions

- Existing categories retain their authored item IDs, ranks and relative weights
  (rounded using largest remainders). Equipment quality and x-data pickup quantities
  receive no extra multiplier. Difficulty still selects the game's authored pools.
- Missing categories first borrow a matching difficulty/area pool in the stage
  pack, then the same area library elsewhere on the disc. The earliest Goburimon
  pool is preferred when available, including for missing tutorial equipment.
  Existing equipment pools never receive additional equipment IDs.
- Equipment never falls back to another area's library. Missing non-equipment
  categories in quest/boss-area libraries can use basic World 1 pickups.
- Some area/difficulty libraries contain no plugs. Those use a lower difficulty
  in the same area where possible, then basic Normal World 1 plugs. This changes
  plug availability; no claim about their permanence or effects is required by
  the patch, and no pickup-effect code is altered.
- Empty/no-drop tables, common quest/epic reward tables, unsupported special item
  categories and tables already totaling at least 10,000 retain their original
  references. This preserves existing guaranteed boss/tutorial rewards and special
  final-enemy rewards. Other final tables receive the same rebalance as ordinary
  tables. Scripted perfect-drop behavior is unchanged.
- Natural crowns receive the same category targets on eligible tables. Their
  existing item selection is retained. No additional natural-crown bonus or
  blanket guarantee is introduced in this first version.
- Crates, barrels and other excluded destructible generators retain their tables,
  even when their original profiles/tables are shared with enemies.

## Implementation and verification

The patch clones entries, item lists, difficulty lists and party profiles, then
repoints enemy generators. Original objects and memory remain intact except the
selected generator references. Both option combinations preserve enemy counts as
selected by the separate spawn switch. The native analysis uses the same resource
options to calculate output size; rapid toggle changes discard stale results.

The retail PS2 roll is `r = RNG % 10000`, followed by subtracting each weight and
selecting when `r <= 0` (`FUN_00384800`, subtraction at `0x3849f4`, branch at
`0x3849f8`). The first entry therefore owns the extra zero outcome. New tables
compensate by subtracting one from the first entry's stored weight, so category
probabilities and the no-drop remainder match the table above exactly. Existing
exempt tables are not adjusted. Unit tests exhaust all 10,000 outcomes for each
difficulty, including small equipment weights.

## Validation

- Frontend: typecheck, production build, tests and lint pass.
- Rust workspace tests and strict Clippy pass; native strict Clippy/build pass.
- The [full release/container matrix](resource-options.md) completed 24 full disc
  builds: all four option combinations on all five supported raw releases, plus
  USA/Japanese GameCube CISO and RVZ builds with both switches enabled.
- Independent USA audits compare both enabled ISOs against their corresponding
  drops-off builds: 217 generator packs change, 755 archive entries stay identical.
  All 2,142 eligible table/difficulty combinations have exact category rates.
  All 2,715 destructible/gimmick generators in the changed packs stay unchanged.
  Original shared loot objects, quest/empty/guaranteed table references, party
  spawn counts, stats, executable and unrelated data remain intact.
- The real native GUI, with only custom drops enabled, produced the same solo
  output fingerprint through its overwrite-and-patch flow. The GUI was reopened
  and its controls checked.
- The published USA v0.9 output is still reproduced byte-for-byte with optional
  patches off and the historical practice-Speed multiplier selected.
- Synthetic graph tests cover a crate sharing an enemy's entire loot graph and a
  single table shared by all difficulties. Exhaustive roll tests cover all three
  rate distributions; donor tests cover missing categories and plug fallbacks.

USA ISO fingerprints (current half-Speed practice balance):

| Custom drops | Four-player spawns | Output bytes | MD5 |
| --- | --- | ---: | --- |
| On | Off | 2,183,974,912 | `8a5f52880805febb25be46c1cc1de1ac` |
| On | On | 2,184,189,952 | `923cdc2a1893624b9290ed9c196a3242` |

These are disc-data and application tests. New loot spawning and gameplay balance
have not been verified in a running emulator.
