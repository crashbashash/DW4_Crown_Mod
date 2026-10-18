import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";

/** A dismissible notice for a disc-selection or patching error. */
export function MessageDialog({ title, message, onClose }: {
  title: string;
  message: string;
  onClose: () => void;
}) {
  return (
    <ModalOverlay isOpen onOpenChange={(open) => { if (!open) onClose(); }} isDismissable>
      <Modal>
        <Dialog>
          <Heading slot="title">{title}</Heading>
          <p>{message}</p>
          <div className="row">
            <Button className="primary" autoFocus onPress={onClose}>OK</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
