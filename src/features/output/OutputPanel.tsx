import type { DiscStatus } from "../../bindings";

export interface OutputPanelProps {
  outputPath: string | null;
  busy: boolean;
  canPatch: boolean;
  status: DiscStatus | null;
  refreshing?: boolean;
  /** Bytes an in-place update of a modded image will produce, once estimated. */
  updateSize?: number | null;
  /** The image's container, so a compressed update says its size is before compression. */
  extension?: string;
  onPatch: () => void;
  onChooseOutput: () => void;
  onSetOutput: (path: string) => void;
}

/** Binary gigabytes, as Windows and most file managers report sizes, under the familiar "GB" label. */
const gigabytes = (bytes: number) => (bytes / 1024 ** 3).toFixed(2);
const compressed = (extension: string) => extension === "rvz" || extension === "ciso";

/** Output selection and the patch action; progress lives in its own dialog. */
export function OutputPanel({
  outputPath,
  busy,
  canPatch,
  status,
  refreshing = false,
  updateSize = null,
  extension = "iso",
  onPatch,
  onChooseOutput,
  onSetOutput,
}: OutputPanelProps) {
  const modded = status?.reason === "already-modded";
  return (
    <section className="panel">
      <h2>{modded ? "Update" : "Output"}</h2>
      {modded ? <p className="hint">Your selected image will be updated in place.</p> : <div className="row">
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
      </div>}
      {refreshing ? <p className="hint" role="status">Updating output size…</p> : modded ? updateSize !== null && (
        <p className="hint">
          The updated image will be {gigabytes(updateSize)} GB{compressed(extension) ? " before compression" : ""}.
        </p>
      ) : status?.kind === "ok" && (
        <p className="hint">
          The output copy needs {gigabytes(status.outputSize)} GB of free space.
        </p>
      )}
      <div className="row patch-actions">
        <button className="primary" type="button" onClick={onPatch} disabled={!canPatch || busy}>
          {modded ? "Update ISO" : "Patch ISO"}
        </button>
      </div>
    </section>
  );
}
