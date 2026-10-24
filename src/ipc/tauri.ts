import { DEFAULT_PATCH_SELECTION, type PatchSelection } from "../bindings/PatchSelection";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DiscStatus, PatchOutcome, PlanSummary, ProgressEvent } from "../bindings";
import type { UpdateCheck } from "../bindings/UpdateCheck";
import type { Backend } from "./backend";

/** The real backend: Tauri invoke + event listen. */
export class TauriBackend implements Backend {
  async checkUpdates(): Promise<UpdateCheck> {
    return invoke<UpdateCheck>("check_updates");
  }

  async acceptUpdate(version: string): Promise<"browser" | "installer"> {
    return invoke("accept_update", { version });
  }
  async chooseIso(): Promise<string | null> {
    return invoke<string | null>("choose_iso");
  }

  async chooseOutput(defaultName: string): Promise<string | null> {
    return invoke<string | null>("choose_output", { defaultName });
  }

  async analyze(path: string, selection: PatchSelection = DEFAULT_PATCH_SELECTION): Promise<DiscStatus> {
    return invoke<DiscStatus>("analyze", { path, selection });
  }

  async defaultOutput(input: string): Promise<string> {
    return invoke<string>("default_output", { input });
  }

  async outputExists(output: string): Promise<boolean> {
    return invoke<boolean>("output_exists", { output });
  }

  async planSummary(): Promise<PlanSummary | null> {
    return invoke<PlanSummary | null>("plan_summary");
  }

  async startPatch(output: string, overwrite: boolean, selection: PatchSelection = DEFAULT_PATCH_SELECTION): Promise<void> {
    await invoke("start_patch", { output, overwrite, selection });
  }

  async updateMod(selection: PatchSelection, original: string | null): Promise<void> {
    await invoke("update_mod", { selection, original });
  }

  async onPatchProgress(handler: (p: ProgressEvent) => void): Promise<() => void> {
    return listen<ProgressEvent>("patch-progress", (event) => handler(event.payload));
  }

  async onPatchDone(handler: (o: PatchOutcome) => void): Promise<() => void> {
    return listen<PatchOutcome>("patch-done", (event) => handler(event.payload));
  }

  async onPatchFailed(handler: (message: string) => void): Promise<() => void> {
    return listen<string>("patch-failed", (event) => handler(event.payload));
  }

  async onFileDropped(handler: (path: string) => void): Promise<() => void> {
    const unlisten = await getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === "drop" && event.payload.paths.length > 0) {
        const iso = event.payload.paths.find((p) => /\.(iso|gcm|ciso|rvz)$/i.test(p));
        if (iso !== undefined) handler(iso);
      }
    });
    return unlisten as UnlistenFn;
  }
}
