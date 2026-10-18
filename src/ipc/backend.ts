import type { UpdateCheck } from "../bindings/UpdateCheck";
import type { DiscStatus, PatchOutcome, PlanSummary, ProgressEvent } from "../bindings";

/**
 * Everything the UI can ask of the shell, through one interface. Components
 * never import `@tauri-apps/api` directly; they get a `Backend` from
 * `useBackend()`, which is the Tauri implementation in the desktop app and
 * the mock in a plain browser tab and in tests.
 */
export interface Backend {
  checkUpdates(): Promise<UpdateCheck>;
  acceptUpdate(version: string): Promise<"browser" | "installer">;
  chooseIso(): Promise<string | null>;
  chooseOutput(defaultName: string): Promise<string | null>;
  analyze(path: string, fourPlayerSpawns?: boolean, crownDrops?: boolean): Promise<DiscStatus>;
  defaultOutput(input: string): Promise<string>;
  outputExists(output: string): Promise<boolean>;
  planSummary(): Promise<PlanSummary | null>;
  startPatch(output: string, overwrite: boolean, fourPlayerSpawns?: boolean, crownDrops?: boolean, randomCrownColours?: boolean): Promise<void>;
  onPatchProgress(handler: (p: ProgressEvent) => void): Promise<() => void>;
  onPatchDone(handler: (o: PatchOutcome) => void): Promise<() => void>;
  onPatchFailed(handler: (message: string) => void): Promise<() => void>;
  /** A disc image (.iso, .gcm, .ciso or .rvz) dropped onto the window (desktop only; mock never fires it). */
  onFileDropped(handler: (path: string) => void): Promise<() => void>;
}
