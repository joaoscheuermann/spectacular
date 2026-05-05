mod support;

use serde_json::{json, Value};
use spectacular_agent::{
    Agent, AgentConfig, AgentError, AgentEvent, Cancellation, OutputSchema, Tool, ToolExecution,
    ToolManifest,
};
use spectacular_llms::{
    FinishReason, MessageDelta, ProviderContextLimits, ProviderError, ProviderFinished,
    ProviderMessage, ProviderMessageRole, ProviderRequest, ProviderStreamEvent, ProviderToolCall,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use support::{
    capabilities, finished_length_without_usage, finished_stop_with_usage, finished_with_reason,
    provider_unavailable, recovered_events, FailingProvider, FakeProvider, ProviderAttempt,
    RecordingProvider, SlowProvider, StreamErrorProvider,
};

const DEFAULT_MAX_PROVIDER_RETRIES: usize = 2;
const LENGTH_CONTINUATION_PROMPT: &str = "Continue from exactly where the previous assistant response stopped. Do not repeat any earlier text, and do not explain that you are continuing.";
const PROVIDER_CANCELLED_MESSAGE: &str = "provider cancelled the response";

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

#[derive(Clone, Debug)]
struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            self.name(),
            "Echo parsed arguments as provider-visible JSON.",
            json!({"type": "object", "additionalProperties": true}),
        )
    }

    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async move { Ok(arguments.to_string()) })
    }
}

#[derive(Clone, Debug)]
struct BuiltInStyleWriteTool;

impl Tool for BuiltInStyleWriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            self.name(),
            "Writes UTF-8 text to a file in the workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        )
    }

    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async move {
            Ok(json!({
                "success": true,
                "path": arguments.get("path").and_then(Value::as_str).unwrap_or_default()
            })
            .to_string())
        })
    }
}

#[test]
fn tool_call_loop_stores_tool_result_then_finishes() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![
            ProviderStreamEvent::Finished(ProviderFinished::tool_calls(vec![
                ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#),
            ])),
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    };
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    assert!(agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::ToolResult { .. })));
}

#[test]
fn provider_request_includes_tool_manifest_and_tool_summary_system_prompt() {
    let provider = RecordingProvider::new(vec![vec![
        ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
        ProviderStreamEvent::Finished(finished_stop_with_usage()),
    ]]);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let requests = requests.lock().unwrap();
    let request = requests.first().unwrap();
    assert_eq!(request.tools.len(), 1);
    assert_eq!(request.tools[0].name, "echo");
    assert_eq!(
        request.tools[0].description,
        "Echo parsed arguments as provider-visible JSON."
    );
    assert_eq!(
            request.messages[0].content,
            "You have access to the following tools:\n* echo - Echo parsed arguments as provider-visible JSON."
        );
    assert!(!request.messages[0].content.contains("Parameters:"));
    assert!(!request.messages[0].content.contains("additionalProperties"));
}

#[test]
fn tool_call_loop_emits_structured_tool_events_with_matching_id() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let tool_call = agent.events().into_iter().find_map(|event| match event {
        AgentEvent::AssistantToolCallRequest {
            tool_call_id,
            name,
            arguments,
        } => Some((tool_call_id, name, arguments)),
        _ => None,
    });
    let tool_result = agent.events().into_iter().find_map(|event| match event {
        AgentEvent::ToolResult {
            tool_call_id,
            name,
            content,
        } => Some((tool_call_id, name, content)),
        _ => None,
    });

    assert_eq!(
        tool_call,
        Some((
            "call-1".to_owned(),
            "echo".to_owned(),
            r#"{"ok":true}"#.to_owned()
        ))
    );
    assert_eq!(
        tool_result,
        Some((
            "call-1".to_owned(),
            "echo".to_owned(),
            r#"{"ok":true}"#.to_owned()
        ))
    );
}

#[test]
fn follow_up_provider_request_replays_assistant_tool_call_and_tool_result() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "echo", r#"{"ok":true}"#)],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let initial = &requests[0];
    assert_eq!(initial.tools.len(), 1);
    assert_eq!(initial.tools[0].name, "echo");
    let follow_up = &requests[1];
    assert_eq!(follow_up.tools.len(), 1);
    assert_eq!(follow_up.tools[0].name, "echo");

    let assistant_tool_call = follow_up
        .messages
        .iter()
        .find(|message| !message.tool_calls.is_empty())
        .unwrap();
    let tool_result = follow_up
        .messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("call-1"))
        .unwrap();

    assert_eq!(assistant_tool_call.tool_calls[0].id, "call-1");
    assert_eq!(assistant_tool_call.tool_calls[0].name, "echo");
    assert_eq!(
        assistant_tool_call.tool_calls[0].arguments,
        r#"{"ok":true}"#
    );
    assert_eq!(tool_result.content, r#"{"ok":true}"#);
    assert_eq!(tool_result.tool_call_id.as_deref(), Some("call-1"));
}

