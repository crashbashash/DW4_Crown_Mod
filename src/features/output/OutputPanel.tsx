import type { DiscStatus } from "../../bindings";

export interface OutputPanelProps {
  outputPath: string | null;
  busy: boolean;
  canPatch: boolean;
  status: DiscStatus | null;
  refreshing?: boolean;
  onPatch: () => void;
  onChooseOutput: () => void;
  onSetOutput: (path: string) => void;
}

/** Output selection and the patch action; progress lives in its own dialog. */
export function OutputPanel({
  outputPath,
  busy,
  canPatch,
  status,
  refreshing = false,
  onPatch,
  onChooseOutput,
  onSetOutput,
}: OutputPanelProps) {
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
      {refreshing ? <p className="hint" role="status">Updating output size…</p> : status?.kind === "ok" && (
        <p className="hint">
          The output copy needs {(status.outputSize / 1_000_000_000).toFixed(2)} GB of free space.
        </p>
      )}
      <div className="row patch-actions">
        <button className="primary" type="button" onClick={onPatch} disabled={!canPatch || busy}>
          Patch ISO
        </button>
      </div>
    </section>
  );
}
