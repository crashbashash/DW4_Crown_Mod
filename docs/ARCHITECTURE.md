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
- Enemy choices and capacity: `crown/random.rs` and `capacity.rs`.
- Fixed protection policy and explanations: `crown/encounters.rs` and the
  [generated encounter catalogue](randomizer-rules.md).
- IGB parsing and lossless editing: `formats/igb.rs`.
- Archive orchestration and crown model capacity: `crown/archive.rs` and `model.rs`.
- Imported model/effect graphs: `crown/graft.rs`, `sound.rs` and `height.rs`.
- EXP preservation: `crown/rewards.rs`; floating numbers: `crown/exp.rs`.
- Disc/container formats: `iso9660.rs`, `gcm.rs`, `elf.rs` and `container.rs`.
- Recovery and identification: `update.rs` and `signature.rs`.

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
