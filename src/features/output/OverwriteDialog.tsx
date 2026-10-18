import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";

export function OverwriteDialog({ onCancel, onConfirm }: {
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <ModalOverlay isOpen onOpenChange={(open) => { if (!open) onCancel(); }} isDismissable>
      <Modal>
        <Dialog>
          <Heading slot="title">File already exists</Heading>
          <p>Overwrite it?</p>
          <div className="row">
            <Button autoFocus onPress={onCancel}>Cancel</Button>
            <Button className="primary" onPress={onConfirm}>Overwrite</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
