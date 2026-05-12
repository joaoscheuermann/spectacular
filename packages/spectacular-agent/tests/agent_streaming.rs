mod support;

use spectacular_agent::{Agent, AgentError, AgentEvent};
use spectacular_llms::{ProviderMessage, ProviderRequest};
use std::sync::Arc;
use support::{FakeProvider, SlowProvider};

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
    assert_eq!(
        agent.events(),
        vec![
            AgentEvent::user_prompt("active"),
            AgentEvent::cancelled("active run cancelled")
        ]
    );
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
/// Verifies dropping an active stream cancels the active run and pending queue.
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
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    assert!(matches!(
        queued.await.unwrap(),
        Err(AgentError::CancellationError)
    ));
    assert_eq!(
        agent.events(),
        vec![
            AgentEvent::user_prompt("active"),
            AgentEvent::cancelled("active run cancelled")
        ]
    );
}

#[tokio::test]
/// Verifies dropping a queued stream propagates cancellation to the active run.
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
    assert_eq!(
        agent.events(),
        vec![
            AgentEvent::user_prompt("active"),
            AgentEvent::cancelled("active run cancelled")
        ]
    );
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

#[test]
/// Verifies raw provider requests default to streaming with optional flags disabled.
fn request_defaults_keep_flags_off_except_streaming() {
    let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

    assert!(request.flags.stream);
    assert!(!request.flags.allow_tools);
    assert!(!request.flags.include_reasoning);
    assert_eq!(request.flags.reasoning_effort, None);
}
