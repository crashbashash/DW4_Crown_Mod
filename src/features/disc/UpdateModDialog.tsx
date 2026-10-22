import { Button, Dialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import type { DiscStatus } from "../../bindings";
import packageInfo from "../../../package.json";

export function UpdateModDialog({ status, path, onCancel, onConfirm }: {
  status: DiscStatus;
  path: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <ModalOverlay isOpen isDismissable onOpenChange={open => { if (!open) onCancel(); }}>
      <Modal>
        <Dialog>
          <Heading slot="title">Update modded game?</Heading>
          <p>{status.modMetadata ? `This image contains Crown Mod v${status.modMetadata.version}.` : "This image is already modded; its version and options are unknown."} Update it to v{packageInfo.version} using the selected settings?</p>
          {status.modMetadata?.options.randomEnemies === true && (
            <p>Its enemies are randomised with seed {status.modMetadata.randomEnemies?.seed ?? status.modMetadata.options.enemySeed ?? "unknown"}.</p>
          )}
          <p>The verified update will replace this image in place: {path}</p>
          {!status.modMetadata?.recovery && <p>This older build needs your original disc image for its first update. You will be asked to select it next.</p>}
          <div className="row">
            <Button autoFocus onPress={onCancel}>Cancel</Button>
            <Button className="primary" onPress={onConfirm}>Update image</Button>
          </div>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
