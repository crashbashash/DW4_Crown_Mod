import { useCallback, useEffect, useRef, useState } from "react";
import type { DiscStatus } from "../../bindings";
import type { Backend } from "../../ipc/backend";

type Selection = { path: string; epoch: number };
type Analysis = { selection: Selection; fourPlayerSpawns: boolean; crownDrops: boolean; status: DiscStatus };

/** Reinspect when the disc or resource options change, preserving a chosen output. */
export function useDisc(backend: Backend, fourPlayerSpawns = false, crownDrops = false, onIdentified?: (status: DiscStatus, epoch: number) => void) {
  const [selection, setSelection] = useState<Selection | null>(null);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const pathEpoch = useRef(0);
  const inputPath = selection?.path ?? null;
  const moddedSelection = analysis?.selection === selection && analysis?.status.reason === "already-modded";
  const current = moddedSelection || (analysis?.selection === selection && analysis?.fourPlayerSpawns === fourPlayerSpawns && analysis?.crownDrops === crownDrops);
  const status = current ? analysis?.status ?? null : null;
  const checking = selection !== null && !current;
  // A new disc needs the blocking analysis dialog. Option changes only refresh
  // the output estimate, so the settings remain usable while that check runs.
  const analyzing = checking && analysis?.selection !== selection;
  const refreshing = checking && !analyzing;

  useEffect(() => {
    if (selection === null || moddedSelection) return;
    let cancelled = false;
    void backend.analyze(selection.path, fourPlayerSpawns, crownDrops)
      .then((status) => {
        if (!cancelled) {
          setAnalysis({ selection, fourPlayerSpawns, crownDrops, status });
          onIdentified?.(status, selection.epoch);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) setAnalysis({ selection, fourPlayerSpawns, crownDrops, status: {
          kind: "unknown", text: String(error), size: 0, outputSize: 0, copies: null,
        } });
      });
    return () => { cancelled = true; };
  }, [backend, selection, fourPlayerSpawns, crownDrops, moddedSelection, onIdentified]);

  const setInputPath = useCallback(async (path: string) => {
    const epoch = ++pathEpoch.current;
    setSelection({ path, epoch });
    const output = await backend.defaultOutput(path);
    if (epoch === pathEpoch.current) setOutputPath(output);
  }, [backend]);

  const pickDisc = useCallback(async () => {
    const path = await backend.chooseIso();
    if (path !== null) await setInputPath(path);
  }, [backend, setInputPath]);

  const replaceStatus = useCallback((expected: DiscStatus, status: DiscStatus) => {
    setAnalysis(previous => previous?.status === expected ? { ...previous, status } : previous);
  }, []);

  const identifiedStatus = analysis?.selection === selection ? analysis?.status ?? null : null;
  return { replaceStatus, identifiedStatus, inputPath, outputPath, status, analyzing, refreshing, checking, pickDisc, setInputPath, setOutputPath };
}
