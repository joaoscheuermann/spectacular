use super::Agent;
use crate::context::TokenCounter;
use crate::error::AgentError;
use crate::event::AgentEvent;
use spectacular_llms::{Cancellation, LlmProvider};
use tokio::sync::mpsc;

/// Records run events into storage and optional live stream output.
pub(super) struct RunRecorder<'a, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    agent: &'a Agent<P, C>,
    cancellation: Cancellation,
    sender: Option<mpsc::Sender<AgentEvent>>,
    cancelled_recorded: bool,
}

impl<'a, P, C> RunRecorder<'a, P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    /// Creates a recorder for one run using shared agent state and optional event sender.
    pub(super) fn new(
        agent: &'a Agent<P, C>,
        cancellation: Cancellation,
        sender: Option<mpsc::Sender<AgentEvent>>,
    ) -> Self {
        Self {
            agent,
            cancellation,
            sender,
            cancelled_recorded: false,
        }
    }

    /// Returns a clone of the run cancellation token for provider or tool calls.
    pub(super) fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    /// Signals cancellation through the run token.
    pub(super) fn cancel(&self) {
        self.cancellation.cancel();
    }

    /// Appends an event to the store and sends it to live stream listeners when present.
    pub(super) async fn record(&mut self, event: AgentEvent) -> Result<(), AgentError> {
        {
            self.agent.store.lock().unwrap().append(event.clone());
        }

        let Some(sender) = self.sender.as_ref() else {
            return Ok(());
        };

        if sender.send(event).await.is_ok() {
            return Ok(());
        }

        self.cancellation.cancel();
        self.record_cancelled().await;
        Err(AgentError::CancellationError)
    }

    /// Records an error event unless cancellation has already won the run outcome.
    pub(super) async fn record_error<T>(&mut self, error: AgentError) -> Result<T, AgentError> {
        if self.cancellation.is_cancelled() {
            self.record_cancelled().await;
            return Err(AgentError::CancellationError);
        }

        self.record(AgentEvent::error(error.to_string())).await?;
        Err(error)
    }

    /// Returns a cancellation error after recording a cancellation event when needed.
    pub(super) async fn return_if_cancelled(&mut self) -> Result<(), AgentError> {
        if !self.cancellation.is_cancelled() {
            return Ok(());
        }

        self.record_cancelled().await;
        Err(AgentError::CancellationError)
    }

    /// Records the default active-run cancellation event.
    pub(super) async fn record_cancelled(&mut self) {
        self.record_cancelled_with_reason("active run cancelled")
            .await;
    }

    /// Records a single cancellation event with a caller-provided reason.
    pub(super) async fn record_cancelled_with_reason(&mut self, reason: impl Into<String>) {
        self.agent.queue.cancel_pending().await;
        if self.cancelled_recorded {
            return;
        }
        self.cancelled_recorded = true;

        let event = AgentEvent::cancelled(reason);
        {
            self.agent.store.lock().unwrap().append(event.clone());
        }

        if let Some(sender) = self.sender.as_ref() {
            let _ = sender.send(event).await;
        }
    }

    /// Returns the number of events currently persisted for the run owner.
    pub(super) fn event_count(&self) -> usize {
        self.agent.store.lock().unwrap().events().len()
    }
}
