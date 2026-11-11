import { useState } from "react";
import { useBackend } from "../ipc/context";
import { MessageDialog } from "./MessageDialog";

/** A quiet footer link to the project's Ko-fi page, opened in the browser. */
export function SupportLink() {
  const backend = useBackend();
  const [error, setError] = useState<string | null>(null);
  const open = async () => {
    try {
      await backend.openSupportPage();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <>
      <button className="support-link" type="button" onClick={() => void open()}>
        <span aria-hidden="true">♥</span> Support on Ko-fi
      </button>
      {error !== null && <MessageDialog title="Unable to open Ko-fi" message={error} onClose={() => setError(null)} />}
    </>
  );
}
