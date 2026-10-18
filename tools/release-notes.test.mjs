import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, expect, test } from "vitest";
import { releaseNotes } from "./release-notes.mjs";
const directories = [];
afterEach(() => { for (const dir of directories.splice(0)) rmSync(dir, { recursive: true, force: true }); });
function fixture(changelog) {
  const root = mkdtempSync(join(tmpdir(), "crown-release-notes-")); directories.push(root);
  for (const dir of ["crates/dw4vhp-core", "crates/dw4vhp-tauri"]) {
    mkdirSync(join(root, dir), { recursive: true });
    writeFileSync(join(root, dir, "Cargo.toml"), '[package]\nversion = "1.2.3"\n');
  }
  writeFileSync(join(root, "package.json"), '{"version":"1.2.3"}');
  writeFileSync(join(root, "crates/dw4vhp-tauri/tauri.conf.json"), '{"version":"1.2.3"}');
  writeFileSync(join(root, "CHANGELOG.md"), changelog);
  return root;
}
test("release notes include the selected changelog rather than older release entries", () => {
  const root=fixture("# Changelog\n\n## Unreleased\n\n- New loot setting.\n\n## v1.2.2\n\n- Older change.\n");
  expect(releaseNotes(root,"v1.2.3")).toBe("DW4 Crown Mod v1.2.3\n\n- New loot setting.\n");
});
test("explicit release notes take precedence over Unreleased", () => {
  const root=fixture("## Unreleased\n\n- Future work.\n\n## v1.2.3\n\n- Shipped change.\n");
  expect(releaseNotes(root,"v1.2.3")).toContain("Shipped change");
  expect(releaseNotes(root,"v1.2.3")).not.toContain("Future work");
});
test("publishing rejects mismatched tags, package versions and empty notes", () => {
  const root=fixture("## Unreleased\n\n## v1.2.2\n\n- Older.\n");
  expect(()=>releaseNotes(root,"v1.2.4")).toThrow(/does not match/);
  expect(()=>releaseNotes(root,"v1.2.3")).toThrow(/empty/);
  writeFileSync(join(root,"crates/dw4vhp-core/Cargo.toml"),'version = "0.1.0"\n');
  expect(()=>releaseNotes(root,"v1.2.3")).toThrow(/version does not match/);
});