#[test]
fn fake_provider_receives_built_in_style_tool_result_with_matching_id() {
    let provider = RecordingProvider::new(vec![
        vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new(
                "call-write-1",
                "write",
                r#"{"path":"foo.txt","content":"hello"}"#,
            )],
        ))],
        vec![
            ProviderStreamEvent::MessageDelta(MessageDelta::assistant("done")),
            ProviderStreamEvent::Finished(finished_stop_with_usage()),
        ],
    ]);
    let requests = Arc::clone(&provider.requests);
    let mut agent = Agent::new(provider);
    agent.register_tool(BuiltInStyleWriteTool).unwrap();
    agent.enqueue_prompt("prompt");

    futures::executor::block_on(agent.run_next()).unwrap();

    let tool_result_event = agent.events().into_iter().find_map(|event| match event {
        AgentEvent::ToolResult {
            tool_call_id,
            name,
            content,
        } => Some((tool_call_id, name, content)),
        _ => None,
    });
    let requests = requests.lock().unwrap();
    let follow_up_tool_result = requests[1]
        .messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("call-write-1"))
        .unwrap();

    assert_eq!(
        tool_result_event,
        Some((
            "call-write-1".to_owned(),
            "write".to_owned(),
            r#"{"path":"foo.txt","success":true}"#.to_owned()
        ))
    );
    assert_eq!(
        follow_up_tool_result.tool_call_id.as_deref(),
        Some("call-write-1")
    );
    assert_eq!(
        follow_up_tool_result.content,
        r#"{"path":"foo.txt","success":true}"#
    );
}

#[test]
fn malformed_tool_call_finish_is_rejected_and_stored() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(ProviderFinished::tool_calls(
            vec![ProviderToolCall::new("call-1", "", "{}")],
        ))],
    };
    let mut agent = Agent::new(provider);
    agent.register_tool(EchoTool).unwrap();
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::MalformedProviderResponse { .. }
    ));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
fn missing_usage_metadata_on_final_response_is_rejected_and_stored() {
    let provider = FakeProvider {
        calls: Arc::new(AtomicUsize::new(0)),
        capabilities: capabilities(),
        events: vec![ProviderStreamEvent::Finished(ProviderFinished::stopped())],
    };
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(
        error,
        AgentError::MalformedProviderResponse { .. }
    ));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[test]
fn context_limit_failure_is_stored_before_provider_io() {
    let mut provider = FakeProvider::text("unused");
    provider.capabilities.context_limits = ProviderContextLimits {
        max_messages: Some(1),
        max_chars: None,
    };
    let calls = Arc::clone(&provider.calls);
    let mut agent = Agent::new(provider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ContextLimitError { .. }));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
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
        AgentEvent::MessageDelta(MessageDelta { content, .. }) if content == "recovered"
    )));
    assert!(!agent
        .events()
        .iter()
        .any(|event| matches!(event, AgentEvent::Error { .. })));
}

#[test]
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
        AgentEvent::MessageDelta(MessageDelta { content, .. }) if content == "partial"
    )));
}

#[test]
fn stream_provider_errors_keep_partial_events_then_store_error() {
    let mut agent = Agent::new(StreamErrorProvider);
    agent.enqueue_prompt("prompt");

    let error = futures::executor::block_on(agent.run_next()).unwrap_err();

    assert!(matches!(error, AgentError::ProviderParsingError { .. }));
    assert!(agent.events().iter().any(|event| matches!(
        event,
        AgentEvent::MessageDelta(MessageDelta { content, .. }) if content == "partial"
    )));
    assert!(matches!(
        agent.events().last(),
        Some(AgentEvent::Error { .. })
    ));
}

#[tokio::test]
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
fn request_defaults_keep_flags_off_except_streaming() {
    let request = ProviderRequest::new(vec![ProviderMessage::user("hello")]);

    assert!(request.flags.stream);
    assert!(!request.flags.allow_tools);
    assert!(!request.flags.include_reasoning);
    assert_eq!(request.flags.reasoning_effort, None);
}
