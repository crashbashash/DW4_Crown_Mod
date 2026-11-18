/**
 * Builds, tests and packages the Windows 7 terminal version (crates/dw4vhp-tui).
 *
 * Rust's tier-1 Windows targets need Windows 10, so this uses the tier-3
 * x86_64-win7-windows-msvc target: a pinned nightly (rust-src and llvm-tools
 * components) with std rebuilt from source and the static CRT, so players need no
 * VC++ redistributable. Windows hosts build natively; other hosts cross-build with
 * cargo-xwin, which downloads the MSVC CRT and SDK (accepting Microsoft's licence).
 *
 *   node tools/win7.mjs [--out DIR]   build, check imports, write the release zip
 *   node tools/win7.mjs test          run core and TUI tests built for the target (Windows hosts)
 */
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const TOOLCHAIN = "nightly-2026-11-17";
const TARGET = "x86_64-win7-windows-msvc";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const windows = process.platform === "win32";
// Functions Windows 7 lacks. Importing any of them stops the program loading at all.
const NEWER_THAN_WINDOWS_7 =
  /ProcessPrng|bcryptprimitives|api-ms-win|WaitOnAddress|WakeByAddress|GetSystemTimePreciseAsFileTime|CreateFile2|SetThreadDescription/i;

function run(command, args, { env = process.env, capture = false } = {}) {
  process.stdout.write(`\n> ${command} ${args.join(" ")}\n`);
  const result = spawnSync(command, args, {
    cwd: root,
    env,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
  return result.stdout;
}

function cargo(action, args) {
  const env = { ...process.env, RUSTFLAGS: "-C target-feature=+crt-static" };
  if (!windows) env.XWIN_ACCEPT_LICENSE = "1";
  const subcommand = windows ? [action] : ["xwin", action];
  run("cargo", [`+${TOOLCHAIN}`, ...subcommand, "--locked", "--target", TARGET, "-Zbuild-std",
    "--target-dir", "target/win7", ...args], { env });
}

if (process.argv[2] === "test") {
  if (!windows) throw new Error("Windows 7 target tests run on Windows hosts only.");
  cargo("test", ["-p", "dw4vhp-core", "-p", "dw4vhp-tui"]);
  process.exit(0);
}

const args = process.argv.slice(2);
const out = resolve(root, args.includes("--out") ? args[args.indexOf("--out") + 1] : "dist/win7");
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;

cargo("build", ["--release", "-p", "dw4vhp-tui"]);
const exe = join(root, "target/win7", TARGET, "release/dw4-crown-mod-tui.exe");

const sysroot = run("rustc", [`+${TOOLCHAIN}`, "--print", "sysroot"], { capture: true }).trim();
const host = /^host: (\S+)$/m.exec(run("rustc", [`+${TOOLCHAIN}`, "-vV"], { capture: true }))[1];
const readobj = join(sysroot, "lib/rustlib", host, "bin", windows ? "llvm-readobj.exe" : "llvm-readobj");
const imports = run(readobj, ["--coff-imports", exe], { capture: true })
  .split("\n")
  .filter(line => /^\s*(Name|Symbol):/.test(line));
if (!imports.some(line => /KERNEL32\.dll/i.test(line))) throw new Error(`No import table read from ${exe}`);
const newer = imports.filter(line => NEWER_THAN_WINDOWS_7.test(line));
if (newer.length) throw new Error(`${exe} imports functions Windows 7 lacks:\n${newer.join("\n")}`);
process.stdout.write(`${imports.length} import lines checked: all available on Windows 7.\n`);

const stage = mkdtempSync(join(tmpdir(), "dw4-win7-"));
try {
  copyFileSync(exe, join(stage, "dw4-crown-mod-tui.exe"));
  // Windows 7 Notepad shows LF-only text as a single line.
  const readme = readFileSync(join(root, "crates/dw4vhp-tui/README.txt"), "utf8");
  writeFileSync(join(stage, "README.txt"), readme.replace(/\r?\n/g, "\r\n"));
  mkdirSync(out, { recursive: true });
  const zip = join(out, `dw4-crown-mod-v${version}-windows7-x86_64.zip`);
  rmSync(zip, { force: true });
  // bsdtar writes zip archives; Windows ships it as System32\tar.exe.
  const tar = windows ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "bsdtar";
  run(tar, ["-a", "-cf", zip, "-C", stage, "dw4-crown-mod-tui.exe", "README.txt"]);
  process.stdout.write(`${zip}\n`);
} finally {
  rmSync(stage, { recursive: true, force: true });
}
