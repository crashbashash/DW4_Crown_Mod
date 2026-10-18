import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";

export function OverwriteDialog({ path, onCancel, onConfirm }: {
  path: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <ModalOverlay isOpen onOpenChange={(open) => { if (!open) onCancel(); }} isDismissable>
      <Modal>
        <Dialog>
          <Heading slot="title">Overwrite existing file?</Heading>
          <p>A file already exists at:</p>
          <p className="overwrite-path">{path}</p>
          <p>Replace it with the patched disc?</p>
          <div className="row">
            <Button autoFocus onPress={onCancel}>Cancel</Button>
            <Button className="primary" onPress={onConfirm}>Overwrite and patch</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
