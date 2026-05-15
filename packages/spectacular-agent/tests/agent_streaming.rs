mod support;

use spectacular_agent::{Agent, AgentError, AgentEvent};
use spectacular_llms::{
    provider_by_id, LlmProvider, Model, ProviderCall, ProviderCapabilities, ProviderError,
    ProviderMessage, ProviderMetadata, ProviderRequest, ProviderStream, ValidationMode,
    OPENROUTER_PROVIDER_ID,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::{capabilities, FakeProvider, PartialThenPendingEventProvider, SlowProvider};

#[tokio::test]
/// Verifies cancelling an active run records cancellation and cancels queued waiters.
async fn cancelling_active_run_keeps_partial_events_and_drops_waiters() {
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = SlowProvider {
        started: Arc::clone(&started),
    };
    let agent = Arc::new(Agent::new(provider));
    let active = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("active").await }
    });
    started.notified().await;

    let queued = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("queued").await }
    });
    tokio::task::yield_now().await;

    assert!(agent.cancel_active().await);
    assert!(matches!(
        active.await.unwrap(),
        Err(AgentError::CancellationError)
    ));
    assert!(matches!(
        queued.await.unwrap(),
        Err(AgentError::CancellationError)
    ));
    assert_cancelled_run_events(&agent.events(), "active run cancelled");
}

#[tokio::test]
/// Verifies cancelling after partial output closes the active message before cancellation.
async fn cancelling_after_partial_provider_output_finishes_active_message() {
    let partial_sent = Arc::new(tokio::sync::Notify::new());
    let provider = PartialThenPendingEventProvider {
        partial_sent: Arc::clone(&partial_sent),
    };
    let agent = Arc::new(Agent::new(provider));
    let active = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("active").await }
    });
    partial_sent.notified().await;

    assert!(agent.cancel_active().await);
    assert!(matches!(
        active.await.unwrap(),
        Err(AgentError::CancellationError)
    ));
    assert_finish_precedes_terminal_cancellation(&agent.events());
}

#[tokio::test]
/// Verifies hard-aborting an active run drops non-cooperative provider work.
async fn hard_aborting_active_run_drops_non_cooperative_provider_work() {
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = NonCooperativeProvider {
        started: Arc::clone(&started),
    };
    let agent = Arc::new(Agent::new(provider));
    let active = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("active").await }
    });
    started.notified().await;

    let start = Instant::now();
    assert!(agent.hard_abort_active().await);
    let result = tokio::time::timeout(Duration::from_millis(500), active)
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(result, Err(AgentError::CancellationError)));
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_cancelled_run_events(&agent.events(), "active run hard-aborted");
}

#[tokio::test]
/// Verifies run streams emit events in the same order as the backing store.
async fn streaming_run_emits_events_in_store_order() {
    let agent = Arc::new(Agent::new(FakeProvider::text("hello")));
    let mut stream = Arc::clone(&agent).run_stream("prompt");
    let mut events = Vec::new();

    while let Some(event) = stream.next().await {
        let terminal = matches!(
            event,
            AgentEvent::Finished { .. } | AgentEvent::Error { .. } | AgentEvent::Cancelled { .. }
        );
        events.push(event);
        if terminal {
            break;
        }
    }

    assert_eq!(events, agent.events());
}

#[tokio::test]
/// Verifies dropping an active stream hard-aborts the active run and pending queue.
async fn dropping_stream_cancels_active_run_and_pending_queue() {
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = SlowProvider {
        started: Arc::clone(&started),
    };
    let agent = Arc::new(Agent::new(provider));
    let stream = Arc::clone(&agent).run_stream("active");
    started.notified().await;

    let queued = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("queued").await }
    });
    tokio::task::yield_now().await;

    drop(stream);

    for _ in 0..20 {
        if matches!(agent.events().last(), Some(AgentEvent::Cancelled { .. })) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    assert!(matches!(
        queued.await.unwrap(),
        Err(AgentError::CancellationError)
    ));
    assert_cancelled_run_events(&agent.events(), "active run hard-aborted");
}

