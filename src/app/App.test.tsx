import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { MockBackend } from "../ipc/mock";
import { App } from "./App";

let media: Record<string, boolean> = {};

function mockMatchMedia(query: string, matches: boolean) {
  media[query] = matches;
}

beforeEach(() => {
  media = {};
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockImplementation((query: string) => ({
      matches: media[query] ?? false,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
    })),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("App layout", () => {
  test("renders the app header", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "DW4 Crown Mod" })).toBeTruthy();
  });

  test("wide layout uses two columns", () => {
    mockMatchMedia("(min-width: 900px)", true);
    render(<App />);
    expect(document.querySelector("main")?.className).toContain("layout-two-col");
  });

  test("narrow layout uses one column", () => {
    mockMatchMedia("(min-width: 900px)", false);
    render(<App />);
    expect(document.querySelector("main")?.className).toContain("layout-one-col");
  });
});

describe("App wiring", () => {
  test("keeps the difficulty settings fixed", () => {
    render(<App />);
    expect(screen.queryByRole("heading", { name: "Crown Mod" })).toBeNull();
    expect(screen.queryByText(/Extreme|Brutal|Custom|Advanced/)).toBeNull();
    expect(screen.queryByLabelText("hp")).toBeNull();
  });

  test("patch button starts disabled until a disc is analysed", async () => {
    render(<App />);
    const patchBtn = screen.getByRole("button", { name: /patch iso/i });
    expect(patchBtn.hasAttribute("disabled")).toBe(true); // no disc yet
    expect(screen.getByText(/choose a disc/i)).toBeTruthy();
  });

});

describe("App disc wiring", () => {
  test("typing an ISO path into the disc field starts analysis", async () => {
    render(<App />);
    const user = (await import("@testing-library/user-event")).default;
    const field = screen.getByLabelText("disc path");
    await user.type(field, "/mock/game.iso");
    await user.tab(); // commit on blur, as the old egui app did
    await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
  });

  test("typing into the disc field suggests the output name", async () => {
    render(<App />);
    const user = (await import("@testing-library/user-event")).default;
    const field = screen.getByLabelText("disc path");
    await user.type(field, "/mock/mygame.iso");
    await user.tab(); // commit on blur
    await waitFor(() =>
      expect(
        (screen.getByLabelText("output path") as HTMLInputElement).value,
      ).toBe("/mock/mygame [CrownMod].iso"),
    );
  });
});

describe("overwrite confirmation", () => {
  test.each([false, true])("existing output asks before patching; confirm=%s", async (confirm) => {
    vi.spyOn(MockBackend.prototype, "outputExists").mockResolvedValue(true);
    const patch = vi.spyOn(MockBackend.prototype, "startPatch").mockResolvedValue();
    render(<App />);
    const user = (await import("@testing-library/user-event")).default;
    // Both resource options reach the confirmed patch request.
    await user.click(screen.getByRole("switch", { name: "4-player enemy spawns" }));
    await user.click(screen.getByRole("switch", { name: "Match drops to crowns" }));
    await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
    await user.tab();
    await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("button", { name: "Patch ISO" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(patch).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: confirm ? "Overwrite" : "Cancel" }));
    if (confirm) {
      expect(patch).toHaveBeenCalledExactlyOnceWith("/mock/game [CrownMod].iso", true, true, true, false);
      expect(await screen.findByRole("dialog", { name: "Patching your game" })).toBeTruthy();
    } else {
      expect(patch).not.toHaveBeenCalled();
      expect(screen.queryByRole("dialog")).toBeNull();
    }
  });
});

