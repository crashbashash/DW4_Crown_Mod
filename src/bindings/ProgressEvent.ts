/** One `patch-progress` event: writer phase plus progress counters (bytes or regions, depending on phase). */
export interface ProgressEvent {
  phase: "inspecting" | "copying" | "writing" | "verifying";
  done: number;
  total: number;
}
