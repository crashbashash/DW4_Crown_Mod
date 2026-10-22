import { useCallback, useEffect, useRef, useState } from "react";
import type { DiscStatus } from "../bindings";
import { UpdateModDialog } from "../features/disc/UpdateModDialog";
import { AnalyzingDialog } from "../features/disc/AnalyzingDialog";
import { DiscPicker } from "../features/disc/DiscPicker";
import { useDisc } from "../features/disc/useDisc";
import { DoneDialog } from "../features/output/DoneDialog";
import { OverwriteDialog } from "../features/output/OverwriteDialog";
import { PatchingDialog } from "../features/output/PatchingDialog";
import { OutputPanel } from "../features/output/OutputPanel";
import { SettingsPanel } from "../features/settings/SettingsPanel";
import { newSeed, parseSeed } from "../features/settings/seed";
import { isSuggestedName, withSeed } from "../features/output/outputName";
import { UpdateCenter } from "../features/updates/UpdateCenter";
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

/** Disc selection and output for the base Crown Mod mode. */
export function App() {
  return (
    <BackendProvider>
      <AppInner />
    </BackendProvider>
  );
}

function AppInner() {
  const backend = useBackend();
  const [fourPlayerSpawns, setFourPlayerSpawns] = useState(false);
  const [crownDrops, setCrownDrops] = useState(false);
  const [randomCrownColours, setRandomCrownColours] = useState(false);
  const [randomEnemies, setRandomEnemies] = useState(false);
  const [enemySeed, setEnemySeed] = useState("");
  const appliedOptionsEpoch = useRef<number | null>(null);
  const applyDetectedOptions = useCallback((status: DiscStatus, epoch: number) => {
    if (appliedOptionsEpoch.current === epoch) return;
    appliedOptionsEpoch.current = epoch;
    if (status.reason === "already-modded") {
      const options = status.modMetadata?.options;
      setFourPlayerSpawns(options?.fourPlayerSpawns ?? false);
      setCrownDrops(options?.betterLoot ?? false);
      setRandomCrownColours(options?.randomCrownColours ?? false);
      const seed = status.modMetadata?.randomEnemies?.seed ?? options?.enemySeed;
      setRandomEnemies(options?.randomEnemies ?? false);
      if (options?.randomEnemies === true && typeof seed === "number") setEnemySeed(String(seed));
    }
  }, []);
  const disc = useDisc(backend, fourPlayerSpawns, crownDrops, applyDetectedOptions);
  const replaceDiscStatus = disc.replaceStatus;
  const wide = useMediaQuery("(min-width: 900px)");

  const [updating, setUpdating] = useState(false);
  const [dismissedDiscStatus, setDismissedDiscStatus] = useState<DiscStatus | null>(null);
  const [updateRequestedFor, setUpdateRequestedFor] = useState<DiscStatus | null>(null);
  const selectedStatus = disc.status ?? disc.identifiedStatus;
  const discError = disc.status !== null && disc.status.kind !== "ok" && disc.status !== dismissedDiscStatus
    && (disc.status.reason !== "already-modded" || updateRequestedFor === disc.status) ? disc.status : null;
  const [overwritePath, setOverwritePath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [patchProgress, setPatchProgress] = useState(0);
  const [patchError, setPatchError] = useState<string | null>(null);
  const [donePopup, setDonePopup] = useState<string | null>(null);
  const outputRef = useRef<string | null>(null);
  const pendingModUpdate = useRef<{ path: string; status: DiscStatus; spawns: boolean; drops: boolean } | null>(null);

  // Patch events: progress streams, one terminal event ends the run.
  useEffect(() => {
    const ups: Promise<() => void>[] = [];
    ups.push(
      backend.onPatchProgress(({ done, total }) => {
        setBusy(true);
        if (total > 0 && Number.isFinite(done) && Number.isFinite(total)) {
          // Only the verified terminal event completes the operation.
          const percentage = Math.min(99, Math.max(0, Math.floor(done / total * 100)));
          setPatchProgress(previous => Math.max(previous, percentage));
        }
      }),
    );
    ups.push(
      backend.onPatchDone(() => {
        setPatchProgress(100);
        setDonePopup(outputRef.current ?? "");
        setBusy(false);
        const updated = pendingModUpdate.current;
        pendingModUpdate.current = null;
        if (updated) {
          void backend.analyze(updated.path, updated.spawns, updated.drops).catch((error: unknown): DiscStatus => ({
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
  }, [backend, replaceDiscStatus]);

  // An ISO dropped onto the window is a chosen disc.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void backend.onFileDropped((path) => {
      if (!busy && !updating && !disc.analyzing) void disc.setInputPath(path);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [backend, disc, busy, updating]);

  const info = selectedStatus?.discInfo;
  const enemySeedValue = randomEnemies ? parseSeed(enemySeed) : null;
  const analyzed = disc.status?.kind === "ok";
  const canPatch = analyzed && disc.outputPath !== null && (!randomEnemies || enemySeedValue !== null);

  // A suggested output name carries the enemy seed; a path the user chose is left alone.
  const { outputPath, setOutputPath } = disc;
  useEffect(() => {
    if (outputPath === null || !isSuggestedName(outputPath)) return;
    const wanted = withSeed(outputPath, enemySeedValue);
    if (wanted !== outputPath) setOutputPath(wanted);
  }, [outputPath, setOutputPath, enemySeedValue]);

  const changeRandomEnemies = (selected: boolean) => {
    setRandomEnemies(selected);
    if (selected && parseSeed(enemySeed) === null) setEnemySeed(newSeed());
  };

  const runPatch = async (output: string, overwrite: boolean) => {
    outputRef.current = output;
    setBusy(true);
    setPatchProgress(0);
    setPatchError(null);
    try {
      await backend.startPatch(output, overwrite, fourPlayerSpawns, crownDrops, randomCrownColours, randomEnemies, enemySeedValue);
    } catch (error) {
      setBusy(false);
      setPatchError(String(error));
    }
  };

  const startPatch = async () => {
    const output = disc.outputPath;
    if (output === null || busy) return;
    setPatchProgress(0);
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

  const updateMod = async (spawns: boolean, drops: boolean, colours: boolean, enemies: boolean, seed: number | null) => {
    const status = discError;
    if (!status || !disc.inputPath || busy) return;
    setBusy(true);
    setPatchProgress(0);
    setPatchError(null);
    try {
      const original = status.modMetadata?.recovery ? null : await backend.chooseIso();
      if (!status.modMetadata?.recovery && original === null) {
        setBusy(false);
        return;
      }
      outputRef.current = disc.inputPath;
      setDismissedDiscStatus(status);
      pendingModUpdate.current = { path: disc.inputPath, status, spawns, drops };
      await backend.updateMod(spawns, drops, colours, enemies, seed, original);
    } catch (error) {
      pendingModUpdate.current = null;
      setBusy(false);
      setPatchError(String(error));
    }
  };

  const chooseOutput = async () => {
    const path = await backend.chooseOutput(
      disc.outputPath?.split("/").pop() ?? "game [CrownMod].iso",
    );
    if (path !== null) disc.setOutputPath(path);
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
            Turn your own clean PS2 or GameCube Digimon World 4 / World X disc into a much harder one, with crown icons for every enemy.
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
            <SettingsPanel fourPlayerSpawns={fourPlayerSpawns} onFourPlayerSpawnsChange={setFourPlayerSpawns} crownDrops={crownDrops} onCrownDropsChange={setCrownDrops} randomCrownColours={randomCrownColours} onRandomCrownColoursChange={setRandomCrownColours} randomEnemies={randomEnemies} onRandomEnemiesChange={changeRandomEnemies} enemySeed={enemySeed} onEnemySeedChange={setEnemySeed} busy={busy || updating || disc.analyzing} />
          </div>
          <div className="col">
            <OutputPanel
              outputPath={disc.outputPath}
              busy={busy || updating || disc.checking}
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
          <UpdateCenter blocked={busy || disc.checking || overwritePath !== null || donePopup !== null || patchError !== null || discError !== null} onBusyChange={setUpdating} />
        </div>
      </div>
      {busy ? <PatchingDialog progress={patchProgress} /> : disc.analyzing && <AnalyzingDialog />}
      {!busy && !disc.analyzing && discError !== null && (
        discError.reason === "already-modded" ? (
          <UpdateModDialog status={discError} path={disc.inputPath ?? ""} onCancel={() => setDismissedDiscStatus(discError)} onConfirm={() => void updateMod(fourPlayerSpawns, crownDrops, randomCrownColours, randomEnemies, enemySeedValue)} />
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
