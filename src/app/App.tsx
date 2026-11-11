import { DEFAULT_PATCH_SELECTION, type PatchSelection } from "../bindings/PatchSelection";
import { useCallback, useEffect, useRef, useState } from "react";
import type { DiscStatus } from "../bindings";
import { UpdateModDialog } from "../features/disc/UpdateModDialog";
import { AnalyzingDialog } from "../features/disc/AnalyzingDialog";
import { DiscPicker } from "../features/disc/DiscPicker";
import { useDisc } from "../features/disc/useDisc";
import { DoneDialog } from "../features/output/DoneDialog";
import { OverwriteDialog } from "../features/output/OverwriteDialog";
import { PatchingDialog } from "../features/output/PatchingDialog";
import { progressStatus } from "../features/output/progressStatus";
import { OutputPanel } from "../features/output/OutputPanel";
import { SettingsFiles } from "../features/settings/SettingsFiles";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import { newSeed, parseSeed } from "../features/settings/seed";
import { isSuggestedName, withOutputExtension, withSeed } from "../features/output/outputName";
import { UpdateCenter } from "../features/updates/UpdateCenter";
import { SupportLink } from "../components/SupportLink";
import { MessageDialog } from "../components/MessageDialog";
import { BackendProvider, useBackend } from "../ipc/context";
import "./theme.css";
import "./layout.css";

function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const mql = window.matchMedia(query);
    const onChange = (e: MediaQueryListEvent) => setMatches(e.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);
  return matches;
}

/** Disc selection and output for the selected patch options. */
export function App() {
  return (
    <BackendProvider>
      <AppInner />
    </BackendProvider>
  );
}

