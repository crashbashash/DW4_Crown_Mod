import { DEFAULT_PATCH_SELECTION, type PatchSelection } from "../../bindings/PatchSelection";
import { useCallback, useEffect, useRef, useState } from "react";
import type { DiscStatus } from "../../bindings";
import type { Backend } from "../../ipc/backend";
import { outputExtension } from "../output/outputName";

type Selection = { path: string; epoch: number };
type Analysis = { crownMod: boolean; selection: Selection; fourPlayerSpawns: boolean; crownDrops: boolean; status: DiscStatus };

/** Reinspect when the disc or resource options change, preserving a chosen output. */
export type ResourceSelection = Pick<PatchSelection, "crownMod" | "fourPlayerSpawns" | "crownDrops">;

export function useDisc(backend: Backend, selectionOptions: ResourceSelection = DEFAULT_PATCH_SELECTION, onIdentified?: (status: DiscStatus, epoch: number) => void) {
  const { crownMod, fourPlayerSpawns, crownDrops } = selectionOptions;
  const [selection, setSelection] = useState<Selection | null>(null);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [extension, setExtension] = useState("iso");
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const pathEpoch = useRef(0);
  const inputPath = selection?.path ?? null;
  const moddedSelection = analysis?.selection === selection && analysis?.status.reason === "already-modded";
  const current = moddedSelection || (analysis?.selection === selection && analysis?.fourPlayerSpawns === fourPlayerSpawns && analysis?.crownDrops === crownDrops && analysis?.crownMod === crownMod);
  const status = current ? analysis?.status ?? null : null;
  const checking = selection !== null && !current;
  // A new disc needs the blocking analysis dialog. Option changes only refresh
  // the output estimate, so the settings remain usable while that check runs.
  const analyzing = checking && analysis?.selection !== selection;
  const refreshing = checking && !analyzing;

  useEffect(() => {
    if (selection === null || moddedSelection) return;
    let cancelled = false;
    void backend.analyze(selection.path, { ...DEFAULT_PATCH_SELECTION, fourPlayerSpawns, crownDrops, crownMod })
      .then((status) => {
        if (!cancelled) {
          setAnalysis({ selection, fourPlayerSpawns, crownDrops, crownMod, status });
          onIdentified?.(status, selection.epoch);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) setAnalysis({ selection, fourPlayerSpawns, crownDrops, crownMod, status: {
          kind: "unknown", text: String(error), size: 0, outputSize: 0, copies: null,
        } });
      });
    return () => { cancelled = true; };
  }, [backend, selection, fourPlayerSpawns, crownDrops, crownMod, moddedSelection, onIdentified]);

  const setInputPath = useCallback(async (path: string) => {
    const epoch = ++pathEpoch.current;
    setSelection({ path, epoch });
    const output = await backend.defaultOutput(path);
    if (epoch === pathEpoch.current) {
      setOutputPath(output);
      setExtension(outputExtension(output));
    }
  }, [backend]);

  const pickDisc = useCallback(async () => {
    const path = await backend.chooseIso();
    if (path !== null) await setInputPath(path);
  }, [backend, setInputPath]);

  const replaceStatus = useCallback((expected: DiscStatus, status: DiscStatus) => {
    setAnalysis(previous => previous?.status === expected ? { ...previous, status } : previous);
  }, []);

  const identifiedStatus = analysis?.selection === selection ? analysis?.status ?? null : null;
  return { replaceStatus, identifiedStatus, inputPath, outputPath, extension, status, analyzing, refreshing, checking, pickDisc, setInputPath, setOutputPath };
}
