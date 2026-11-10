import { Dialog, Heading, Modal, ModalOverlay, ProgressBar } from "react-aria-components";

/** Analysis uses an activity animation; patching supplies overall progress. */
export function ActivityDialog({ title, message, value, detail }: {
  title: string;
  message: string;
  value?: number;
  /** The current step. Not a live region: it changes every few seconds. */
  detail?: string;
}) {
  return (
    <ModalOverlay isOpen isDismissable={false} isKeyboardDismissDisabled>
      <Modal>
        <Dialog aria-describedby="activity-description">
          <Heading slot="title">{title}</Heading>
          <p id="activity-description" role="status">{message}</p>
          <ProgressBar isIndeterminate={value === undefined} value={value} aria-label={title} valueLabel={value !== undefined && detail ? `${Math.floor(value)}%, ${detail}` : undefined} className="activity-progress">
            {value !== undefined && (
              <div className="activity-progress-label">
                <span className="activity-progress-detail">{detail}</span>
                <span className="activity-progress-value">{Math.floor(value)}%</span>
              </div>
            )}
            <div className="activity-progress-track">
              <span className={value === undefined ? "activity-progress-indicator" : "activity-progress-fill"} style={value === undefined ? undefined : { width: `${value}%` }} />
            </div>
          </ProgressBar>
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
