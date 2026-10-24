# Contributing

Start with the [architecture map](docs/ARCHITECTURE.md) and [development guide](docs/DEVELOPMENT.md).
The UI can run with a mock backend; ordinary checks do not require a game image.

```sh
npm ci
npm run dev
npm run check
```

Generated contracts and encounter documentation are checked for drift. Run
`npm run bindings` or `npm run docs:rules` after editing their Rust definitions.

The complete check requires Rust, Node.js and the desktop dependencies listed in
the development guide. It checks the Rust engine, React app **and** Tauri shell.
For a focused change, use `npm run check:core`, `npm run check:frontend` or
`npm run check:desktop`. The root Cargo workspace intentionally excludes Tauri;
`cargo test --workspace` alone is not the complete application check.

Keep changes small and explain the resulting behaviour and validation in the PR.
For refactors, preserve randomizer rules, decision order, image metadata and
patched bytes. Compare fixed seeds against a reference build with explicit enemy
and crown-colour seeds. Describe any intended output change explicitly.

Game images, extracted assets, saves and memory dumps must stay outside the
repository. Use test-owned synthetic fixtures for ordinary tests. Real-disc and
emulator checks are opt-in; report which releases and features were actually
exercised, and distinguish disc-data verification from gameplay verification.
See [the validation tiers](docs/DEVELOPMENT.md#validation-tiers).
