use dw4vhp_core::disc::DiscReport;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

/// What the shell holds between commands: the analysed disc (from the last
/// successful [`analyze`]), and the input path it was read from. The patch
/// worker reads both; the frontend never sees the enemy table.
#[derive(Default)]
pub struct AppState {
    pub analysis_epoch: AtomicU64,
    operation: Arc<AtomicBool>,
    pub input: Mutex<Option<PathBuf>>,
    pub report: Mutex<Option<DiscReport>>,
}

/// Reserves patching/installation before a worker is queued; release on every exit.
pub struct OperationPermit(Arc<AtomicBool>);
impl Drop for OperationPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
impl AppState {
    pub fn begin_operation(&self) -> Result<OperationPermit, String> {
        self.operation
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| {
                "Finish the current patch or update before starting another.".to_string()
            })?;
        Ok(OperationPermit(self.operation.clone()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patching_and_installation_are_mutually_exclusive() {
        let state = AppState::default();
        let permit = state.begin_operation().unwrap();
        assert!(state.begin_operation().is_err());
        drop(permit);
        assert!(state.begin_operation().is_ok());
    }
}
