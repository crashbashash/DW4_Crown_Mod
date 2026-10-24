import { beforeEach, describe, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { DEFAULT_PATCH_SELECTION, type PatchSelection } from "../bindings/PatchSelection";
import { TauriBackend } from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.clearAllMocks());
const selection = (overrides: Partial<PatchSelection> = {}): PatchSelection => ({ ...DEFAULT_PATCH_SELECTION, ...overrides });

describe("typed native requests", () => {
  test("requests a summary without a configurable plan", async () => {
    await new TauriBackend().planSummary();
    expect(invoke).toHaveBeenCalledExactlyOnceWith("plan_summary");
  });

  test("defaults to an empty feature selection", async () => {
    await new TauriBackend().startPatch("out.iso", false);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("start_patch", {
      output: "out.iso", overwrite: false, selection: selection(),
    });
  });

  test.each([
    { fourPlayerSpawns: true },
    { crownMod: true, crownDrops: true },
    { randomCrownColours: true },
    { randomEnemies: true, enemySeed: 1520 },
  ])("forwards named options without positional booleans: %j", async overrides => {
    const backend = new TauriBackend();
    const options = selection(overrides);
    await backend.analyze("clean.iso", options);
    expect(invoke).toHaveBeenLastCalledWith("analyze", { path: "clean.iso", selection: options });
    await backend.startPatch("out.iso", false, options);
    expect(invoke).toHaveBeenLastCalledWith("start_patch", { output: "out.iso", overwrite: false, selection: options });
    await backend.updateMod(options, null);
    expect(invoke).toHaveBeenLastCalledWith("update_mod", { selection: options, original: null });
  });
});

test("update acceptance names only the offered version, never a webview-supplied installer URL", async () => {
  const backend = new TauriBackend();
  await backend.checkUpdates();
  expect(invoke).toHaveBeenLastCalledWith("check_updates");
  await backend.acceptUpdate("0.10.0");
  expect(invoke).toHaveBeenLastCalledWith("accept_update", { version: "0.10.0" });
});
