mod support;

use serde_json::json;
use spectacular_agent::{Agent, AgentConfig, AgentError, AgentEvent, OutputSchema, TokenCounter};
use spectacular_llms::{
    FinishReason, MessageDelta, ProviderMessage, ProviderMessageRole, ProviderStreamEvent,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use support::{
    capabilities, finished_length_without_usage, finished_stop_with_usage, finished_with_reason,
    EchoTool, FakeProvider, RecordingProvider,
};

const LENGTH_CONTINUATION_PROMPT: &str = "Continue from exactly where the previous assistant response stopped. Do not repeat any earlier text, and do not explain that you are continuing.";
const PROVIDER_CANCELLED_MESSAGE: &str = "provider cancelled the response";

#[derive(Clone, Debug)]
struct FixedTokenCounter;

impl TokenCounter for FixedTokenCounter {
    /// Counts every text fragment as one token for predictable compaction tests.
    fn count_text_tokens(&self, _text: &str) -> usize {
        1
    }

    /// Counts every provider message as one token for predictable compaction tests.
    fn count_message_tokens(&self, _message: &ProviderMessage) -> usize {
        1
    }
}

/// Coalesces assistant deltas from stored events into final response text.
fn final_assistant_response(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageDelta(delta) if delta.role == ProviderMessageRole::Assistant => {
                Some(delta.content.as_str())
            }
            _ => None,
        })
        .collect::<String>()
}

#[test]
/// Verifies a simple provider run stores prompt, delta, usage, and finish events in order.
fn no_tool_run_stores_events_in_order() {
    let mut agent = Agent::new(FakeProvider::text("hello"));
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert!(matches!(agent.events()[0], AgentEvent::UserPrompt { .. }));
    assert!(matches!(agent.events()[1], AgentEvent::MessageDelta(_)));
    assert!(matches!(agent.events()[2], AgentEvent::UsageMetadata(_)));
    assert!(matches!(agent.events()[3], AgentEvent::Finished { .. }));
}

#[test]
/// Verifies length finishes trigger continuation requests without storing interim finish events.
fn length_finish_silently_continues_until_stop() {
    let provider = RecordingProvider::new(vec![
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("first ")),
            ProviderStreamEvent::Finished(finished_length_without_usage()),
        ],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("second")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let calls = Arc::clone(&provider.calls);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(final_assistant_response(&agent.events()), "first second");
    assert!(!agent.events().iter().any(|event| matches!(
        event,
        AgentEvent::Finished {
            finish_reason: FinishReason::Length
        }
    )));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Finished {
            finish_reason: FinishReason::Stop
        })
    ));

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let continuation = requests[1].messages.last().unwrap();
    assert_eq!(continuation.role, ProviderMessageRole::User);
    assert_eq!(continuation.content, LENGTH_CONTINUATION_PROMPT);
    assert!(requests[1].messages.iter().any(|message| {
        message.role == ProviderMessageRole::Assistant && message.content == "first "
    }));
}

#[test]
/// Verifies content-filter finishes become stored safety guardrail errors.
fn content_filter_finish_records_safety_guardrail_error() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(finished_with_reason(
            FinishReason::ContentFilter,
        ))],
    };
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ContentFiltered));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { message }) if message.contains("safety guardrails")
    ));
    assert!(!agent.events().iter().any(|event| matches!(
        event,
        AgentEvent::Finished {
            finish_reason: FinishReason::ContentFilter
        }
    )));
}

#[test]
/// Verifies provider error finish reasons are surfaced as provider finish errors.
fn error_finish_records_provider_finish_error() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(finished_with_reason(
            FinishReason::Error,
        ))],
    };
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ProviderFinishError { .. }));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { message }) if message.contains("finish_reason=error")
    ));
}

#[test]
/// Verifies provider-cancelled finishes record a cancellation event.
fn cancelled_finish_records_provider_cancelled_event() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(finished_with_reason(
            FinishReason::Cancelled,
        ))],
    };
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::CancellationError));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Cancelled { reason }) if reason == PROVIDER_CANCELLED_MESSAGE
    ));
}

#[test]
/// Verifies structured-output requirements are checked before provider I/O.
fn structured_output_capability_mismatch_happens_before_provider_io() {
    let mut provider = FakeProvider::text(r#"{"status":"ready"}"#);
    provider.capabilities.structured_output = false;
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::with_config(
        provider,
        AgentConfig {
            output_schema: Some(OutputSchema::new(json!({"type":"object"})).unwrap()),
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::CapabilityMismatch {
            capability: "structured_output"
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
/// Verifies usage metadata requirements are checked before provider I/O.
fn usage_metadata_capability_mismatch_happens_before_provider_io() {
    let mut provider = FakeProvider::text("unused");
    provider.capabilities.usage_metadata = false;
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::CapabilityMismatch {
            capability: "usage_metadata"
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
/// Verifies reasoning requests fail fast when the provider lacks reasoning support.
fn reasoning_request_capability_mismatch_happens_before_provider_io() {
    let mut provider = FakeProvider::text("unused");
    provider.capabilities.reasoning = false;
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::with_config(
        provider,
        AgentConfig {
            include_reasoning: true,
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::CapabilityMismatch {
            capability: "reasoning"
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
/// Verifies reasoning metadata requirements fail fast when unsupported.
fn reasoning_metadata_requirement_capability_mismatch_happens_before_provider_io() {
    let mut provider = FakeProvider::text("unused");
    provider.capabilities.reasoning_metadata = false;
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::with_config(
        provider,
        AgentConfig {
            require_reasoning_metadata: true,
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::CapabilityMismatch {
            capability: "reasoning_metadata"
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
/// Verifies agents accept an explicitly injected token counter dependency.
fn custom_token_counter_can_be_injected_at_agent_boundary() {
    let agent = Agent::with_token_counter(FakeProvider::text("hello"), FixedTokenCounter);

    futures::executor::block_on(agent.run("prompt")).unwrap();

    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Finished {
            finish_reason: FinishReason::Stop
        })
    ));
}

#[test]
/// Verifies registered tools require provider tool-call capability before I/O.
fn registered_tool_requires_provider_tool_capability_before_provider_io() {
    let mut provider = FakeProvider::text("unused");
    provider.capabilities.tool_calls = false;
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::CapabilityMismatch {
            capability: "tool_calls"
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
/// Verifies invalid final assistant content is rejected by the output schema.
fn structured_output_validation_rejects_invalid_response() {
    let mut agent = Agent::with_config(
        FakeProvider::text(r#"{"status":"draft"}"#),
        AgentConfig {
            output_schema: Some(
                OutputSchema::new(json!({
                    "type": "object",
                    "properties": {"status": {"const": "ready"}},
                    "required": ["status"]
                }))
                .unwrap(),
            ),
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ValidationError { .. }));
    assert!(agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::ValidationError { .. })));
}

#[test]
/// Verifies valid final assistant content passes output schema validation.
fn structured_output_validation_allows_valid_response() {
    let mut agent = Agent::with_config(
        FakeProvider::text(r#"{"status":"ready"}"#),
        AgentConfig {
            output_schema: Some(
                OutputSchema::new(json!({
                    "type": "object",
                    "properties": {"status": {"const": "ready"}},
                    "required": ["status"]
                }))
                .unwrap(),
            ),
            ..AgentConfig::default()
        },
    );
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Finished {
            finish_reason: FinishReason::Stop
        })
    ));
}
