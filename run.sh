#!/usr/bin/env bash
# Build and run DW4 Very Hard Plus.
#
#   ./run.sh              live-reloading development window (npx tauri dev)
#   ./run.sh --release    optimised build, then open the app
#   ./run.sh --test       core tests, frontend tests, typecheck and lint
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
    npx tauri build --no-bundle
    exec "$root/target/release/dw4-veryhard-plus"
    ;;
  --test)
    cargo test -p dw4vhp-core
    npm test --silent
    npm run -s typecheck
    npm run -s lint
    ;;
  "") exec npx tauri dev ;;
  *) usage >&2; exit 2 ;;
esac
