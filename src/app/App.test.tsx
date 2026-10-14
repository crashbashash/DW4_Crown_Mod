import { render, screen, waitFor } from "@testing-library/react";
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
    expect(screen.getByRole("heading", { name: "DW4 Very Hard Plus" })).toBeTruthy();
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
  test("offers only the base mode without editable difficulty settings", () => {
    render(<App />);
    expect(screen.queryByRole("heading", { name: "Very Hard Plus" })).toBeNull();
    expect(screen.queryByRole("checkbox")).toBeNull();
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
    await waitFor(() => expect(screen.getByText(/clean mock disc/)).toBeTruthy());
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
      ).toBe("/mock/mygame [VeryHardPlus].iso"),
    );
  });
});

describe("overwrite confirmation", () => {
  test.each([false, true])("existing output asks before patching; confirm=%s", async (confirm) => {
    vi.spyOn(MockBackend.prototype, "outputExists").mockResolvedValue(true);
    const patch = vi.spyOn(MockBackend.prototype, "startPatch").mockResolvedValue();
    render(<App />);
    const user = (await import("@testing-library/user-event")).default;
    await user.type(screen.getByLabelText("disc path"), "/mock/game.iso");
    await user.tab();
    await waitFor(() => expect(screen.getByText(/clean mock disc/)).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "Patch ISO" }));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(patch).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: confirm ? "Overwrite and patch" : "Cancel" }));
    if (confirm) expect(patch).toHaveBeenCalledExactlyOnceWith("/mock/game [VeryHardPlus].iso", true);
    else expect(patch).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
