/** One `patch-progress` event: internal phase plus estimated overall work counters. */
export interface ProgressEvent {
  phase:
    | "inspecting"
    | "unpacking"
    | "restoring"
    | "building"
    | "randomisingEnemies"
    | "randomisingBosses"
    | "recording"
    | "copying"
    | "writing"
    | "packing"
    | "verifying";
  done: number;
  total: number;
  /** The enemy area being planned (1-based) and how many there are. */
  area?: number;
  areas?: number;
  /** The boss difficulty being shuffled. */
  difficulty?: "normal" | "hard" | "veryHard";
}
