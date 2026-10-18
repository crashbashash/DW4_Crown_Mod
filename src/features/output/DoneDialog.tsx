import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";

/** The popup shown when a patch has been written and verified. */
export function DoneDialog({ path, onClose }: {
  path: string;
  onClose: () => void;
}) {
  return (
    <ModalOverlay isOpen onOpenChange={(open) => { if (!open) onClose(); }} isDismissable>
      <Modal>
        <Dialog>
          <Heading slot="title">Ready to play</Heading>
          <p>Your Crown Mod game has been saved to:</p>
          <p className="overwrite-path">{path}</p>
          <p className="hint">Open this file in your emulator to play.</p>
          <div className="row">
            <Button className="primary" autoFocus onPress={onClose}>OK</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
