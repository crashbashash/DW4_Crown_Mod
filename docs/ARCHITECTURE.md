# Architecture

The application has three layers:

| Layer | Location | Responsibility |
| --- | --- | --- |
| React UI | `src/` | Disc selection, options, output selection, progress and update dialogs. |
| Tauri shell | `crates/dw4vhp-tauri/src/` | IPC, native file dialogs, background workers and application updates. |
| Rust engine | `crates/dw4vhp-core/src/` | Disc detection, patch planning, archive/resource edits, verified output and recovery. |

Components use `src/ipc/backend.ts` and the typed `PatchSelection` contract.
Rust defines it in `options.rs`; `npm run bindings` generates its TypeScript
interface and empty defaults. `npm run check` rejects generated-file drift.
`tauri.ts` implements the desktop transport;
`mock.ts` supports browser development and UI tests. Game data stays in Rust.

## Following a patch

1. `App.tsx` collects selected features. `useDisc.ts` requests analysis when the
   disc or resource options change.
2. Tauri's `commands.rs` validates the selection and queues a worker. Work never
   blocks the desktop event loop.
3. `disc.rs` detects the release and validates the authored tables.
4. `plan.rs` computes balance changes. `crown/` plans feature/resource edits and
   rebuilds the resource archive; randomizer decisions use an explicit seed.
5. `patch.rs` merges writes. `writer.rs` streams a separate temporary output.
6. Tables, archive contents and metadata are verified before the final rename.
   `signature.rs` identifies outputs; `update.rs` captures and restores original
   regions so updating a mod does not stack changes.

## Where to make changes

- User flows: `src/features/`, with orchestration in `src/app/App.tsx`.
- Native IPC and application updates: Tauri's `commands.rs` and `updates.rs`.
- Stat transforms: `plan.rs`, `ratio.rs` and `table.rs`.
- Enemy choices and capacity: `crown/random.rs`, `random/distribution.rs` and `capacity.rs`. Native-body reclamation and pool shrinking live in `reclaim.rs`.
- Optional dungeon bosses: `crown/random/boss.rs` applies after patrol planning;
  `boss_plan.rs` preserves the encounter inventory, and `boss_intro.rs` retargets
  each arena's localized info card. `PatchSelection.random_bosses` controls this production
  path; development-report environment variables never enable the feature.
- Fixed protection policy and explanations: `crown/encounters.rs` and the
  [generated encounter catalogue](randomizer-rules.md).
- IGB parsing and lossless editing: `formats/igb.rs`.
- Archive orchestration and crown model capacity: `crown/archive.rs` and `model.rs`.
- Difficulty-specific model/sound files and manifest selectors: `crown/difficulty.rs`.
  Guarded PS2/GameCube filename dispatch lives in `difficulty_io.rs`; allocator
  and dispatch hooks are included in the archive plan's verified/recoverable writes.
- Imported model/effect graphs: `crown/graft.rs`, `sound.rs` and `height.rs`.
- EXP preservation: `crown/rewards.rs`; floating numbers: `crown/exp.rs`.
- Disc/container formats: `iso9660.rs`, `gcm.rs`, `elf.rs` and `container.rs`.
- Recovery and identification: `update.rs` and `signature.rs`.

Boss intros retain native language selection and indexed Unicode rendering.
`boss_intro.rs` appends donor message banks to each arena’s localized AREA packs;
the difficulty-specific intro scripts select the matching four indexes. Existing
messages keep their indexes and original resources keep their IDs. The rebuilt
AREA archive receives final balance and local MODEL mappings before serialization
and is appended alongside NDMW.AFS, with its ISO/FST extent updated. Original image
table counts are verified separately from appended resources, which are verified
byte-for-byte against the plan. Recovery restores both archive extents.

## Compatibility boundaries

Image options and recovery descriptors must remain readable for existing images.
Images predating the optional Crown Mod switch imply Crown Mod was enabled.
New UI selections start with every option disabled. Low-level `PatchPlan::default`
retains the historical Crown Mod preset for library callers; it is not a UI default.

A seed reproduces a plan within a randomizer rules version. Pure refactors must
preserve rules versions, ordered iteration and PRNG calls. Keep binary-layout
validation and refusal paths intact when moving format code.

`docs/testing/` records evidence and limitations. It is not a substitute for the
current architecture or contributor instructions.
