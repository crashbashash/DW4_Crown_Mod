import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import type { UpdateCheck, UpdateOffer } from "../../bindings/UpdateCheck";
import { MessageDialog } from "../../components/MessageDialog";
import { useBackend } from "../../ipc/context";

/** Check quietly at startup; defer the offer until other operations/dialogs finish. */
export function UpdateCenter({ blocked, onBusyChange }: {
  blocked: boolean;
  onBusyChange: (busy: boolean) => void;
}) {
  const backend = useBackend();
  const startup = useRef<Promise<UpdateCheck> | null>(null);
  const [checking, setChecking] = useState(true);
  const [offer, setOffer] = useState<UpdateOffer | null>(null);
  const [notice, setNotice] = useState<{ title: string; message: string } | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    startup.current ??= backend.checkUpdates();
    void startup.current.then((result) => {
      if (!cancelled) setOffer(result.update);
    }).catch(() => {
      // Being offline at launch must not interrupt disc patching.
    }).finally(() => { if (!cancelled) setChecking(false); });
    return () => { cancelled = true; };
  }, [backend]);

  const check = async () => {
    setChecking(true);
    setUpdateError(null);
    try {
      const result = await backend.checkUpdates();
      setOffer(result.update);
      if (result.update === null) setNotice({ title: "You're up to date", message: `You're using DW4 Crown Mod ${result.currentVersion}.` });
    } catch {
      setNotice({ title: "Unable to check for updates", message: "Check your internet connection and try again later." });
    } finally {
      setChecking(false);
    }
  };

  const accept = async () => {
    if (offer === null || updating) return;
    setUpdating(true);
    onBusyChange(true);
    setUpdateError(null);
    let installerStarted = false;
    try {
      const result = await backend.acceptUpdate(offer.version);
      installerStarted = result === "installer";
      if (!installerStarted) setOffer(null);
    } catch (error) {
      setUpdateError(String(error));
    } finally {
      if (!installerStarted) {
        setUpdating(false);
        onBusyChange(false);
      }
    }
  };

  const visible = !blocked || updating;
  return (
    <>
      <button className="update-check" type="button" onClick={() => void check()} disabled={checking || blocked || updating}>
        {checking ? "Checking for updates…" : "Check for updates"}
      </button>
      {visible && notice !== null && (
        <MessageDialog title={notice.title} message={notice.message} onClose={() => setNotice(null)} />
      )}
      {visible && offer !== null && (
        <ModalOverlay isOpen isDismissable={!updating} onOpenChange={(open) => { if (!open && !updating) { setOffer(null); setUpdateError(null); } }}>
          <Modal className="react-aria-Modal update-modal">
            <Dialog>
              <Heading slot="title">Update available</Heading>
              <p>DW4 Crown Mod {offer.version} is available. You're using {offer.currentVersion}.</p>
              <div className="update-notes" role="region" aria-label="What's new" tabIndex={0}>
                {offer.releases.map((release) => (
                  <section key={release.version}>
                    <h3>Version {release.version}</h3>
                    <div className="release-notes">{release.notes}</div>
                  </section>
                ))}
              </div>
              <p className="hint">
                {offer.method === "msi"
                  ? "Download the update and open Windows Installer? The app will close when the installer starts."
                  : "Open the GitHub release page to download the update?"}
              </p>
              {updateError !== null && <p className="status-refused" role="alert">{updateError}</p>}
              {updating && <p role="status">{offer.method === "msi" ? "Downloading and verifying the update…" : "Opening the release page…"}</p>}
              <div className="row">
                <Button autoFocus isDisabled={updating} onPress={() => { setOffer(null); setUpdateError(null); }}>Later</Button>
                <Button className="primary" isDisabled={updating} onPress={() => void accept()}>
                  {offer.method === "msi" ? "Download and update" : "Open GitHub release"}
                </Button>
              </div>
            </Dialog>
          </Modal>
        </ModalOverlay>
      )}
    </>
  );
}
