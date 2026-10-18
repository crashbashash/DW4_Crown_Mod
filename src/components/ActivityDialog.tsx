import { Dialog, Heading, Modal, ModalOverlay, ProgressBar } from "react-aria-components";

/** Analysis uses an activity animation; patching supplies overall progress. */
export function ActivityDialog({ title, message, value }: {
  title: string;
  message: string;
  value?: number;
}) {
  return (
    <ModalOverlay isOpen isDismissable={false} isKeyboardDismissDisabled>
      <Modal>
        <Dialog aria-describedby="activity-description">
          <Heading slot="title">{title}</Heading>
          <p id="activity-description" role="status">{message}</p>
          <ProgressBar isIndeterminate={value === undefined} value={value} aria-label={title} className="activity-progress">
            {value !== undefined && <div className="activity-progress-value">{Math.floor(value)}%</div>}
            <div className="activity-progress-track">
              <span className={value === undefined ? "activity-progress-indicator" : "activity-progress-fill"} style={value === undefined ? undefined : { width: `${value}%` }} />
            </div>
          </ProgressBar>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
