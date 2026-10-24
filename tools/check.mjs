/** Cross-platform entry point for the checks used by contributors and CI. */
import { spawnSync } from "node:child_process";
import process from "node:process";

const scope = process.argv[2] ?? "all";
if (!["all", "core", "frontend", "desktop"].includes(scope)) {
  throw new Error("Usage: npm run check -- [all|core|frontend|desktop]");
}
const retailInputs = ["DW4_RETAIL_USA_ISO", "DW4_RETAIL_EUROPE_ISO", "DW4_RETAIL_JAPAN_ISO", "DW4_RETAIL_GC_USA_ISO", "DW4_RETAIL_GC_JAPAN_ISO"];
const configured = retailInputs.filter(name => process.env[name]);
process.stdout.write(`Retail image inputs: ${configured.length}/${retailInputs.length} configured. Unconfigured opt-in cases skip; this does not establish gameplay coverage.\n`);
const shellManifest = "crates/dw4vhp-tauri/Cargo.toml";
function run(command, args) {
  process.stdout.write(`\n> ${command} ${args.join(" ")}\n`);
  const result = spawnSync(command, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
function npm(script) {
  // npm supplies its JS entry point on Windows too, avoiding .cmd quoting.
  if (!process.env.npm_execpath) throw new Error("Run this checker through npm run check.");
  run(process.execPath, [process.env.npm_execpath, "run", script]);
}
if (scope === "all" || scope === "core") {
  npm("bindings:check");
  npm("docs:rules:check");
  run("cargo", ["fmt", "--all", "--check"]);
  run("cargo", ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]);
  run("cargo", ["test", "--workspace", "--locked"]);
}
if (scope === "all" || scope === "frontend") {
  for (const script of ["typecheck", "lint", "test", "build"]) npm(script);
}
if (scope === "all" || scope === "desktop") {
  if (scope === "desktop") npm("build");
  run("cargo", ["fmt", "--manifest-path", shellManifest, "--check"]);
  for (const action of ["clippy", "test", "build"]) {
    const args = [action, "--manifest-path", shellManifest, "--locked"];
    if (action === "clippy") args.push("--all-targets", "--", "-D", "warnings");
    run("cargo", args);
  }
}
process.stdout.write(`\n${scope} checks passed. Real-disc audits are separate opt-in checks; see docs/DEVELOPMENT.md.\n`);
