use super::run_control::RunControl;
use crate::event::AgentEvent;
use crate::queue::RunQueue;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;

/// Streaming handle for a background agent run.
pub struct AgentRunStream {
    receiver: mpsc::Receiver<AgentEvent>,
    control: Arc<RunControl>,
    queue: Arc<RunQueue>,
    completed: Arc<AtomicBool>,
    active_run_control: Arc<Mutex<Option<Arc<RunControl>>>>,
}

impl AgentRunStream {
    /// Creates a stream handle around a background run channel and cancellation state.
    pub(super) fn new(
        receiver: mpsc::Receiver<AgentEvent>,
        control: Arc<RunControl>,
        queue: Arc<RunQueue>,
        completed: Arc<AtomicBool>,
        active_run_control: Arc<Mutex<Option<Arc<RunControl>>>>,
    ) -> Self {
        Self {
            receiver,
            control,
            queue,
            completed,
            active_run_control,
        }
    }

    /// Receives the next persisted run event and marks terminal streams completed.
    pub async fn next(&mut self) -> Option<AgentEvent> {
        let event = self.receiver.recv().await;
        if is_terminal_event(&event) {
            self.completed.store(true, Ordering::SeqCst);
        }

        event
    }

    /// Cancels the active run and any queued waiters unless the stream already completed.
    pub fn cancel(&self) {
        if self.completed.load(Ordering::SeqCst) {
            return;
        }

        self.control.cancel();
        if let Some(active_control) = self.active_run_control.lock().unwrap().as_ref() {
            active_control.cancel();
        }
        self.queue.cancel_pending_now();
    }

    /// Hard-aborts the active run and any queued waiters unless the stream already completed.
    pub fn hard_abort(&self) {
        if self.completed.load(Ordering::SeqCst) {
            return;
        }

        self.control.request_hard_abort();
        if let Some(active_control) = self.active_run_control.lock().unwrap().as_ref() {
            active_control.request_hard_abort();
        }
        self.queue.cancel_pending_now();
    }
}

impl Drop for AgentRunStream {
    /// Hard-aborts unfinished background work when the stream handle is dropped.
    fn drop(&mut self) {
        self.hard_abort();
    }
}

/// Returns true when an optional event represents a terminal run state.
fn is_terminal_event(event: &Option<AgentEvent>) -> bool {
    matches!(
        event,
        Some(AgentEvent::Finished { .. } | AgentEvent::Error { .. } | AgentEvent::Cancelled { .. })
    )
}
