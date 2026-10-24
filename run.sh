#!/usr/bin/env bash
# Build and run DW4 Crown Mod.
#
#   ./run.sh              live-reloading development window (npx tauri dev)
#   ./run.sh --release    optimised build, then open the app
#   ./run.sh --test       complete engine, frontend and desktop checks
#
# The app asks for your own PS2 or GameCube disc image (.iso, .ciso, .rvz).
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$root"

usage() { sed -n "2,8p" "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

[[ -d node_modules ]] || npm ci

case "${1:-}" in
  -h|--help) usage ;;
  --release)
    CARGO_TARGET_DIR="$root/target" npx tauri build --no-bundle
    exec "$root/target/release/dw4-crown-mod"
    ;;
  --test)
    npm run check
    ;;
  "") exec npx tauri dev ;;
  *) usage >&2; exit 2 ;;
esac
