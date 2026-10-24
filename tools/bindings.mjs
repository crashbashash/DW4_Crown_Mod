import process from "node:process";
/** Keep the frontend patch-selection contract synchronized with Rust serde. */
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
const target = "src/bindings/PatchSelection.ts";
const result = spawnSync("cargo", ["run", "--quiet", "--locked", "-p", "dw4vhp-core", "--example", "export_patch_selection"], { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
const generated = result.stdout.replaceAll("\r\n", "\n");
if (process.argv.includes("--check")) {
  if (readFileSync(target, "utf8").replaceAll("\r\n", "\n") !== generated) {
    throw new Error(`${target} is out of date. Run npm run bindings and commit the result.`);
  }
} else {
  writeFileSync(target, generated);
}
