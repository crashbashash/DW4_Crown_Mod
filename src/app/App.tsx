import { useEffect, useRef, useState } from "react";
import type { PatchOutcome, ProgressEvent } from "../bindings";
import { DiscPicker } from "../features/disc/DiscPicker";
import { useDisc } from "../features/disc/useDisc";
import { DoneDialog } from "../features/output/DoneDialog";
import { OverwriteDialog } from "../features/output/OverwriteDialog";
import { OutputPanel } from "../features/output/OutputPanel";
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

/** Disc selection and output for the base Very Hard Plus mode. */
export function App() {
  return (
    <BackendProvider>
      <AppInner />
    </BackendProvider>
  );
}

function AppInner() {
  const backend = useBackend();
  const disc = useDisc(backend);
  const wide = useMediaQuery("(min-width: 900px)");

  const [overwritePath, setOverwritePath] = useState<string | null>(null);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  const [lastOutcome, setLastOutcome] = useState<PatchOutcome | null>(null);
  const [donePopup, setDonePopup] = useState<PatchOutcome | null>(null);
  const outputRef = useRef<string | null>(null);

  // Patch events: progress streams, one terminal event ends the run.
  useEffect(() => {
    const ups: Promise<() => void>[] = [];
    ups.push(
      backend.onPatchProgress((p) => {
        setProgress(p);
        setBusy(true);
      }),
    );
    ups.push(
      backend.onPatchDone((o) => {
        setLastOutcome(o);
        setDonePopup(o);
        setProgress(null);
        setBusy(false);
        setLog((lines) => [
          ...lines,
          `Patched ${(o.bytes / 1_000_000_000).toFixed(2)} GB, md5 ${o.md5}`,
        ]);
      }),
    );
    ups.push(
      backend.onPatchFailed((message) => {
        setProgress(null);
        setBusy(false);
        setLog((lines) => [...lines, `patch refused: ${message}`]);
      }),
    );
    return () => {
      for (const up of ups) void up.then((unlisten) => unlisten());
    };
  }, [backend]);

  // An ISO dropped onto the window is a chosen disc.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void backend.onFileDropped((path) => void disc.setInputPath(path)).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [backend, disc]);

  const analyzed = disc.status?.kind === "ok";
  const canPatch = analyzed && disc.outputPath !== null;

  const runPatch = async (output: string, overwrite: boolean) => {
    outputRef.current = output;
    setBusy(true);
    setLog((lines) => [...lines, `patching ${disc.inputPath} -> ${output}`]);
    try {
      await backend.startPatch(output, overwrite);
    } catch (error) {
      setBusy(false);
      setLog((lines) => [...lines, `patch refused: ${String(error)}`]);
    }
  };

  const startPatch = async () => {
    const output = disc.outputPath;
    if (output === null || busy) return;
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
      setLog((lines) => [...lines, `patch refused: ${String(error)}`]);
    }
  };

  const chooseOutput = async () => {
    const path = await backend.chooseOutput(
      disc.outputPath?.split("/").pop() ?? "game [VeryHardPlus].iso",
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
        <h1>DW4 Very Hard Plus</h1>
        <p className="app-subtitle">
          Turn your own clean PS2 or GameCube Digimon World 4 / World X disc into a much harder one, with crown icons for every enemy.
        </p>
      </header>
      <main className={`app-main ${wide ? "layout-two-col" : "layout-one-col"}`}>
        <div className="col">
          <DiscPicker
            key={disc.inputPath ?? ""}
            inputPath={disc.inputPath}
            outputPath={disc.outputPath}
            status={disc.status}
            analyzing={disc.analyzing}
            onPick={() => void disc.pickDisc()}
            onCommitDiscPath={setDiscPath}
          />
        </div>
        <div className="col">
          <OutputPanel
            outputPath={disc.outputPath}
            busy={busy || disc.analyzing}
            canPatch={canPatch}
            progress={progress}
            log={log}
            status={disc.status}
            lastOutcome={lastOutcome}
            onPatch={() => void startPatch()}
            onChooseOutput={() => void chooseOutput()}
            onSetOutput={setOutput}
          />
        </div>
      </main>
      {donePopup !== null && (
        <DoneDialog
          path={outputRef.current ?? ""}
          outcome={donePopup}
          onClose={() => setDonePopup(null)}
        />
      )}
      {overwritePath !== null && (
        <OverwriteDialog
          path={overwritePath}
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
