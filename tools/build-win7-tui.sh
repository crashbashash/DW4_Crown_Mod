#!/usr/bin/env bash
# Cross-builds the terminal front end for Windows 7+ from Linux and zips it.
#
# Rust's tier-1 Windows targets need Windows 10, so this uses the tier-3
# x86_64-win7-windows-msvc target: nightly, std rebuilt from source, the static
# CRT (no VC++ redistributable) and cargo-xwin for the MSVC CRT/SDK libraries
# (downloading them accepts Microsoft's licence).
#
# Requires rustup with a nightly toolchain and the rust-src component,
# cargo-xwin, clang, lld and bsdtar. TOOLCHAINS overrides where they live.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
toolchains="${TOOLCHAINS:-$root/../.toolchains}"
target=x86_64-win7-windows-msvc
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/crates/dw4vhp-tui/Cargo.toml" | head -1)"

export RUSTUP_HOME="$toolchains/rustup" CARGO_HOME="$toolchains/cargo"
export PATH="$CARGO_HOME/bin:$PATH" XWIN_ACCEPT_LICENSE=1 XWIN_CACHE_DIR="$toolchains/xwin"
export RUSTFLAGS="-C target-feature=+crt-static"

cd "$root"
cargo +nightly xwin build --release -p dw4vhp-tui --target "$target" -Zbuild-std \
  --target-dir target/win7

exe="target/win7/$target/release/dw4-crown-mod-tui.exe"
# Anything Windows 7 lacks would stop the program loading at all.
if llvm-readobj --coff-imports "$exe" |
  grep -E "ProcessPrng|bcryptprimitives|api-ms-win|WaitOnAddress|WakeByAddress|GetSystemTimePreciseAsFileTime|CreateFile2|SetThreadDescription"; then
  echo "error: $exe imports an API that Windows 7 lacks" >&2
  exit 1
fi

out="dist/win7"
mkdir -p "$out"
zip="$out/dw4-crown-mod-tui-$version-windows7-x86_64.zip"
rm -f "$zip"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp "$exe" "$stage/"
# Windows 7 Notepad shows LF-only text as a single line.
sed 's/$/\r/' crates/dw4vhp-tui/README.txt >"$stage/README.txt"
bsdtar -a -cf "$zip" -C "$stage" dw4-crown-mod-tui.exe README.txt
echo "$zip"