function AppInner() {
  const backend = useBackend();
  const [crownMod, setCrownMod] = useState(DEFAULT_PATCH_SELECTION.crownMod);
  const [fourPlayerSpawns, setFourPlayerSpawns] = useState(DEFAULT_PATCH_SELECTION.fourPlayerSpawns);
  const [crownDrops, setCrownDrops] = useState(DEFAULT_PATCH_SELECTION.crownDrops);
  const [randomCrownColours, setRandomCrownColours] = useState(DEFAULT_PATCH_SELECTION.randomCrownColours);
  const [randomEnemies, setRandomEnemies] = useState(DEFAULT_PATCH_SELECTION.randomEnemies);
  const [randomBosses, setRandomBosses] = useState(DEFAULT_PATCH_SELECTION.randomBosses);
  const [enemySeed, setEnemySeed] = useState("");
  const [soloButtons, setSoloButtons] = useState(DEFAULT_PATCH_SELECTION.soloButtons);
  const appliedOptionsEpoch = useRef<number | null>(null);
  const applyDetectedOptions = useCallback((status: DiscStatus, epoch: number) => {
    if (appliedOptionsEpoch.current === epoch) return;
    appliedOptionsEpoch.current = epoch;
    if (status.reason === "already-modded") {
      const options = status.modMetadata?.options;
      setFourPlayerSpawns(options?.fourPlayerSpawns ?? false);
      setCrownMod(options?.crownMod ?? true);
      setCrownDrops((options?.crownMod ?? true) && (options?.betterLoot ?? false));
      setRandomCrownColours(options?.randomCrownColours ?? false);
      const seed = status.modMetadata?.randomEnemies?.seed ?? options?.enemySeed;
      setRandomEnemies(options?.randomEnemies ?? false);
      setRandomBosses(options?.randomEnemies === true && options?.randomBosses === true);
      if (options?.randomEnemies === true && typeof seed === "number") setEnemySeed(String(seed));
      setSoloButtons(options?.soloButtons ?? false);
    }
  }, []);
  const disc = useDisc(backend, { fourPlayerSpawns, crownDrops, crownMod, soloButtons }, applyDetectedOptions);
  const replaceDiscStatus = disc.replaceStatus;
  const renameInput = disc.renameInput;
  const wide = useMediaQuery("(min-width: 900px)");

  const [settingsBusy, setSettingsBusy] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [dismissedDiscStatus, setDismissedDiscStatus] = useState<DiscStatus | null>(null);
  const [updateRequestedFor, setUpdateRequestedFor] = useState<DiscStatus | null>(null);
  const selectedStatus = disc.status ?? disc.identifiedStatus;
  const discError = disc.status !== null && disc.status.kind !== "ok" && disc.status !== dismissedDiscStatus
    && (disc.status.reason !== "already-modded" || updateRequestedFor === disc.status) ? disc.status : null;
  const [overwritePath, setOverwritePath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [patchProgress, setPatchProgress] = useState(0);
  const [patchStatus, setPatchStatus] = useState<string | undefined>(undefined);
  const [updatingMod, setUpdatingMod] = useState(false);
  const furthestProgress = useRef(0);
  const [patchError, setPatchError] = useState<string | null>(null);
  const [donePopup, setDonePopup] = useState<string | null>(null);
  const outputRef = useRef<string | null>(null);
  const pendingModUpdate = useRef<{ path: string; status: DiscStatus; selection: PatchSelection } | null>(null);

  // Patch events: progress streams, one terminal event ends the run.
  useEffect(() => {
    const ups: Promise<() => void>[] = [];
    ups.push(
      backend.onPatchProgress((event) => {
        const { done, total } = event;
        setBusy(true);
        if (total > 0 && Number.isFinite(done) && Number.isFinite(total)) {
          // Only the verified terminal event completes the operation.
          const percentage = Math.min(99, Math.max(0, Math.floor(done / total * 100)));
          // A stale event moves neither the bar nor the step text.
          if (percentage >= furthestProgress.current) {
            furthestProgress.current = percentage;
            setPatchStatus(progressStatus(event));
          }
          setPatchProgress(previous => Math.max(previous, percentage));
        }
      }),
    );
    ups.push(
      backend.onPatchDone((outcome) => {
        const updated = pendingModUpdate.current;
        pendingModUpdate.current = null;
        // An updated image named after its seed was renamed to the new seed.
        const path = updated && outcome.renamed ? outcome.renamed : null;
        if (updated && path !== null) {
          renameInput(updated.path, path);
          outputRef.current = path;
        }
        setPatchProgress(100);
        setDonePopup(outputRef.current ?? "");
        setBusy(false);
        if (updated) {
          void backend.analyze(path ?? updated.path, updated.selection).catch((error: unknown): DiscStatus => ({
            kind: "unknown", text: String(error), size: 0, outputSize: 0, copies: null,
          })).then(status => {
            replaceDiscStatus(updated.status, status);
            setDismissedDiscStatus(status);
          });
        }
      }),
    );
    ups.push(
      backend.onPatchFailed((message) => {
        pendingModUpdate.current = null;
        setBusy(false);
        setPatchError(message);
      }),
    );
    return () => {
      for (const up of ups) void up.then((unlisten) => unlisten());
    };
  }, [backend, replaceDiscStatus, renameInput]);

  // An ISO dropped onto the window is a chosen disc.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void backend.onFileDropped((path) => {
      if (!busy && !updating && !settingsBusy && !disc.analyzing) void disc.setInputPath(path);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [backend, disc, busy, updating, settingsBusy]);

  const info = selectedStatus?.discInfo;
  const enemySeedValue = randomEnemies ? parseSeed(enemySeed) : null;
  const selection: PatchSelection = { crownMod, fourPlayerSpawns, crownDrops, randomCrownColours, randomEnemies, randomBosses, enemySeed: enemySeedValue, soloButtons };
  const analyzed = disc.status?.kind === "ok";
  const hasOptions = crownMod || fourPlayerSpawns || randomCrownColours || randomEnemies || soloButtons;
  const canPatch = hasOptions && (analyzed || selectedStatus?.reason === "already-modded")
    && disc.outputPath !== null && (!randomEnemies || enemySeedValue !== null);
  const changeCrownMod = (selected: boolean) => {
    setCrownMod(selected);
    if (!selected) setCrownDrops(false);
  };

  // A suggested output name carries the enemy seed; a path the user chose is left alone.
  const { outputPath, setOutputPath } = disc;
  useEffect(() => {
    if (outputPath === null || !isSuggestedName(outputPath)) return;
    const wanted = withSeed(outputPath, enemySeedValue);
    if (wanted !== outputPath) setOutputPath(wanted);
  }, [outputPath, setOutputPath, enemySeedValue]);

  const changeRandomEnemies = (selected: boolean) => {
    setRandomEnemies(selected);
    if (!selected) setRandomBosses(false);
    if (selected && parseSeed(enemySeed) === null) setEnemySeed(newSeed());
  };

  const importSettings = (settings: PatchSelection) => {
    setCrownMod(settings.crownMod);
    setCrownDrops(settings.crownDrops);
    setFourPlayerSpawns(settings.fourPlayerSpawns);
    setRandomCrownColours(settings.randomCrownColours);
    setRandomEnemies(settings.randomEnemies);
    setRandomBosses(settings.randomEnemies && settings.randomBosses);
    setEnemySeed(settings.enemySeed === null ? "" : String(settings.enemySeed));
    setSoloButtons(settings.soloButtons);
  };

  const resetProgress = () => {
    furthestProgress.current = 0;
    setPatchProgress(0);
    setPatchStatus(undefined);
  };

  const runPatch = async (output: string, overwrite: boolean) => {
    outputRef.current = output;
    setUpdatingMod(false);
    setBusy(true);
    resetProgress();
    setPatchError(null);
    try {
      await backend.startPatch(output, overwrite, selection);
    } catch (error) {
      setBusy(false);
      setPatchError(String(error));
    }
  };

  const startPatch = async () => {
    if (!canPatch || disc.outputPath === null || busy) return;
    const output = withOutputExtension(disc.outputPath, disc.extension);
    disc.setOutputPath(output);
    resetProgress();
    setBusy(true);
    try {
      if (await backend.outputExists(output)) {
        setOverwritePath(output);
        setBusy(false);
      } else {
        await runPatch(output, false);
      }
    } catch (error) {
      setBusy(false);
      setPatchError(String(error));
    }
  };

  const updateMod = async () => {
    const status = discError;
    if (!canPatch || !status || !disc.inputPath || busy) return;
    setUpdatingMod(true);
    setBusy(true);
    resetProgress();
    setPatchError(null);
    try {
      outputRef.current = disc.inputPath;
      setDismissedDiscStatus(status);
      pendingModUpdate.current = { path: disc.inputPath, status, selection: { ...DEFAULT_PATCH_SELECTION, crownMod, fourPlayerSpawns, crownDrops, soloButtons } };
      await backend.updateMod(selection);
    } catch (error) {
      pendingModUpdate.current = null;
      setBusy(false);
      setPatchError(String(error));
    }
  };

  const chooseOutput = async () => {
    const defaultName = withOutputExtension(
      disc.outputPath?.split(/[/\\]/).pop() ?? "game [CrownMod]",
      disc.extension,
    );
    const path = await backend.chooseOutput(
      defaultName,
    );
    if (path !== null) disc.setOutputPath(withOutputExtension(path, disc.extension));
  };

  const setDiscPath = (path: string) => {
    // Typing a path by hand commits on Enter/blur via analysis; until then it
    // only fills the field. A typed path is treated like a dropped file once
    // it names an .iso.
    void disc.setInputPath(path);
  };

  const setOutput = (path: string) => {
    disc.setOutputPath(path);
  };

  return (
    <>
      <header className="app-header">
        <div className="brand">
          <img src="/crown.svg" alt="" width={28} height={28} />
          <h1>DW4 Crown Mod</h1>
        </div>
        <span className="platforms">PS2 · GameCube</span>
      </header>
      <div className="workspace">
        <div className="app-intro">
          <h2>Patch your disc</h2>
          <p className="app-subtitle">
            Choose how to play Digimon World 4 / World X on PS2 or GameCube. Select Crown Mod, random enemies, or combine your preferred options.
          </p>
        </div>
        <main className={`app-main ${wide ? "layout-two-col" : "layout-one-col"}`}>
          <div className="col">
            <DiscPicker
              key={disc.inputPath ?? ""}
              inputPath={disc.inputPath}
              info={info}
              modMetadata={selectedStatus?.modMetadata}
              onPick={() => void disc.pickDisc()}
              onCommitDiscPath={setDiscPath}
            />
            <SettingsPanel crownMod={crownMod} onCrownModChange={changeCrownMod} fourPlayerSpawns={fourPlayerSpawns} onFourPlayerSpawnsChange={setFourPlayerSpawns} crownDrops={crownDrops} onCrownDropsChange={setCrownDrops} randomCrownColours={randomCrownColours} onRandomCrownColoursChange={setRandomCrownColours} randomEnemies={randomEnemies} onRandomEnemiesChange={changeRandomEnemies} randomBosses={randomBosses} onRandomBossesChange={setRandomBosses} enemySeed={enemySeed} onEnemySeedChange={setEnemySeed} soloButtons={soloButtons} onSoloButtonsChange={setSoloButtons} busy={busy || updating || settingsBusy || disc.analyzing}
              fileActions={<SettingsFiles
                selection={{ ...selection, enemySeed: parseSeed(enemySeed) }}
                valid={!randomEnemies || enemySeedValue !== null}
                disabled={busy || updating || settingsBusy || disc.analyzing || overwritePath !== null || discError !== null}
                onImport={importSettings} onBusyChange={setSettingsBusy}
              />} />
          </div>
          <div className="col">
            <OutputPanel
              outputPath={disc.outputPath}
              busy={busy || updating || settingsBusy || disc.checking}
              canPatch={canPatch}
              status={selectedStatus}
              refreshing={disc.refreshing}
              onPatch={() => {
                if (disc.status?.reason === "already-modded") {
                  setDismissedDiscStatus(null);
                  setUpdateRequestedFor(disc.status);
                } else {
                  void startPatch();
                }
              }}
              onChooseOutput={() => void chooseOutput()}
              onSetOutput={setOutput}
            />
          </div>
        </main>
        <div className="workspace-footer footer-row">
          <SupportLink />
          <UpdateCenter blocked={busy || settingsBusy || disc.checking || overwritePath !== null || donePopup !== null || patchError !== null || discError !== null} onBusyChange={setUpdating} />
        </div>
      </div>
      {busy ? <PatchingDialog progress={patchProgress} status={patchStatus} updating={updatingMod} /> : disc.analyzing && <AnalyzingDialog />}
      {!busy && !disc.analyzing && discError !== null && (
        discError.reason === "already-modded" ? (
          <UpdateModDialog status={discError} path={disc.inputPath ?? ""} onCancel={() => setDismissedDiscStatus(discError)} onConfirm={() => void updateMod()} />
        ) : <MessageDialog title="Unable to open disc" message={discError.text} onClose={() => setDismissedDiscStatus(discError)} />
      )}
      {!busy && !disc.analyzing && patchError !== null && (
        <MessageDialog title="Unable to patch disc" message={patchError} onClose={() => setPatchError(null)} />
      )}
      {!busy && !disc.analyzing && donePopup !== null && (
        <DoneDialog
          path={donePopup}
          onClose={() => setDonePopup(null)}
        />
      )}
      {!busy && !disc.analyzing && overwritePath !== null && (
        <OverwriteDialog
          onCancel={() => setOverwritePath(null)}
          onConfirm={() => {
            const output = overwritePath;
            setOverwritePath(null);
            void runPatch(output, true);
          }}
        />
      )}
    </>
  );
}