#[tokio::test]
/// Verifies hard-aborting a stream returns promptly when providers ignore cancellation.
async fn hard_aborting_stream_drops_non_cooperative_provider_work() {
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = NonCooperativeProvider {
        started: Arc::clone(&started),
    };
    let agent = Arc::new(Agent::new(provider));
    let stream = Arc::clone(&agent).run_stream("active");
    started.notified().await;

    let start = Instant::now();
    stream.hard_abort();
    drop(stream);

    for _ in 0..100 {
        if matches!(agent.events().last(), Some(AgentEvent::Cancelled { .. })) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    assert!(start.elapsed() < Duration::from_millis(500));
    assert_cancelled_run_events(&agent.events(), "active run hard-aborted");
}

#[tokio::test]
/// Verifies dropping a queued stream propagates hard abort to the active run.
async fn dropping_queued_stream_cancels_current_active_run() {
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = SlowProvider {
        started: Arc::clone(&started),
    };
    let agent = Arc::new(Agent::new(provider));
    let active = tokio::spawn({
        let agent = Arc::clone(&agent);
        async move { agent.run("active").await }
    });
    started.notified().await;

    let queued_stream = Arc::clone(&agent).run_stream("queued");
    tokio::task::yield_now().await;

    drop(queued_stream);

    assert!(matches!(
        active.await.unwrap(),
        Err(AgentError::CancellationError)
    ));
    assert_cancelled_run_events(&agent.events(), "active run hard-aborted");
}

#[tokio::test]
/// Verifies dropping a completed stream does not poison subsequent runs.
async fn dropping_completed_stream_does_not_reject_next_run() {
    let agent = Arc::new(Agent::new(FakeProvider::text("hello")));
    let mut stream = Arc::clone(&agent).run_stream("first");

    while let Some(event) = stream.next().await {
        if matches!(
            event,
            AgentEvent::Finished { .. } | AgentEvent::Error { .. } | AgentEvent::Cancelled { .. }
        ) {
            break;
        }
    }

    drop(stream);

    agent.run("second").await.unwrap();
    assert!(agent.events().iter().any(|event| {
        matches!(event, AgentEvent::UserPrompt { content } if content == "second")
    }));
}

/// Asserts a message finish boundary was recorded before the terminal cancellation event.
fn assert_finish_precedes_terminal_cancellation(events: &[AgentEvent]) {
    let finish_index = events
        .iter()
        .position(|event| matches!(event, AgentEvent::MessageFinish { .. }))
        .unwrap();
    let cancellation_index = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Cancelled { .. }))
        .unwrap();

    assert!(finish_index < cancellation_index);
}

/// Asserts cancellation tests keep the expected runtime-only context usage event.
fn assert_cancelled_run_events(events: &[AgentEvent], expected_reason: &str) {
    assert!(matches!(
        events,
        [
            AgentEvent::UserPrompt { content },
            AgentEvent::ContextTokenUsage(usage),
            AgentEvent::Cancelled { reason },
        ] if content == "active"
            && usage.input_tokens > 0
            && usage.context_window_tokens.is_none()
            && reason == expected_reason
    ));
}

#[derive(Clone, Debug)]
struct NonCooperativeProvider {
    started: Arc<tokio::sync::Notify>,
}

impl LlmProvider for NonCooperativeProvider {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata {
        provider_by_id(OPENROUTER_PROVIDER_ID).unwrap()
    }

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, _mode: ValidationMode, _value: &str) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, _api_key: &str) -> Result<Vec<Model>, ProviderError> {
        Ok(Vec::new())
    }

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        capabilities()
    }

    /// Starts a streaming completion request and ignores cooperative cancellation.
    fn stream_completion<'a>(
        &'a self,
        _request: ProviderRequest,
        _cancellation: spectacular_agent::Cancellation,
    ) -> ProviderCall<'a> {
        let started = Arc::clone(&self.started);
        Box::pin(async move {
            started.notify_one();
            std::future::pending::<Result<ProviderStream, ProviderError>>().await
        })
    }
}

#[test]
/// Verifies raw provider requests default to streaming with optional flags disabled.
fn request_defaults_keep_flags_off_except_streaming() {
    let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

    assert!(request.flags.stream);
    assert!(!request.flags.allow_tools);
    assert!(!request.flags.include_reasoning);
    assert_eq!(request.flags.reasoning_effort, None);
}
