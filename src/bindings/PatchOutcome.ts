/** The `patch-done` payload: verified result of the write. */
export interface PatchOutcome {
  md5: string;
  bytes: number;
  summary: import("./PlanSummary").PlanSummary;
  /** The image's new path when an in-place update renamed it to its new seed. */
  renamed?: string;
}