describe("patch finished popup", () => {
  async function patchOnce() {
    render(<App />);
    const user = (await import("@testing-library/user-event")).default;
    await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
    await user.tab();
    await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("button", { name: "Patch ISO" }));
    return user;
  }

  test("a successful patch opens a popup that OK closes", async () => {
    const user = await patchOnce();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Ready to play");
    expect(dialog.textContent).toContain("/mock/game [CrownMod].iso");
    await user.click(screen.getByRole("button", { name: "OK" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("a refused patch opens an error popup instead of the completion popup", async () => {
    vi.spyOn(MockBackend.prototype, "startPatch").mockImplementation(async function (this: MockBackend) {
      (this as unknown as { emit: (n: string, p: string) => void }).emit("patch-failed", "nope");
    });
    await patchOnce();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain("Unable to patch disc");
    expect(dialog.textContent).toContain("nope");
    expect(screen.queryByText("Ready to play")).toBeNull();
    const user = (await import("@testing-library/user-event")).default;
    await user.click(screen.getByRole("button", { name: "OK" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

 test("a modded-disc popup can be dismissed and a clean selection recovers", async () => {
   vi.spyOn(MockBackend.prototype, "analyze").mockResolvedValueOnce({ kind: "refused", reason: "already-modded", text: "This disc is already modded. Choose an original copy.", size: 0, outputSize: 0, copies: null });
   render(<App />);
   const user = (await import("@testing-library/user-event")).default;
   await user.type(screen.getByLabelText("disc path"), "/mock/modded.iso");
   await user.tab();
   expect((await screen.findByRole("dialog")).textContent).toContain("Disc already modded");
   expect(screen.getByRole("button", { name: "Patch ISO", hidden: true }).hasAttribute("disabled")).toBe(true);
   await user.click(screen.getByRole("button", { name: "OK" }));
   expect(screen.queryByRole("dialog")).toBeNull();
   await user.clear(screen.getByLabelText("disc path"));
   await user.type(screen.getByLabelText("disc path"), "/mock/clean.iso");
   await user.tab();
   await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
   expect(screen.queryByText(/clean mock disc|output: /)).toBeNull();
 });

 test("an immediate patch-start failure opens the error popup", async () => {
   vi.spyOn(MockBackend.prototype, "startPatch").mockRejectedValue(new Error("Unable to create output."));
   render(<App />);
   const user = (await import("@testing-library/user-event")).default;
   await user.type(screen.getByLabelText("disc path"), "/mock/clean.iso");
   await user.tab();
   await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
   await user.click(screen.getByRole("button", { name: "Patch ISO" }));
   expect((await screen.findByRole("dialog")).textContent).toContain("Unable to create output.");
 });

test("an update offer waits until a disc-error popup is dismissed", async () => {
  let resolveCheck: (value: Awaited<ReturnType<MockBackend["checkUpdates"]>>) => void = () => {};
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockImplementation(() => new Promise(resolve => { resolveCheck = resolve; }));
  vi.spyOn(MockBackend.prototype, "analyze").mockResolvedValueOnce({ kind: "unknown", text: "The disc could not be read.", size: 0, outputSize: 0, copies: null });
  render(<App />);
  const user = (await import("@testing-library/user-event")).default;
  await user.type(screen.getByLabelText("disc path"), "/mock/missing.iso");
  await user.tab();
  expect((await screen.findByRole("dialog")).textContent).toContain("Unable to open disc");
  resolveCheck({ currentVersion: "0.9.0", update: { currentVersion: "0.9.0", version: "0.10.0", method: "browser", releases: [{ version: "0.10.0", notes: "New features." }] } });
  await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1));
  expect(screen.queryByText("Update available")).toBeNull();
  await user.click(screen.getByRole("button", { name: "OK" }));
  expect((await screen.findByRole("dialog")).textContent).toContain("Update available");
  await user.click(screen.getByRole("button", { name: "Later" }));
});

test.each(["ok", "refused"] as const)("analysis stays in a modal until the %s result arrives", async (kind) => {
  let finish: (value: Awaited<ReturnType<MockBackend["analyze"]>>) => void = () => {};
  vi.spyOn(MockBackend.prototype, "analyze").mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  render(<App />);
  const user = (await import("@testing-library/user-event")).default;
  await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
  await user.tab();
  const dialog = await screen.findByRole("dialog", { name: "Analyzing disc" });
  expect(dialog.textContent).toContain("Checking your disc");
  expect(screen.getByRole("progressbar", { name: "Analyzing disc" }).hasAttribute("aria-valuenow")).toBe(false);
  expect(screen.queryByText("working…")).toBeNull();
  await user.keyboard("{Escape}");
  expect(screen.getByRole("dialog", { name: "Analyzing disc" })).toBeTruthy();
  finish({ kind, text: kind === "ok" ? "clean" : "Please choose an original disc.", size: 1, outputSize: 2, copies: kind === "ok" ? [665, 665, 665] : null });
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Analyzing disc" })).toBeNull());
  if (kind === "ok") {
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false);
  } else {
    expect(screen.getByRole("dialog", { name: "Unable to open disc" }).textContent).toContain("Please choose an original disc");
  }
});

test.each(["success", "failure"] as const)("patching uses only the modal until %s", async (ending) => {
  const outcome = { bytes: 1000, md5: "fixture", summary: { rowsChanged: 640, liveRows: 580, attackPinned: 0, hpCapped: 0, critPinned: 0, practiceRows: 6, collapsedRows: 0, elfStep: false, serialStep: false } };
  let send: (name: string, payload: unknown) => void = () => {};
  let finish: () => void = () => {};
  vi.spyOn(MockBackend.prototype, "startPatch").mockImplementation(function (this: MockBackend) {
    send = (name, payload) => (this as unknown as { emit: (name: string, payload: unknown) => void }).emit(name, payload);
    return new Promise(resolve => { finish = resolve; });
  });
  render(<App />);
  const user = (await import("@testing-library/user-event")).default;
  await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
  await user.tab();
  await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
  await user.click(screen.getByRole("button", { name: "Patch ISO" }));
  expect(await screen.findByRole("dialog", { name: "Patching your game" })).toBeTruthy();
  expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("0");
  expect(document.querySelector("main [role=progressbar], main progress")).toBeNull();
  expect(screen.queryByText("working…")).toBeNull();
  await user.keyboard("{Escape}");
  expect(screen.getByRole("dialog", { name: "Patching your game" })).toBeTruthy();
  const bar = screen.getByRole("progressbar");
  const message = screen.getByRole("status").textContent;
  // Compressed images unpack, copy, patch, verify, repack and verify again.
  // Overall counters include the compressed round trip. Phase names may repeat;
  // stale events cannot move the bar backwards or finish it before success.
  for (const [phase, done, total, expected] of [
    ["copying", 1500, 10000, 15], ["inspecting", 1500, 10000, 15],
    ["copying", 3000, 10000, 30], ["writing", 5000, 10000, 50],
    ["verifying", 6500, 10000, 65], ["writing", 8500, 10000, 85],
    ["verifying", 9200, 10000, 92], ["writing", 1, 10000, 92],
    ["verifying", 0, 0, 92], ["verifying", NaN, 10000, 92],
    ["verifying", 10000, 10000, 99],
  ] as const) {
    act(() => send("patch-progress", { phase, done, total }));
    expect(screen.getByRole("progressbar")).toBe(bar);
    expect(bar.getAttribute("aria-valuenow")).toBe(String(expected));
    expect(screen.getByText(`${expected}%`)).toBeTruthy();
    expect(screen.getByRole("status").textContent).toBe(message);
  }
  await act(async () => {
    send(ending === "success" ? "patch-done" : "patch-failed", ending === "success" ? outcome : "Please choose another destination.");
    finish();
  });
  expect(await screen.findByRole("dialog", { name: ending === "success" ? "Ready to play" : "Unable to patch disc" })).toBeTruthy();
  expect(screen.queryByRole("dialog", { name: "Patching your game" })).toBeNull();
  if (ending === "failure") {
    await user.click(screen.getByRole("button", { name: "OK" }));
    await user.click(screen.getByRole("button", { name: "Patch ISO" }));
    expect(await screen.findByRole("dialog", { name: "Patching your game" })).toBeTruthy();
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("0");
    await act(async () => {
      send("patch-failed", "Please choose another destination.");
      finish();
    });
  }
});

 test("random crown colours reach patching without reanalyzing or enabling other options", async () => {
   const analyze=vi.spyOn(MockBackend.prototype,"analyze");
   const patch=vi.spyOn(MockBackend.prototype,"startPatch").mockResolvedValue();
   render(<App />);const user=(await import("@testing-library/user-event")).default;
   await user.type(screen.getByLabelText("disc path"),"/mock/game.iso");await user.tab();
   await waitFor(()=>expect(screen.getByRole("button",{name:"Patch ISO"}).hasAttribute("disabled")).toBe(false));
   const before=analyze.mock.calls.length;
   await user.click(screen.getByRole("switch",{name:"Random crown colours"}));
   expect(analyze.mock.calls.length).toBe(before);
   await user.click(screen.getByRole("button",{name:"Patch ISO"}));
   await waitFor(()=>expect(patch).toHaveBeenCalledWith("/mock/game [CrownMod].iso",false,false,false,true));
 });

test("settings refresh output size in the background and only the latest choice enables patching", async () => {
  type Verdict = Awaited<ReturnType<MockBackend["analyze"]>>;
  const verdict: Verdict = { kind: "ok", text: "clean", size: 100, outputSize: 200, copies: [665, 665, 665] };
  const pending: Array<(value: Verdict) => void> = [];
  const analyze = vi.spyOn(MockBackend.prototype, "analyze")
    .mockResolvedValueOnce(verdict)
    .mockImplementation(() => new Promise(resolve => pending.push(resolve)));
  render(<App />);
  const user = (await import("@testing-library/user-event")).default;
  await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
  await user.tab();
  await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
  await user.click(screen.getByRole("switch", { name: "4-player enemy spawns" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByText("Updating output size…")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(true);
  await user.click(screen.getByRole("switch", { name: "Match drops to crowns" }));
  expect(analyze).toHaveBeenLastCalledWith("/mock/game.iso", true, true);
  expect(screen.queryByRole("dialog")).toBeNull();
  await act(async () => pending[0]!({ ...verdict, kind: "refused", text: "stale result" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(true);
  await act(async () => pending[1]!({ ...verdict, outputSize: 300 }));
  expect(screen.queryByText("Updating output size…")).toBeNull();
  expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false);
});

test("a failed background size refresh still opens the disc-error popup", async () => {
  let finish: (value: Awaited<ReturnType<MockBackend["analyze"]>>) => void = () => {};
  vi.spyOn(MockBackend.prototype, "analyze")
    .mockResolvedValueOnce({ kind: "ok", text: "clean", size: 100, outputSize: 200, copies: [665, 665, 665] })
    .mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  render(<App />);
  const user = (await import("@testing-library/user-event")).default;
  await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
  await user.tab();
  await waitFor(() => expect(screen.getByRole("button", { name: "Patch ISO" }).hasAttribute("disabled")).toBe(false));
  await user.click(screen.getByRole("switch", { name: "4-player enemy spawns" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  await act(async () => finish({ kind: "unknown", text: "The disc could not be read.", size: 0, outputSize: 0, copies: null }));
  expect(screen.getByRole("dialog", { name: "Unable to open disc" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Patch ISO", hidden: true }).hasAttribute("disabled")).toBe(true);
});
