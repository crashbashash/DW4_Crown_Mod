import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import type { PatchOutcome } from "../../bindings";

/** The popup shown when a patch has been written and verified. */
export function DoneDialog({ path, outcome, onClose }: {
  path: string;
  outcome: PatchOutcome;
  onClose: () => void;
}) {
  return (
    <ModalOverlay isOpen onOpenChange={(open) => { if (!open) onClose(); }} isDismissable>
      <Modal>
        <Dialog>
          <Heading slot="title">Patching complete</Heading>
          <p>Your Very Hard Plus disc was written and verified:</p>
          <p className="overwrite-path">{path}</p>
          <p className="hint">
            {(outcome.bytes / 1_000_000_000).toFixed(2)} GB, {outcome.summary.rowsChanged} enemy
            rows changed, md5 {outcome.md5}
          </p>
          <div className="row">
            <Button autoFocus onPress={onClose}>OK</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
