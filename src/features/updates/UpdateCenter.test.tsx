import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { BackendProvider } from "../../ipc/context";
import { MockBackend } from "../../ipc/mock";
import type { UpdateCheck } from "../../bindings/UpdateCheck";
import { UpdateCenter } from "./UpdateCenter";

const result: UpdateCheck = { currentVersion: "0.9.0", update: {
  currentVersion: "0.9.0", version: "0.10.0", method: "browser",
  releases: [{ version: "0.10.0", notes: "Custom drops and four-player enemy spawns." }, { version: "0.9.1", notes: "Earlier fixes." }],
} };
afterEach(() => vi.restoreAllMocks());
function setup(blocked = false) {
  const busy = vi.fn();
  const view = render(<BackendProvider><UpdateCenter blocked={blocked} onBusyChange={busy} /></BackendProvider>);
  return { ...view, busy };
}
test("startup shows changes before opening the browser, and Later declines", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockResolvedValue(result);
  const accept = vi.spyOn(MockBackend.prototype, "acceptUpdate");
  setup();
  const dialog = await screen.findByRole("dialog");
  expect(dialog.textContent).toContain("Custom drops");
  expect(dialog.textContent).toContain("Earlier fixes");
  expect(accept).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Later" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(accept).not.toHaveBeenCalled();
});
test("portable/Linux acceptance opens the selected release", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockResolvedValue(result);
  const accept = vi.spyOn(MockBackend.prototype, "acceptUpdate").mockResolvedValue("browser");
  const { busy } = setup();
  await userEvent.click(await screen.findByRole("button", { name: "Open GitHub release" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(accept).toHaveBeenCalledExactlyOnceWith("0.10.0");
  expect(busy.mock.calls).toEqual([[true], [false]]);
});
test("MSI acceptance is explicit and installer handoff keeps patching disabled", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockResolvedValue({ ...result, update: { ...result.update!, method: "msi" } });
  const accept = vi.spyOn(MockBackend.prototype, "acceptUpdate").mockResolvedValue("installer");
  const { busy } = setup();
  expect((await screen.findByRole("dialog")).textContent).toContain("The app will close");
  expect(accept).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Download and update" }));
  await waitFor(() => expect(busy).toHaveBeenCalledWith(true));
  expect(screen.getByRole("button", { name: "Later" }).hasAttribute("disabled")).toBe(true);
  expect(busy).not.toHaveBeenCalledWith(false);
});
test("an available update waits until the current patch/dialog is finished", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockResolvedValue(result);
  const view = setup(true);
  await waitFor(() => expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy());
  expect(screen.queryByRole("dialog")).toBeNull();
  view.rerender(<BackendProvider><UpdateCenter blocked={false} onBusyChange={vi.fn()} /></BackendProvider>);
  expect(await screen.findByRole("dialog")).toBeTruthy();
});
test("offline startup stays quiet and a manual retry explains the failure", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockRejectedValue(new Error("offline"));
  setup();
  await waitFor(() => expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy());
  expect(screen.queryByRole("dialog")).toBeNull();
  await userEvent.click(screen.getByRole("button", { name: "Check for updates" }));
  expect((await screen.findByRole("dialog")).textContent).toContain("Check your internet connection");
});
test("failed update can be retried or dismissed without losing the offer", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockResolvedValue(result);
  vi.spyOn(MockBackend.prototype, "acceptUpdate").mockRejectedValue(new Error("Download failed."));
  const { busy } = setup();
  await userEvent.click(await screen.findByRole("button", { name: "Open GitHub release" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Download failed");
  expect(busy).toHaveBeenLastCalledWith(false);
  await userEvent.click(screen.getByRole("button", { name: "Later" }));
  expect(screen.queryByRole("dialog")).toBeNull();
});
test("manual checks report the installed version when no update is available", async () => {
  setup();
  await waitFor(() => expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy());
  await userEvent.click(screen.getByRole("button", { name: "Check for updates" }));
  expect((await screen.findByRole("dialog")).textContent).toContain("You're up to date");
});


test("an older Linux build shows the newer version and release notes", async () => {
  vi.spyOn(MockBackend.prototype, "checkUpdates").mockResolvedValue({ currentVersion: "0.10.0", update: {
    currentVersion: "0.10.0", version: "0.11.0", method: "browser",
    releases: [{ version: "0.11.0", notes: "Crowned river turrets and reduced plug drops." }],
  } });
  setup();
  const dialog = await screen.findByRole("dialog");
  expect(dialog.textContent).toContain("DW4 Crown Mod 0.11.0 is available. You're using 0.10.0.");
  expect(dialog.textContent).toContain("Crowned river turrets");
  expect(screen.getByRole("button", { name: "Open GitHub release" })).toBeTruthy();
});
