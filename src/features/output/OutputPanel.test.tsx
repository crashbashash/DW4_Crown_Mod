import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import type { DiscStatus } from "../../bindings";
import { OutputPanel } from "./OutputPanel";

function setup(overrides: Partial<Parameters<typeof OutputPanel>[0]> = {}) {
  const onPatch = vi.fn();
  const onChooseOutput = vi.fn();
  const onSetOutput = vi.fn();
  const props = {
    outputPath: "/mock/game [CrownMod].iso",
    busy: false,
    canPatch: true,
    status: null as DiscStatus | null,
    onPatch,
    onChooseOutput,
    onSetOutput,
    ...overrides,
  };
  render(<OutputPanel {...props} />);
  return { onPatch, onSetOutput, onChooseOutput };
}

describe("OutputPanel", () => {
  test("Patch button disabled when cannot patch", () => {
    setup({ canPatch: false });
    expect(screen.getByRole("button", { name: /patch iso/i }).hasAttribute("disabled")).toBe(true);
  });

  test("Patch button enabled and triggers the callback", async () => {
    const { onPatch } = setup();
    await userEvent.click(screen.getByRole("button", { name: /patch iso/i }));
    expect(onPatch).toHaveBeenCalled();
  });

  test("free-space note uses the expanded output size", () => {
    setup({ status: { kind: "ok", text: "", size: 1448902656, outputSize: 2179143680, copies: [1, 1, 1] } });
    expect(screen.getByText(/2.18 GB of free space/)).toBeTruthy();
  });

});

 test("modded images replace Patch ISO with Update ISO and hide the separate output path", async () => {
   const action = vi.fn();
   setup({canPatch: true, status: {kind: "refused", reason: "already-modded", text: "Crown Mod", size: 0, outputSize: 0, copies: null}, onPatch: action});
   expect(screen.queryByRole("button", {name: "Patch ISO"})).toBeNull();
   expect(screen.queryByLabelText("output path")).toBeNull();
   expect(screen.queryByRole("button", {name: "Save as…"})).toBeNull();
   await userEvent.click(screen.getByRole("button", {name: "Update ISO"}));
   expect(action).toHaveBeenCalledOnce();
 });
