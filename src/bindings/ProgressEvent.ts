/** One `patch-progress` event: internal phase plus estimated overall work counters. */
export interface ProgressEvent {
  phase: "inspecting" | "copying" | "writing" | "verifying";
  done: number;
  total: number;
}
