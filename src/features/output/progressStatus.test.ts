import { describe, expect, test } from "vitest";
import { progressStatus } from "./progressStatus";

describe("progressStatus", () => {
  test("counts enemy areas", () => {
    expect(progressStatus({ phase: "randomisingEnemies", done: 1, total: 2, area: 12, areas: 74 }))
      .toBe("Randomising enemies: area 12 of 74…");
    expect(progressStatus({ phase: "randomisingEnemies", done: 1, total: 2 })).toBe("Randomising enemies…");
  });

  test("names the boss difficulty", () => {
    expect(progressStatus({ phase: "randomisingBosses", done: 1, total: 2, difficulty: "veryHard" }))
      .toBe("Randomising bosses: Very Hard…");
  });

  test("describes update and container steps", () => {
    expect(progressStatus({ phase: "restoring", done: 1, total: 2 })).toBe("Restoring the original game files…");
    expect(progressStatus({ phase: "unpacking", done: 1, total: 2 })).toBe("Unpacking the disc image…");
    expect(progressStatus({ phase: "packing", done: 1, total: 2 })).toBe("Compressing the disc image…");
  });
});
