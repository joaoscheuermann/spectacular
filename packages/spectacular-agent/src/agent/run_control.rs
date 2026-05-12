use spectacular_llms::Cancellation;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::Notify;

pub(super) const ACTIVE_RUN_CANCELLED_REASON: &str = "active run cancelled";
pub(super) const ACTIVE_RUN_HARD_ABORTED_REASON: &str = "active run hard-aborted";
pub(super) const HARD_ABORT_GRACE: Duration = Duration::from_millis(50);

/// Owns per-run cancellation, hard-abort signaling, and cancellation event de-duplication.
#[derive(Debug)]
pub(super) struct RunControl {
    cancellation: Cancellation,
    hard_abort_requested: AtomicBool,
    hard_abort_notify: Notify,
    cancellation_reason: Mutex<String>,
    cancelled_recorded: AtomicBool,
}

impl RunControl {
    /// Creates uncancelled control state for one agent run.
    pub(super) fn new() -> Self {
        Self {
            cancellation: Cancellation::default(),
            hard_abort_requested: AtomicBool::new(false),
            hard_abort_notify: Notify::new(),
            cancellation_reason: Mutex::new(ACTIVE_RUN_CANCELLED_REASON.to_owned()),
            cancelled_recorded: AtomicBool::new(false),
        }
    }

    /// Returns a clone of the cooperative cancellation token.
    pub(super) fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    /// Requests cooperative cancellation with the current cancellation reason.
    pub(super) fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Requests a bounded hard abort and notifies the supervisor.
    pub(super) fn request_hard_abort(&self) {
        *self.cancellation_reason.lock().unwrap() = ACTIVE_RUN_HARD_ABORTED_REASON.to_owned();
        self.cancel();
        self.hard_abort_requested.store(true, Ordering::SeqCst);
        self.hard_abort_notify.notify_waiters();
    }

    /// Waits until hard abort has been requested.
    pub(super) async fn hard_abort_requested(&self) {
        if self.hard_abort_requested.load(Ordering::SeqCst) {
            return;
        }

        let notified = self.hard_abort_notify.notified();
        if self.hard_abort_requested.load(Ordering::SeqCst) {
            return;
        }

        notified.await;
    }

    /// Returns the cancellation reason currently associated with this run.
    pub(super) fn cancellation_reason(&self) -> String {
        self.cancellation_reason.lock().unwrap().clone()
    }

    /// Overrides the cancellation reason when an upstream component supplies one.
    pub(super) fn set_cancellation_reason(&self, reason: impl Into<String>) {
        *self.cancellation_reason.lock().unwrap() = reason.into();
    }

    /// Returns true only once for this run's cancellation event.
    pub(super) fn try_record_cancelled(&self) -> bool {
        !self.cancelled_recorded.swap(true, Ordering::SeqCst)
    }
}
