import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import { DiscPicker } from "./DiscPicker";

function setup() {
  const pickDisc = vi.fn();
  render(
    <DiscPicker
      inputPath={null}
      onPick={pickDisc}
      onCommitDiscPath={() => {}}
    />,
  );
  return pickDisc;
}

describe("DiscPicker", () => {
  test("idle prompt invites the dialog or a drag", () => {
    setup();
    expect(screen.getByText(/choose a disc/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: /browse/i })).toBeTruthy();
  });

  test("Browse triggers the pick callback", async () => {
    const pickDisc = setup();
    await userEvent.click(screen.getByRole("button", { name: /browse/i }));
    expect(pickDisc).toHaveBeenCalled();
  });

});

 test.each([
   [{ console: "PS2", region: "Europe", modded: false, modVersion: null }, "Unmodified"],
   [{ console: "GameCube", region: "Japan", modded: true, modVersion: "0.11.0" }, "Crown Mod v0.11.0"],
   [{ console: "PS2", region: "USA", modded: true, modVersion: null }, "Modded · version unknown"],
 ] as const)("disc information reflects the detected image", (info, status) => {
   render(<DiscPicker inputPath="/mock/game.iso" info={info} onPick={() => {}} onCommitDiscPath={() => {}} />);
   expect(screen.getByText(info.console)).toBeTruthy();
   expect(screen.getByText(info.region)).toBeTruthy();
   expect(screen.getByText(status)).toBeTruthy();
 });
