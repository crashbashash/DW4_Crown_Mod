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

  test("patches with only the output and overwrite choice", async () => {
    await new TauriBackend().startPatch("out.iso", false);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("start_patch", {
      output: "out.iso",
      overwrite: false,
    });
  });
});
