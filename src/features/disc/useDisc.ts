import { DEFAULT_PATCH_SELECTION, type PatchSelection } from "../../bindings/PatchSelection";
import { useCallback, useEffect, useRef, useState } from "react";
import type { DiscStatus } from "../../bindings";
import type { Backend } from "../../ipc/backend";
import { outputExtension } from "../output/outputName";

type Selection = { path: string; epoch: number };
/** Reinspect when the disc or resource options change, preserving a chosen output. */
export type ResourceSelection = Pick<PatchSelection, "crownMod" | "fourPlayerSpawns" | "crownDrops" | "soloButtons" | "randomEnemies" | "randomBosses" | "fixDeathSoftlock">;
type Analysis = { selection: Selection; options: ResourceSelection; status: DiscStatus };
/** A modded image's update size for one selection; `null` when it could not be estimated. */
type UpdateEstimate = { selection: Selection; options: ResourceSelection; size: number | null };

const sameOptions = (a: ResourceSelection, b: ResourceSelection) =>
  a.crownMod === b.crownMod && a.fourPlayerSpawns === b.fourPlayerSpawns && a.crownDrops === b.crownDrops && a.soloButtons === b.soloButtons
  && a.randomEnemies === b.randomEnemies && a.randomBosses === b.randomBosses && a.fixDeathSoftlock === b.fixDeathSoftlock;

export function useDisc(backend: Backend, selectionOptions: ResourceSelection = DEFAULT_PATCH_SELECTION, onIdentified?: (status: DiscStatus, epoch: number) => void) {
  const { crownMod, fourPlayerSpawns, crownDrops, soloButtons, randomEnemies, randomBosses, fixDeathSoftlock } = selectionOptions;
  const [selection, setSelection] = useState<Selection | null>(null);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [extension, setExtension] = useState("iso");
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
  const [estimate, setEstimate] = useState<UpdateEstimate | null>(null);
  const pathEpoch = useRef(0);
  const selectionRef = useRef(selection);
  useEffect(() => {
    selectionRef.current = selection;
  }, [selection]);
  const inputPath = selection?.path ?? null;
  const moddedSelection = analysis?.selection === selection && analysis?.status.reason === "already-modded";
  const options: ResourceSelection = { crownMod, fourPlayerSpawns, crownDrops, soloButtons, randomEnemies, randomBosses, fixDeathSoftlock };
  const current = moddedSelection || (analysis?.selection === selection && sameOptions(analysis.options, options));
  const status = current ? analysis?.status ?? null : null;
  const checking = selection !== null && !current;
  // A new disc needs the blocking analysis dialog. Option changes only refresh
  // the output estimate, so the settings remain usable while that check runs.
  const analyzing = checking && analysis?.selection !== selection;
  // A modded image keeps its verdict; option changes refresh only its update size.
  const estimated = estimate?.selection === selection && sameOptions(estimate.options, options);
  const updateSize = moddedSelection && estimated ? estimate.size : null;
  const refreshing = (checking && !analyzing) || (moddedSelection && !estimated);

  useEffect(() => {
    if (selection === null || moddedSelection) return;
    let cancelled = false;
    const options = { crownMod, fourPlayerSpawns, crownDrops, soloButtons, randomEnemies, randomBosses, fixDeathSoftlock };
    void backend.analyze(selection.path, { ...DEFAULT_PATCH_SELECTION, ...options })
      .then((status) => {
        if (!cancelled) {
          setAnalysis({ selection, options, status });
          onIdentified?.(status, selection.epoch);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) setAnalysis({ selection, options, status: {
          kind: "unknown", text: String(error), size: 0, outputSize: 0, copies: null,
        } });
      });
    return () => { cancelled = true; };
  }, [backend, selection, fourPlayerSpawns, crownDrops, crownMod, soloButtons, randomEnemies, randomBosses, fixDeathSoftlock, moddedSelection, onIdentified]);

  useEffect(() => {
    if (selection === null || !moddedSelection) return;
    let cancelled = false;
    const options = { crownMod, fourPlayerSpawns, crownDrops, soloButtons, randomEnemies, randomBosses, fixDeathSoftlock };
    void backend.estimateUpdate(selection.path, { ...DEFAULT_PATCH_SELECTION, ...options })
      .catch(() => null)
      .then((size) => {
        if (!cancelled) setEstimate({ selection, options, size });
      });
    return () => { cancelled = true; };
  }, [backend, selection, fourPlayerSpawns, crownDrops, crownMod, soloButtons, randomEnemies, randomBosses, fixDeathSoftlock, moddedSelection]);

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

  /** Follow a file the app renamed, keeping its verdict instead of analysing it again. */
  const renameInput = useCallback((from: string, to: string) => {
    const current = selectionRef.current;
    if (current?.path !== from) return;
    const renamed = { path: to, epoch: current.epoch };
    selectionRef.current = renamed;
    setSelection(renamed);
    setAnalysis(previous => previous?.selection === current ? { ...previous, selection: renamed } : previous);
    setEstimate(previous => previous?.selection === current ? { ...previous, selection: renamed } : previous);
  }, []);

  const identifiedStatus = analysis?.selection === selection ? analysis?.status ?? null : null;
  return { replaceStatus, identifiedStatus, inputPath, outputPath, extension, status, updateSize, analyzing, refreshing, checking, pickDisc, setInputPath, setOutputPath, renameInput };
}
