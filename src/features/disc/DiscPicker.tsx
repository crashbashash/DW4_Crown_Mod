import { useState } from "react";
import type { DiscStatus } from "../../bindings";

export interface DiscPickerProps {
  inputPath: string | null;
  info?: DiscStatus["discInfo"];
  onPick: () => void;
  /** A typed path, committed on Enter or blur (not per keystroke). */
  onCommitDiscPath: (path: string) => void;
}

/** Disc selection; the app owns analysis notices and other dialogs. */
export function DiscPicker({
  inputPath,
  info,
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
      {info && <dl className="disc-info" aria-label="Disc information">
        <div><dt>Console</dt><dd>{info.console}</dd></div>
        <div><dt>Region</dt><dd>{info.region}</dd></div>
        <div><dt>Mod</dt><dd>{info.modded ? info.modVersion ? `Crown Mod v${info.modVersion}` : "Modded · version unknown" : "Unmodified"}</dd></div>
      </dl>}
      <p className="hint">{inputPath === null ? "Choose a disc with Browse, or drag and drop it onto this window." : "You can also drag and drop a disc onto this window."}</p>
    </section>
  );
}
