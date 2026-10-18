import type { DiscStatus, PatchOutcome, ProgressEvent } from "../../bindings";

export interface OutputPanelProps {
  outputPath: string | null;
  busy: boolean;
  canPatch: boolean;
  progress: ProgressEvent | null;
  log: string[];
  status: DiscStatus | null;
  lastOutcome: PatchOutcome | null;
  onPatch: () => void;
  onChooseOutput: () => void;
  onSetOutput: (path: string) => void;
}

/** Output path, the one Patch button, progress and the log. */
export function OutputPanel({
  outputPath,
  busy,
  canPatch,
  progress,
  log,
  status,
  lastOutcome,
  onPatch,
  onChooseOutput,
  onSetOutput,
}: OutputPanelProps) {
  const fraction =
    progress === null || progress.total === 0
      ? 0
      : Math.min(1, Math.max(0, progress.done / progress.total));

  return (
    <section className="panel">
      <h2>Output</h2>
      <div className="row">
        <input
          className="path-field"
          type="text"
          value={outputPath ?? ""}
          placeholder="where to write the patched disc"
          aria-label="output path"
          onChange={(e) => onSetOutput(e.target.value)}
        />
        <button type="button" onClick={onChooseOutput}>
          Save as…
        </button>
      </div>
      {status !== null && (
        <p className="hint">
          The output copy needs {(status.outputSize / 1_000_000_000).toFixed(2)} GB of free space.
        </p>
      )}
      {lastOutcome !== null && (
        <p className="note-ok">
          Patched {(lastOutcome.bytes / 1_000_000_000).toFixed(2)} GB, md5 {lastOutcome.md5}
        </p>
      )}
      <div className="row patch-actions">
        <button className="primary" type="button" onClick={onPatch} disabled={!canPatch || busy}>
          Patch ISO
        </button>
        {progress !== null && (
          <span className="progress-line">
            <span>{progress.phase}</span>
            <progress value={fraction} />
            <span>
              {Math.round(fraction * 100)}%
            </span>
          </span>
        )}
        {busy && progress === null && <span className="hint">working…</span>}
      </div>
      <div className="log-panel" aria-label="log">
        {log.length === 0 && <span className="log-empty">Patching progress will appear here.</span>}
        {log.map((line, i) => (
          /* The log is append-only; the index is stable per line. */
          <div key={i} className="log-line">
            {line}
          </div>
        ))}
      </div>
    </section>
  );
}
