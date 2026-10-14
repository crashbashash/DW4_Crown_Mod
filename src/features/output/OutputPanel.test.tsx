import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import type { DiscStatus, PatchOutcome, ProgressEvent } from "../../bindings";
import { OutputPanel } from "./OutputPanel";

function setup(overrides: Partial<Parameters<typeof OutputPanel>[0]> = {}) {
  const onPatch = vi.fn();
  const onChooseOutput = vi.fn();
  const onSetOutput = vi.fn();
  const props = {
    outputPath: "/mock/game [VeryHardPlus].iso",
    busy: false,
    canPatch: true,
    progress: null as ProgressEvent | null,
    log: [] as string[],
    status: null as DiscStatus | null,
    lastOutcome: null as PatchOutcome | null,
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

  test("free-space note shows the analysed size", () => {
    setup({ status: { kind: "ok", text: "", size: 4600000000, copies: [1, 1, 1] } });
    expect(screen.getByText(/4.60 GB of free space/)).toBeTruthy();
  });

  test("completed patch size uses GB", () => {
    setup({ lastOutcome: { bytes: 1448902656, md5: "test", summary: {
      rowsChanged: 580, liveRows: 580, attackPinned: 337, hpCapped: 0,
      critPinned: 0, practiceRows: 6, collapsedRows: 0, elfStep: false, serialStep: false,
    } } });
    expect(screen.getByText(/Patched 1.45 GB/)).toBeTruthy();
    expect(screen.queryByText(/pinned|bytes/)).toBeNull();
  });

  test("engine failure message lands in the log verbatim", () => {
    setup({
      log: ["patch refused: the output file already exists — choose a new name, or allow overwriting it"],
    });
    expect(screen.getByText(/the output file already exists/)).toBeTruthy();
  });

  test("progress bar shows phase and fraction", () => {
    setup({ progress: { phase: "copying", done: 500, total: 1000 }, busy: true });
    expect(screen.getByText(/copying/)).toBeTruthy();
    expect(screen.getByText("50%")).toBeTruthy();
  });
});
