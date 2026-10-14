# DW4 Very Hard Plus

[![CI Linux](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/ci-linux.yml?label=CI%20Linux&branch=main)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/ci-linux.yml)
[![CI Windows](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/ci-windows.yml?label=CI%20Windows&branch=main)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/ci-windows.yml)
[![Release Linux](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/release-linux.yml?label=Release%20Linux)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/release-linux.yml)
[![Release Windows](https://img.shields.io/github/actions/workflow/status/crashbashash/DW4_VeryHard_Plus/release-windows.yml?label=Release%20Windows)](https://github.com/crashbashash/DW4_VeryHard_Plus/actions/workflows/release-windows.yml)

This is a little desktop app for **Digimon World 4** players who found the game too easy.
It takes your own copy of the game and turns it into a much harder one, in a few clicks —
no modding knowledge required.

Think of it like this: the people who made the original "yellow crown" mod edited their own
disc by hand. This app does that same edit for you, automatically, on your machine, using
your own disc.

## How it works (the short version)

1. You give the app a copy of your game disc (an `.iso` file).
2. The app applies the base Very Hard Plus mode.
3. The app writes out a **new, separate file** — your original is never touched.

You end up with something like `Digimon World 4 [VeryHardPlus].iso`, which you can play
in an emulator or burn/load like any other copy.

## What you need

- A **US release** of Digimon World 4 (`SLUS_208.36`). If you give it a disc from another
  region or a different version, the app tells you it won't work — it doesn't guess.
- Windows or Linux. Nothing else to install.

## The mode

**Very Hard Plus** boosts enemy health, attack, and other stats and adds the
"yellow crown" worn by the strongest enemy variants. This base mode has been
checked byte-for-byte against real discs. It is the only mode available for now;
there are no difficulty presets or custom settings to choose.
If the output already exists, pressing **Patch ISO** asks before replacing it.

Attack values are already near the game's maximum, so on Very Hard most enemies
hit just as hard as each other. Much of the extra difficulty comes from health
and other stat boosts.

## Is my disc safe?

Yes. The app opens your file **read-only**, does its work on a temporary copy, checks the
result, and only then names it as the finished file. If anything goes wrong or you cancel
halfway, all that exists is an incomplete extra file — your original is never modified.
It also checks up front that your disc is the right one, isn't already modded, and that
you have enough disk space, and writes nothing at all unless everything checks out.

## Where do I get it?

Download a ready-made version from the **Releases** page of this project — there are
installers for Windows (`.msi` or a plain `.zip`) and Linux (`.AppImage`, `.deb`, or a
plain download). No building required.

## For developers and the curious

The rest of the details live here:

- **Building from source** needs a Rust toolchain, Node.js, and — on Linux — the
  Tauri webview dependencies:

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

- The code has two parts: `dw4vhp-core` (the engine that actually edits the disc, fully
  testable on its own) and `dw4vhp-tauri` (the window you see: a Tauri shell plus the React app in `src/`).
- **Testing against real discs:** the "golden test" proves the app reproduces known,
  shipped discs byte-for-byte. It needs real disc images supplied through environment
  variables (`DW4_GOLDEN_SHIPPED_ISO`, `DW4_GOLDEN_BLOCKS_BIN`, `DW4_GOLDEN_BLOCKS_JSON`)
  and takes ~7 minutes; it skips cleanly (rather than failing) when those aren't set.
  There's also a row-group test (`tests/rowgroup.rs`, plus `tests/rowgroup_snapshot.rs`
  with `DW4_ROWGROUP_SNAPSHOT`) that checks the engine's enemy grouping utilities.
- **One rule of this repository:** no game data is ever committed here — no disc images,
  no extracted files, no memory dumps. Everything the app needs as a constant is an
  offset or instruction; the actual "before" values come from the player's own disc.
- Release builds (Windows zip/MSI, Linux AppImage/deb/tarball) are produced automatically
  by CI for tagged releases. The Linux ones are launch-tested; the Windows ones are built
  but never run here, and like all release assets, they're unsigned.

## Driving the desktop app with an agent

The optional `agent-control` build exposes the real webview through a token-protected
loopback endpoint. Ordinary builds do not include it. This drives the rendered UI and
native patch engine; it does not use the browser mock backend.

```sh
npm run build
cargo build --manifest-path crates/dw4vhp-tauri/Cargo.toml --features agent-control,tauri/custom-protocol
python3 tools/agent.py --session /tmp/dw4vhp-agent.json start --binary crates/dw4vhp-tauri/target/debug/dw4-veryhard-plus
python3 tools/agent.py --session /tmp/dw4vhp-agent.json snapshot
python3 tools/agent.py --session /tmp/dw4vhp-agent.json fill "disc path" "/path/to/clean.iso"
python3 tools/agent.py --session /tmp/dw4vhp-agent.json fill "output path" "/tmp/patched.iso"
python3 tools/agent.py --session /tmp/dw4vhp-agent.json click "Patch ISO"
```

`eval` accepts a JavaScript expression and returns its JSON result. `screenshot /tmp/app.png`
uses `xdotool` and ImageMagick on an X11/XWayland window (launch with `GDK_BACKEND=x11`).
The session file contains private credentials and the app's PID. Use a fresh session
path for each launch. Close the test app and remove the session file afterward.

Run the full UI test on a freshly launched app:

```sh
python3 tools/e2e.py --session /tmp/dw4vhp-agent.json --iso "/path/to/clean.iso" --expected-md5 df426b9ff17c9e8218779f8ff1ecf524
```

The test checks invalid and already-modded inputs, patching, overwrite cancellation
and confirmation, and refusal to overwrite the input. It checks the output checksum
and verifies that the original is unchanged. Reports, screenshots and test ISOs go to
a new directory under `/tmp`; game data is never added to the repository. The UI
requires a working desktop display. Screenshots require the X11 tools above.

## Licence

GPL-3.0-or-later — see `LICENSE`.
