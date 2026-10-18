import { useState } from "react";

export interface DiscPickerProps {
  inputPath: string | null;
  outputPath: string | null;
  status: { kind: "ok" | "refused" | "unknown"; text: string } | null;
  analyzing: boolean;
  onPick: () => void;
  /** A typed path, committed on Enter or blur (not per keystroke). */
  onCommitDiscPath: (path: string) => void;
}

/** The disc row: input path, Browse, drag-drop hint, verdict line. */
export function DiscPicker({
  inputPath,
  outputPath,
  status,
  analyzing,
  onPick,
  onCommitDiscPath,
}: DiscPickerProps) {
  // The parent keys this component on `inputPath`, so picking or dropping a
  // new disc remounts it with the fresh text — no state-sync effect needed.
  const [text, setText] = useState(inputPath ?? "");

  const commit = () => {
    const trimmed = text.trim();
    if (trimmed.length > 0 && trimmed !== inputPath) {
      onCommitDiscPath(trimmed);
    }
  };

  return (
    <section className="panel">
      <h2>Disc</h2>
      <div className="row">
        <input
          className="path-field"
          type="text"
          value={text}
          placeholder="path to the ISO"
          aria-label="disc path"
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
          }}
          onBlur={commit}
        />
        <button type="button" onClick={onPick}>
          Browse…
        </button>
      </div>
      <p className="hint">{inputPath === null ? "Choose a disc with Browse, or drag and drop it onto this window." : "You can also drag and drop a disc onto this window."}</p>
      {analyzing && <p className="status-line status-unknown">analyzing…</p>}
      {status !== null && <p className={`status-line status-${status.kind}`}>{status.text}</p>}
      {outputPath !== null && <p className="hint">output: {outputPath}</p>}
    </section>
  );
}
