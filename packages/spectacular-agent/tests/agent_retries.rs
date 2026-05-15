mod support;

use spectacular_agent::{Agent, AgentConfig, AgentError, AgentEvent};
use spectacular_llms::{FinishReason, MessageDelta, ProviderError, ProviderStreamEvent};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use support::{
    provider_unavailable, recovered_events, FailingProvider, ProviderAttempt, RecordingProvider,
    StreamErrorProvider,
};

const DEFAULT_MAX_PROVIDER_RETRIES: usize = 2;

#[test]
/// Verifies exhausted retryable provider errors are stored as run errors.
fn provider_errors_are_stored() {
    let calls = Arc::new(AtomicUsize::new(0));
    let provider = FailingProvider {
        calls: Arc::clone(&calls),
    };
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        DEFAULT_MAX_PROVIDER_RETRIES + 1
    );
    assert!(matches!(error, AgentError::ProviderNetworkError { .. }));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
/// Verifies retryable call-opening errors are retried before any stream events.
fn retryable_provider_error_before_stream_is_retried() {
    let provider = RecordingProvider::with_attempts(vec![
        ProviderAttempt::Error(provider_unavailable()),
        ProviderAttempt::Error(provider_unavailable()),
        ProviderAttempt::Events(recovered_events()),
    ]);
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::with_config(
        provider,
        AgentConfig {
            max_provider_retries: 2,
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(agent.events().iter().any(|event| matches!(
        event,
        AgentEvent::MessageDelta { content, .. } if content == "recovered"
    )));
    assert!(!agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::Error { .. })));
}

#[test]
/// Verifies retryable stream errors are retried before any provider events escape.
fn retryable_stream_error_before_events_is_retried() {
    let provider = RecordingProvider::with_attempts(vec![
        ProviderAttempt::Events(vec![Err(provider_unavailable())]),
        ProviderAttempt::Events(vec![Err(provider_unavailable())]),
        ProviderAttempt::Events(recovered_events()),
    ]);
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::with_config(
        provider,
        AgentConfig {
            max_provider_retries: 2,
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Finished {
            finish_reason: FinishReason::Stop
        })
    ));
}

#[test]
/// Verifies retryable stream errors are not retried after partial output is stored.
fn retryable_stream_error_after_events_is_not_retried() {
    let provider = RecordingProvider::with_attempts(vec![ProviderAttempt::Events(vec![
        Ok(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
            "partial",
        ))),
        Err(provider_unavailable()),
    ])]);
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        error,
        AgentError::Provider(ProviderError::ProviderUnavailable { .. })
    ));
    assert!(agent.events().iter().any(|event| matches!(
        event,
        AgentEvent::MessageDelta { content, .. } if content == "partial"
    )));
}

#[test]
/// Verifies non-retryable stream errors preserve partial output and store an error.
fn stream_provider_errors_keep_partial_events_then_store_error() {
    let mut agent = Agent::new(StreamErrorProvider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ProviderParsingError { .. }));
    assert!(agent.events().iter().any(|event| matches!(
        event,
        AgentEvent::MessageDelta { content, .. } if content == "partial"
    )));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}
