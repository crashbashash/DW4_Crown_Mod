import type { ProgressEvent } from "../../bindings";

const DIFFICULTIES = { normal: "Normal", hard: "Hard", veryHard: "Very Hard" } as const;

/** The patching dialog's description of the step under way. */
export function progressStatus(event: ProgressEvent): string {
  switch (event.phase) {
    case "inspecting": return "Checking your disc…";
    case "unpacking": return "Unpacking the disc image…";
    case "restoring": return "Restoring the original game files…";
    case "building": return "Preparing game files…";
    case "randomisingEnemies":
      return event.area !== undefined && event.areas !== undefined
        ? `Randomising enemies: area ${event.area} of ${event.areas}…`
        : "Randomising enemies…";
    case "randomisingBosses":
      return event.difficulty !== undefined
        ? `Randomising bosses: ${DIFFICULTIES[event.difficulty]}…`
        : "Randomising bosses…";
    case "recording": return "Saving update data…";
    case "copying": return "Copying game files…";
    case "writing": return "Writing your new game…";
    case "packing": return "Compressing the disc image…";
    case "verifying": return "Verifying your new game…";
    default: return "Working…";
  }
}
