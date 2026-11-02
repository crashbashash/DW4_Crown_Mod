# Mangrove Forest seed 151 crash

The report concerns GameCube USA, public v0.15.1, random enemies only, seed
151, with Crown Mod disabled. The supplied state is just before the GekoGeko
Land boat. The later screenshot places the reported crash deeper in Mangrove,
near a fork over water. Entering Mangrove successfully does not validate that
encounter area.

## Failure and correction

The exact public rules-16 build reproduces a native null-buffer fault after
activating encounters beyond the entrance. The controlled route activates the
GENE-00 area for 480 video fields, then the GENE-06 area at `(305, 10, -185)`.
The game stops advancing at field 491. A 4,208-byte system-pool allocation
returns null; its caller nevertheless enters the buffer constructor, which
faults at `0x803b4260` in the USA executable.

The system allocator has only 3,372 free bytes and a largest free block of
2,132 bytes. General and stage arenas still contain 1,543,812 and 942,524 free
bytes respectively, with largest free blocks of 188,880 and 74,504 bytes.
Their free lists, block bounds and non-overlap checks pass. This is allocation
failure under resource pressure, rather than established heap corruption.

Covering system allocations alone gets past that fault but exposes a separate
16,384-byte rendering-pool allocation failure at `0x800738f0`. The final hook
therefore retries failed fresh allocations from both native system pool 0 and
rendering pool 6 in the existing general arena. Native allocation is always
attempted first. The existing scene-arena fallback remains intact.

Later resize/free calls resolve the actual general or stage owner of borrowed
buffers. Zero-size operations, native free operations and custom allocator
policies retain native handling. A guard keeps nested arena/backing allocations
on the native path, preventing retry recursion and reentry into an allocator
whose operation is still in progress. The guard clears after either successful
or failed native calls. Ownership checks wait until the global pool table is
fully initialized, preserving early system-pool startup operations. All code and the guard occupy verified zero executable
padding; exception-handler instructions remain intact.

The independent cold-boot route also exposed a growing existing buffer at the
boat exit: a general-arena realloc failed with only 88 bytes in its largest
free block, while the stage arena still had a 128,304-byte free block. Its caller
stored the null result and attempted to clear the newly added elements.

Existing-buffer fallback now tries native realloc first. After failure it uses
the native allocation-size query, obtains a fresh block through the guarded
scene-arena fallback, copies at most `min(old usable size, new size)`, then
frees the old block through its owner. If allocation fails the old block stays
allocated and untouched. Native allocator code gates its old-block copy/free
on successful allocation, so retrying after its failure retains a valid source.
Tests verify payload preservation, truncation, zero usable size, native success,
allocation failure, custom allocators, reentry and cleanup ordering.

This changes runtime allocation handling, rather than rerolling Mangrove's
enemies or reducing its donor pool. It applies to both GameCube regions.

## Validation

Native PowerPC instruction tests cover system/rendering allocation retries,
existing-buffer ownership, absent tables/candidates, incompatible allocator
classes, native failure, nested calls, absent or partially initialized startup
tables, and preservation of arguments, stack,
return address and the guard. Both retail GameCube executables pass the guarded
patch-region checks, including refusal of modified code/padding and overlapping
region checks. Full frontend, core and desktop checks and devtools Clippy pass.

The public plan was rebuilt with the same digest `64caf8b77852abdc`. The
initial system-only test image's NDMW archive is byte-identical to that public
baseline (SHA-256 `6880fe1818867ad4e4c46dc3ae78e73724e096359cc3da6013d1458816cee1e0`).
Preserved public states were replayed with replacement allocator instructions;
no changed archive metadata was loaded into those states. This isolates the
allocator change from the newer river policy.

The system/rendering fallback completed 22 sampled encounter areas across
Mangrove, including Pharaohmon summons and the original failure point, then
three GekoGeko–Mangrove return trips (six stage loads). Movement and all four
relevant allocators' free-list/block invariants passed after each load.

The final production image cold-booted from a native memory-card character
on Hard, without loading any savestate or writing executable RAM. It entered
GekoGeko Land and Mangrove, completed all 22 sampled encounter areas, and then
three complete return trips (six further stage loads). Movement and all four
allocator-invariant checks passed after each load. The allocation guard was 0
at the final checkpoint. The isolated Dolphin process closed successfully.

The earlier preserved-state migration replay independently passed the exact
previously failing boat exit and six subsequent stage loads. All seven runtime
patch regions in the final production image match that successful replay.

## Evidence

Private workspace evidence: `outputs/mangrove-seed151/`. It contains the public
and current production build logs, preserved failure states, native captures,
allocator snapshots, guarded replay, and cold-boot route checks. The old
`public-mangrove-fault` filenames resulted from an obsolete diagnostic limit
on arena span counts; those files are healthy entrance states. The actual
fault is `explore-fault`, with the second failure in `fixed-explore-fault`.

The final rules-17 randomizer-only production image is 2,285,017,088 bytes,
MD5 `1cae7c455a1a30d438a3b053f0dc1c37`, plan digest `d8e9958a7f5203d1`.
Its private filename is `seed151-randomizer-fixed.iso`; full evidence includes
`validated-production-build.log`, `verified-result.json`, all native checkpoints
and screenshots, and the six `verified-cleanup-*-pools.json` reports.
The changed full-plan digest includes the separately validated river policy.
