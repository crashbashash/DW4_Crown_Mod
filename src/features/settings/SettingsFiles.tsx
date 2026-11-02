import { useRef, useState } from "react";
import { Button } from "react-aria-components";
import type { PatchSelection } from "../../bindings/PatchSelection";
import { useBackend } from "../../ipc/context";

export function SettingsFiles({ selection, disabled, valid, onImport, onBusyChange }: {
  selection: PatchSelection;
  disabled: boolean;
  valid: boolean;
  onImport: (settings: PatchSelection) => void;
  onBusyChange: (busy: boolean) => void;
}) {
  const backend = useBackend();
  const running = useRef(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const transfer = async (kind: "import" | "export") => {
    if (running.current || disabled) return;
    running.current = true;
    onBusyChange(true);
    setNotice("");
    setError("");
    try {
      if (kind === "import") {
        const settings = await backend.importSettings();
        if (settings !== null) {
          onImport(settings);
          setNotice("Settings imported.");
        }
      } else if (await backend.exportSettings(selection)) {
        setNotice("Settings exported.");
      }
    } catch (error) {
      setError(`Unable to ${kind} settings. ${String(error)}`);
    } finally {
      running.current = false;
      onBusyChange(false);
    }
  };
  return (
    <div className="settings-files">
      <div className="row">
        <Button isDisabled={disabled} onPress={() => void transfer("import")}>Import settings…</Button>
        <Button isDisabled={disabled || !valid} onPress={() => void transfer("export")}>Export settings…</Button>
      </div>
      <p className="hint">Share your options and seed. Disc and output paths are never included.</p>
      {notice && <p className="hint" role="status">{notice}</p>}
      {error && <p className="field-error" role="alert">{error}</p>}
    </div>
  );
}
