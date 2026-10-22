import { expect, test } from "vitest";
import { isSuggestedName, withSeed } from "./outputName";

test("adds, replaces and removes the seed in a suggested name", () => {
  expect(withSeed("/games/Digimon World 4 (USA) [CrownMod].iso", 1234)).toBe("/games/Digimon World 4 (USA) [CrownMod] [Seed 1234].iso");
  expect(withSeed("/games/Digimon World 4 (USA) [CrownMod] [Seed 1234].iso", 99)).toBe("/games/Digimon World 4 (USA) [CrownMod] [Seed 99].iso");
  expect(withSeed("/games/Digimon World 4 (USA) [CrownMod] [Seed 1234].ciso", null)).toBe("/games/Digimon World 4 (USA) [CrownMod].ciso");
  expect(withSeed("C:\\games\\game [CrownMod].rvz", 7)).toBe("C:\\games\\game [CrownMod] [Seed 7].rvz");
});

test("leaves a name the user chose alone", () => {
  for (const path of ["/games/my own name.iso", "/games/game [CrownMod] extra.iso", "/games/[CrownMod] game.iso", "/games/dir.d/game"]) {
    expect(isSuggestedName(path)).toBe(false);
    expect(withSeed(path, 5)).toBe(path);
  }
  expect(isSuggestedName("/games/game [CrownMod].iso")).toBe(true);
  expect(isSuggestedName("/games/game [CrownMod] [Seed 3].iso")).toBe(true);
});
