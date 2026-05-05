use super::{MessageDelta, ProviderError, ProviderFinished, ReasoningDelta};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Cooperative cancellation input for provider calls.
#[derive(Clone, Debug, Default)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
}

impl Cancellation {
    pub fn cancelled() -> Self {
        let cancellation = Self::default();
        cancellation.cancel();
        cancellation
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl PartialEq for Cancellation {
    fn eq(&self, other: &Self) -> bool {
        self.is_cancelled() == other.is_cancelled()
    }
}

impl Eq for Cancellation {}

/// Stream event emitted by async providers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderStreamEvent {
    MessageDelta(MessageDelta),
    ReasoningDelta(ReasoningDelta),
    Finished(ProviderFinished),
}

pub struct ProviderStream {
    receiver: mpsc::Receiver<Result<ProviderStreamEvent, ProviderError>>,
}

impl ProviderStream {
    pub fn new(receiver: mpsc::Receiver<Result<ProviderStreamEvent, ProviderError>>) -> Self {
        Self { receiver }
    }

    pub fn from_events(
        events: impl IntoIterator<Item = Result<ProviderStreamEvent, ProviderError>>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(128);
        for event in events {
            if sender.try_send(event).is_err() {
                break;
            }
        }
        drop(sender);
        Self { receiver }
    }

    pub async fn next(&mut self) -> Option<Result<ProviderStreamEvent, ProviderError>> {
        self.receiver.recv().await
    }
}

pub type ProviderCall<'a> =
    Pin<Box<dyn Future<Output = Result<ProviderStream, ProviderError>> + Send + 'a>>;
