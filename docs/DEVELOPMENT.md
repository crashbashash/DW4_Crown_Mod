# Development guide

Details for people who want to build, test or contribute to DW4 Very Hard Plus.
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

## One rule

No game data is ever committed here: no disc images, no extracted files, no memory
dumps. The repository stores only metadata such as offsets, instructions and
verification fingerprints. The "before" values always come from the player's own disc.

## Releases

Release builds (Windows zip/MSI, Linux AppImage/deb/tarball) are produced by CI for
tagged releases. The Linux builds are launch-tested. The Windows builds are built but
never run in CI. Like all release assets, they are unsigned.

## Testing against real discs

Point these environment variables at clean local images:
`DW4_RETAIL_USA_ISO`, `DW4_RETAIL_EUROPE_ISO`, `DW4_RETAIL_JAPAN_ISO`.

GameCube images were checked by hand with `patch_file` on NKit `.iso`, `.ciso` and `.rvz` inputs, and the RVZ and CISO outputs were read back with Dolphin's `dolphin-tool`. The tables sit at a fixed distance from the start of `area.afs`, because NKit and RVZ conversions move files.

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

## Regional notes

The three releases contain the same 649-row enemy tables, repeated 665 times. The
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
python3 tools/agent.py --session /tmp/dw4vhp-agent.json start --binary crates/dw4vhp-tauri/target/debug/dw4-veryhard-plus
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
python3 tools/e2e.py --session /tmp/dw4vhp-agent.json --iso "/path/to/clean.iso" --expected-md5 9ba8432d2237330a687d4ac1a0203d68 --expected-size 2179143680
```

The test checks invalid and already-modded inputs, patching, overwrite cancellation
and confirmation, and refusal to overwrite the input. It verifies the output
checksum and that the original is unchanged. Reports, screenshots and test ISOs go
to a new directory under `/tmp`. The UI needs a working desktop display, and
screenshots need the X11 tools above. Pass `--region europe` or `--region japan`
and the matching checksum to test those releases.
