# Development guide

Details for people who want to build, test or contribute to DW4 Crown Mod.
If you just want to play, see the [README](../README.md).

## Project layout

- `crates/dw4vhp-core`: the engine that edits the disc. It can be tested on its own.
- `crates/dw4vhp-tauri`: the desktop window. A Tauri shell plus the React app in `src/`.

## Building from source

You need a Rust toolchain and Node.js. On Linux you also need the Tauri webview
dependencies:

```sh
sudo apt-get install -y \
  libxkbcommon-dev \
  libwayland-dev \
  libx11-dev \
  libxi-dev \
  libgl1-mesa-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

Then:

```sh
npm ci
npm run dev        # frontend alone (mock backend, in a browser tab)
npx tauri dev      # the desktop app, live-reloading
npx tauri build    # distributables (exe/AppImage/deb/MSI)
```

## App artwork

`public/crown.svg` is the editable source for the UI mark, favicon and desktop
icons. It follows the DigiLab / DigiAtlas badge layout with Crown Mod gold.
After editing it, regenerate the committed PNG and Windows ICO with
`python3 tools/gen_icon.py` (requires `rsvg-convert` and Python Pillow).

## One rule

No game data is ever committed here: no disc images, no extracted files, no memory
dumps. The repository stores only metadata such as offsets, instructions and
verification fingerprints. The "before" values always come from the player's own disc.

## Releases

Release builds (Windows zip/MSI, Linux AppImage/deb/tarball) are produced by CI for
tagged releases. The release workflow waits for both platform builds, uploads all
assets to a draft, and publishes only after upload completes. The Linux builds are launch-tested. The Windows builds are built but
never run in CI. Like all release assets, they are unsigned.

## Image identification

New outputs carry the [versioned image signature](image-signature.md), including patch
options. Detection checks it first and retains the game-data scan for older images.

## Testing against real discs

See [random enemies](testing/random-enemies.md) for shared enemy capacity, audio and scripted entrance regressions.

Point these environment variables at clean local images:
`DW4_RETAIL_USA_ISO`, `DW4_RETAIL_EUROPE_ISO`, `DW4_RETAIL_JAPAN_ISO`.

The current optional-resource matrix also accepts `DW4_RETAIL_GC_USA_ISO` and
`DW4_RETAIL_GC_JAPAN_ISO` for plain/NKit GameCube images, plus
`DW4_RETAIL_GC_USA_CISO`, `DW4_RETAIL_GC_JAPAN_CISO`,
`DW4_RETAIL_GC_USA_RVZ` and `DW4_RETAIL_GC_JAPAN_RVZ` for compressed images.
The [completed release/container matrix](testing/resource-options.md) records the
current validation. The test exercises all four spawn/drop switch combinations on each raw release and
both options together through each compressed pipeline, plus a case with random
crown colours enabled alongside them. It verifies all 1,995
current-balance table blocks and unchanged original files, and checks the detected
release, output format and predicted raw output size. Outputs are temporary.
It also checks [overall patch progress](testing/patch-progress.md) for every build.

```sh
cargo test --release --test resource_options -- --nocapture
```

Unconfigured inputs are skipped. The tables sit at a fixed distance from the start
of `area.afs`, because NKit and RVZ conversions can move files.

```sh
cargo test -p dw4vhp-core --test retail -- --nocapture
```

Each configured image is patched to a temporary output and checked against the
shipped table fingerprints and independently computed full-disc checksums. The test
also checks for changes outside the intended table ranges, and confirms a second
patch attempt is refused. Unconfigured images are skipped. CI still runs the
generated regional detection fixtures.

Other tests:

- **Legacy golden test:** verifies the original shipped stat transform. It needs
  `DW4_GOLDEN_SHIPPED_ISO`, `DW4_GOLDEN_BLOCKS_BIN` and `DW4_GOLDEN_BLOCKS_JSON`,
  takes about 7 minutes, and skips cleanly when they are not set.
- **Row-group tests:** `tests/rowgroup.rs`, plus `tests/rowgroup_snapshot.rs` with
  `DW4_ROWGROUP_SNAPSHOT`. They check the engine's enemy grouping utilities.
- **Frontend:** `npm test`, `npm run typecheck`, `npm run lint`.

The [river turret checks](testing/river-turrets.md) cover missile-only boat
damage, colour rolls, crown lifecycle and Lua branch relocation. They include
an optional real-pack audit and a reference Lua 4 runtime harness.

The [four-player spawn and practice Speed checks](testing/four-player-spawns.md)
describe the optional spawn transform, its preserved data and current validation.

## Regional notes

The five supported releases contain the same 649-row enemy tables, repeated 665 times. The
app uses each release's own offsets and patches every copy, preserving the original
boot serial, language, video mode and save identity.

The low-level `force_very_hard` ELF option is USA-only and is refused for
Europe/Japan. The desktop app does not use it.

See the [changelog](../CHANGELOG.md) for output checksums and the
[crown expansion validation](testing/crowns-v0.8.0.md).

## Driving the desktop app with an agent

The optional `agent-control` build exposes the real webview through a
token-protected loopback endpoint. Ordinary builds do not include it. It drives the
rendered UI and the native patch engine, not the browser mock backend.

```sh
npm run build
cargo build --manifest-path crates/dw4vhp-tauri/Cargo.toml --features agent-control,tauri/custom-protocol
python3 tools/agent.py --session /tmp/dw4vhp-agent.json start --binary crates/dw4vhp-tauri/target/debug/dw4-crown-mod
python3 tools/agent.py --session /tmp/dw4vhp-agent.json snapshot
python3 tools/agent.py --session /tmp/dw4vhp-agent.json fill "disc path" "/path/to/clean.iso"
python3 tools/agent.py --session /tmp/dw4vhp-agent.json fill "output path" "/tmp/patched.iso"
python3 tools/agent.py --session /tmp/dw4vhp-agent.json click "Patch ISO"
```

`eval` accepts a JavaScript expression and returns its JSON result.
`screenshot /tmp/app.png` uses `xdotool` and ImageMagick on an X11/XWayland window
(launch with `GDK_BACKEND=x11`). The session file contains private credentials and
the app's PID, so use a fresh session path for each launch and remove the file
afterward.

Run the full UI test on a freshly launched app:

```sh
python3 tools/e2e.py --session /tmp/dw4vhp-agent.json --iso "/path/to/clean.iso" --expected-md5 73ac2e226a8a3fa504de9740ed9d446c --expected-size 2179188736
```

The test checks invalid and already-modded inputs, patching, overwrite cancellation
and confirmation, and refusal to overwrite the input. It verifies the output
checksum and that the original is unchanged. Reports, screenshots and test ISOs go
to a new directory under `/tmp`. The UI needs a working desktop display, and
screenshots need the X11 tools above. Pass `--region europe` or `--region japan`
and the matching checksum to test those releases.

Optional loot rebalance details: [custom crown drops](testing/custom-drops.md).

Disc-selection and patching errors use dismissible notices; already-modded images
have a specific notice title. Successful analysis enables patching without adding
technical status text to the Disc panel. The patching console and duplicate output
path are removed; progress and completion feedback remain. Live native UI checks
covered invalid files, missing files, already-modded discs and an unwritable output
parent, including dismissal and recovery to a clean selection. Frontend tests also
cover error notices and recovery; native tests check the structured modded reason.

Application update behavior, release-version checks and validation are described
in [application updates](testing/app-updates.md).

Disc analysis shows a modal with an animated, indeterminate progress bar. It
closes automatically when inspection finishes; errors then use the usual notice
popup. Tests cover successful/refused results and prevent premature dismissal.

Patching uses one continuous activity indicator and a stable message until work
finishes. Internal counters can restart during unpacking, copying, archive writing,
compression and verification, so they are not presented as overall percentages or
repeated stage labels. The main Output panel contains no progress/status readout.
Success opens the friendly completion popup; failures open an error notice. Tests
cover the compressed-image event sequence, completion/failure handoff and
non-dismissal while busy.

## Development-build disc performance

Development builds optimize the core disc-processing crate and its hashing
crates while retaining debug symbols, debug assertions and overflow checks.
The profile overrides appear in both workspace roots: the core workspace and
the separately built Tauri shell. Release profiles are unchanged.

A 2026-10-19 local benchmark hashed the same 128 MiB input three times per
algorithm. Median throughput increased from 61.5 to 647.1 MiB/s for MD5 (10.5x)
and from 68.7 to 1981.9 MiB/s for SHA-1 (28.9x). Before/after digests matched.
These are hashing measurements; complete patching also includes disc parsing,
copying, compression and verification.
