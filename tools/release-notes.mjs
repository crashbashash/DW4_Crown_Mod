/** Write identical, version-checked changelog notes for both release workflows. */
import process from "node:process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
export function releaseNotes(root, tag) {
  const read = (path) => readFileSync(resolve(root, path), "utf8");
  const version = JSON.parse(read("package.json")).version;
  if (tag !== `v${version}`) throw Error(`Tag ${tag} does not match app version v${version}. Update the package versions before releasing.`);
  for (const path of ["crates/dw4vhp-core/Cargo.toml", "crates/dw4vhp-tauri/Cargo.toml"]) {
    if (read(path).match(/^version\s*=\s*"([^"]+)"/m)?.[1] !== version) throw Error(`${path} version does not match ${version}`);
  }
  if (JSON.parse(read("crates/dw4vhp-tauri/tauri.conf.json")).version !== version) throw Error("Tauri version does not match package.json");
  const changelog = read("CHANGELOG.md");
  const sections = [...changelog.matchAll(/^## (.+)\r?$/gm)];
  const index = sections.findIndex(s => s[1] === tag);
  const selected = index >= 0 ? index : sections.findIndex(s => s[1] === "Unreleased");
  if (selected < 0) throw Error("Add release notes to CHANGELOG.md before releasing");
  const section = sections[selected];
  const body = changelog.slice(section.index + section[0].length, sections[selected + 1]?.index ?? changelog.length).trim();
  if (!body) throw Error("Release changelog must not be empty");
  return `DW4 Crown Mod ${tag}\n\n${body}\n`;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const tag = args[args.indexOf("--tag") + 1];
  const output = args[args.indexOf("--output") + 1];
  if (!args.includes("--tag") || !args.includes("--output")) throw Error("Usage: node tools/release-notes.mjs --tag vX.Y.Z --output release-notes.md");
  writeFileSync(output, releaseNotes(process.cwd(), tag));
}
