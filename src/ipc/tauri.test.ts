import { beforeEach, describe, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { TauriBackend } from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => vi.clearAllMocks());

describe("base-mode native commands", () => {
  test("requests a summary without a configurable plan", async () => {
   await new TauriBackend().planSummary();
   expect(invoke).toHaveBeenCalledExactlyOnceWith("plan_summary");
  });

  test("defaults to the normal enemy spawn counts", async () => {
   await new TauriBackend().startPatch("out.iso", false);
   expect(invoke).toHaveBeenCalledExactlyOnceWith("start_patch", {
     output: "out.iso",
     overwrite: false,
     fourPlayerSpawns: false, crownDrops: false, randomCrownColours: false, randomEnemies: false, enemySeed: null,
   });
  });

  test("passes the spawn option to inspection and patching without other settings", async () => {
   const backend = new TauriBackend();
   await backend.analyze("clean.iso", true);
   expect(invoke).toHaveBeenLastCalledWith("analyze", { path: "clean.iso", fourPlayerSpawns: true, crownDrops: false });
   await backend.startPatch("out.iso", false, true);
   expect(invoke).toHaveBeenLastCalledWith("start_patch", {
     output: "out.iso", overwrite: false, fourPlayerSpawns: true, crownDrops: false, randomCrownColours: false, randomEnemies: false, enemySeed: null,
   });
  });
});

test("forwards custom loot independently of spawn counts", async () => {
  const backend = new TauriBackend();
  await backend.analyze("clean.iso", false, true);
  expect(invoke).toHaveBeenLastCalledWith("analyze", { path: "clean.iso", fourPlayerSpawns: false, crownDrops: true });
  await backend.startPatch("out.iso", false, false, true);
  expect(invoke).toHaveBeenLastCalledWith("start_patch", { output: "out.iso", overwrite: false, fourPlayerSpawns: false, crownDrops: true, randomCrownColours: false, randomEnemies: false, enemySeed: null });
});

test("update acceptance names only the offered version, never a webview-supplied installer URL", async () => {
  const backend = new TauriBackend();
  await backend.checkUpdates();
  expect(invoke).toHaveBeenLastCalledWith("check_updates");
  await backend.acceptUpdate("0.10.0");
  expect(invoke).toHaveBeenLastCalledWith("accept_update", { version: "0.10.0" });
});

 test("passes per-spawn cosmetic colours independently from spawn counts and loot", async () => {
   await new TauriBackend().startPatch("out.iso", false, false, false, true);
   expect(invoke).toHaveBeenLastCalledWith("start_patch", { output: "out.iso", overwrite: false, fourPlayerSpawns: false, crownDrops: false, randomCrownColours: true, randomEnemies: false, enemySeed: null });
 });
