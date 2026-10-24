import process from "node:process";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
const target = "docs/randomizer-rules.md";
const result = spawnSync("cargo", ["run", "--quiet", "--locked", "-p", "dw4vhp-core", "--example", "export_randomizer_rules"], { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
const generated = result.stdout.replaceAll("\r\n", "\n");
if (process.argv.includes("--check")) {
  if (readFileSync(target, "utf8").replaceAll("\r\n", "\n") !== generated) throw new Error(`Run npm run docs:rules to update ${target}.`);
} else writeFileSync(target, generated);
