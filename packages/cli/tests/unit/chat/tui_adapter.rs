use super::*;
use crate::chat::RuntimeSelection;
use ::agent::{AgentErrorDetails, AgentErrorKind, AgentErrorStage, AgentEvent};
use ::commands::{Command, CommandControl, CommandRegistry};
use ::config::ProviderAuthMode;
use ::llms::{FinishReason, UsageMetadata};
use ::tui::{
    ChatTuiAction, CommandDescriptor, CommandValueValidation as TuiCommandValueValidation,
    CompletionValues, ContextTokenUsage as TuiContextTokenUsage, DisplayLine, DisplayLineStyle,
    DisplayMetadata as TuiDisplayMetadata, DisplaySpan,
    ProviderUsageMetadata as TuiProviderUsageMetadata, ReasoningLevel as TuiReasoningLevel,
    RuntimeSelection as TuiRuntimeSelection, SessionId, ToolDisplayStatus, TranscriptItemId,
};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "tui/display.rs"]
mod display;

/// Verifies that an agent start event maps to the TUI running boundary.
#[test]
fn agent_start_event_maps_to_agent_started() {
    assert_eq!(agent_started_action(), ChatTuiAction::AgentStarted);
}

/// Verifies assistant lifecycle IDs from agent events are passed directly to TUI actions.
#[test]
fn assistant_lifecycle_events_use_agent_provided_transcript_id() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::message_start("agent-message-7")),
        vec![ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("agent-message-7"),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::message_delta("agent-message-7", "hello")),
        vec![ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("agent-message-7"),
            text: "hello".to_owned(),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::message_finish("agent-message-7")),
        vec![ChatTuiAction::MessageFinished {
            id: TranscriptItemId::new("agent-message-7"),
        }]
    );
}

/// Verifies reasoning lifecycle IDs from agent events are passed directly to TUI actions.
#[test]
fn reasoning_lifecycle_events_use_agent_provided_transcript_id() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::reasoning_start("agent-reasoning-3")),
        vec![ChatTuiAction::ReasoningStarted {
            id: TranscriptItemId::new("agent-reasoning-3"),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::reasoning_delta("agent-reasoning-3", "thinking")),
        vec![ChatTuiAction::ReasoningDelta {
            id: TranscriptItemId::new("agent-reasoning-3"),
            text: "thinking".to_owned(),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::reasoning_finish("agent-reasoning-3")),
        vec![ChatTuiAction::ReasoningFinished {
            id: TranscriptItemId::new("agent-reasoning-3"),
        }]
    );
}

/// Verifies terminal events no longer synthesize assistant or reasoning finish actions.
#[test]
fn terminal_events_do_not_synthesize_lifecycle_finishes() {
    let mut adapter = TuiEventAdapter::new();

    let _ = adapter.adapt_agent_event(&AgentEvent::message_start("agent-message-7"));
    let _ = adapter.adapt_agent_event(&AgentEvent::reasoning_start("agent-reasoning-3"));

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        }),
        vec![ChatTuiAction::AgentFinished]
    );
}

/// Verifies explicit tool lifecycle events map directly into TUI tool actions.
#[test]
fn tool_lifecycle_events_map_directly_to_tui_actions() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::tool_call_start("call-1", "read", "{}")),
        vec![
            ChatTuiAction::ToolCallStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "read".to_owned(),
                arguments: "{}".to_owned(),
            },
            ChatTuiAction::ToolDisplayStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "read".to_owned(),
                call_line: DisplayLine::from_spans(vec![DisplaySpan::new(
                    "read",
                    DisplayLineStyle::Tool,
                )]),
                argument_lines: Vec::new(),
            },
        ]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::tool_call_delta("call-1", "chunk")),
        vec![ChatTuiAction::ToolCallDelta {
            tool_call_id: "call-1".to_owned(),
            text: "chunk".to_owned(),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::tool_call_finish("call-1", "read", "output")),
        vec![
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "call-1".to_owned(),
                name: "read".to_owned(),
                output: "output".to_owned(),
            },
            ChatTuiAction::ToolDisplayFinished {
                tool_call_id: "call-1".to_owned(),
                status: ToolDisplayStatus::Succeeded,
                output_lines: vec![DisplayLine::new("output", DisplayLineStyle::CommandOutput)],
            },
        ]
    );
}

/// Verifies command registry metadata maps into prompt command descriptors.
#[test]
fn command_registry_loading_maps_to_commands_loaded() {
    let registry = CommandRegistry::<()>::new()
        .with(Command {
            name: "test",
            usage: "/test",
            summary: "Run test command",
            execute: noop_command,
        })
        .unwrap();

    assert_eq!(
        commands_loaded_action(&registry),
        ChatTuiAction::CommandsLoaded(vec![CommandDescriptor::with_usage(
            "test",
            "Run test command",
            "/test"
        )])
    );
}

/// Verifies structured completion metadata is projected into TUI command descriptors.
#[test]
fn command_registry_loading_maps_structured_completions_to_tui_descriptors() {
    let adapter = crate::chat::commands::registry().unwrap();
    let model = tui_test_model();

    let action = commands_loaded_with_completions_action(&adapter, &model);

    let ChatTuiAction::CommandsLoaded(commands) = action else {
        panic!("expected command load action");
    };
    let provider = commands
        .iter()
        .find(|command| command.name == "provider")
        .expect("provider command should be projected");
    assert_eq!(
        provider
            .subcommands
            .iter()
            .map(|subcommand| subcommand.name.as_str())
            .collect::<Vec<_>>(),
        vec!["add", "remove", "auth"]
    );
    let provider_add = provider
        .subcommands
        .iter()
        .find(|subcommand| subcommand.name == "add")
        .expect("provider add should be projected");
    let provider_field = &provider_add.fields[0];
    assert_eq!(provider_field.name, "provider");
    assert_eq!(
        provider_field.validation,
        TuiCommandValueValidation::OneOfValues
    );
    let CompletionValues::Static(provider_types) = &provider_field.values else {
        panic!("provider add should use static provider types");
    };
    assert!(provider_types.iter().any(|value| value == "openrouter"));
    assert!(provider_types.iter().any(|value| value == "openai"));

    let model = commands
        .iter()
        .find(|command| command.name == "model")
        .expect("model command should be projected");
    let model_add = model
        .subcommands
        .iter()
        .find(|subcommand| subcommand.name == "add")
        .expect("model add should be projected");
    let id_field = model_add
        .fields
        .iter()
        .find(|field| field.name == "id")
        .expect("id field should be projected");
    assert!(matches!(
        &id_field.values,
        CompletionValues::CachedModelIds(_)
    ));

    let git = commands
        .iter()
        .find(|command| command.name == "git")
        .expect("git command should be projected");
    assert_eq!(
        git.subcommands
            .iter()
            .map(|subcommand| subcommand.name.as_str())
            .collect::<Vec<_>>(),
        vec!["status", "commit"]
    );
}

/// Verifies runtime, display, and session metadata maps to TUI state actions.
#[test]
fn runtime_config_and_session_metadata_map_to_tui_actions() {
    let runtime = RuntimeSelection {
        provider_type: "openrouter".to_owned(),
        provider_auth: Some(ProviderAuthMode::ApiKey),
        provider: "openrouter".to_owned(),
        api_key: "sk-test".to_owned(),
        model_key: "coding".to_owned(),
        model: "test/model".to_owned(),
        reasoning: ::config::ReasoningLevel::Medium,
        context_window_tokens: Some(128_000),
    };
    let usage = Some(::agent::ContextTokenUsage {
        input_tokens: 10,
        context_window_tokens: Some(128_000),
    });

    assert_eq!(
        runtime_selection_action(&runtime),
        ChatTuiAction::RuntimeSelectionChanged(TuiRuntimeSelection::new(
            "openrouter",
            "openrouter",
            "test/model",
            TuiReasoningLevel::Medium,
            Some(128_000),
        ))
    );
    assert_eq!(
        display_metadata_action("session-1", &runtime, Path::new("/workspace"), usage),
        ChatTuiAction::DisplayMetadataChanged(TuiDisplayMetadata::new(
            "openrouter",
            "test/model",
            "medium",
            "/workspace",
            "session-1",
            Some(TuiContextTokenUsage::new(10, Some(128_000))),
        ))
    );
    assert_eq!(
        session_changed_action("session-1"),
        ChatTuiAction::SessionChanged {
            id: SessionId::new("session-1"),
        }
    );
}

/// Verifies user prompt submission maps into semantic TUI prompt submission.
#[test]
fn user_prompt_submission_maps_to_submit_prompt() {
    assert_eq!(
        submit_prompt_action("prompt-1", "hello"),
        ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("prompt-1"),
            text: "hello".to_owned(),
        }
    );
}

/// Verifies user prompt agent events only render when the runtime supplied a prompt ID.
#[test]
fn user_prompt_agent_events_require_prompt_id() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::user_prompt_with_id("local-prompt-1", "hello")),
        vec![ChatTuiAction::SubmitPrompt {
            id: TranscriptItemId::new("local-prompt-1"),
            text: "hello".to_owned(),
        }]
    );
    assert!(adapter
        .adapt_agent_event(&AgentEvent::user_prompt("no id"))
        .is_empty());
}

/// Verifies usage events map to distinct context estimates and provider turn usage updates.
#[test]
fn usage_events_map_to_usage_actions() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::ContextTokenUsage(
            ::agent::ContextTokenUsage {
                input_tokens: 42,
                context_window_tokens: Some(100),
            },
        )),
        vec![ChatTuiAction::ContextUsageUpdated(
            TuiContextTokenUsage::new(42, Some(100)),
        )]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::UsageMetadata(UsageMetadata {
            input_tokens: Some(7),
            output_tokens: Some(11),
            total_tokens: Some(18),
        })),
        vec![ChatTuiAction::ProviderUsageReported(
            TuiProviderUsageMetadata::new(Some(7), Some(11), Some(18)),
        )]
    );
}

/// Verifies runtime terminal failures produce deterministic status and transcript actions.
#[test]
fn runtime_error_and_cancelled_events_map_to_terminal_actions() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::error("boom")),
        vec![ChatTuiAction::AgentFailed {
            message: "boom".to_owned(),
            details: None,
        }]
    );
    let details = AgentErrorDetails {
        kind: AgentErrorKind::ProviderUnavailable,
        provider: Some("OpenAI".to_owned()),
        stage: Some(AgentErrorStage::HttpStatus),
        retryable: true,
        http_status: Some(503),
        provider_code: Some("temporarily_unavailable".to_owned()),
        excerpt: Some("temporary outage".to_owned()),
        debug_events: vec!["responses_error_body".to_owned()],
    };
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::error_with_details(
            "OpenAI is unavailable",
            details
        )),
        vec![ChatTuiAction::AgentFailed {
            message: "OpenAI is unavailable".to_owned(),
            details: Some(
                "kind: provider_unavailable\nprovider: OpenAI\nstage: http_status\nretryable: true\nhttp status: 503\nprovider code: temporarily_unavailable\nexcerpt: temporary outage\ndebug events: responses_error_body"
                    .to_owned()
            ),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::cancelled("user stopped run")),
        vec![ChatTuiAction::AgentCancelled {
            reason: "user stopped run".to_owned(),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::Finished {
            finish_reason: FinishReason::Length,
        }),
        vec![ChatTuiAction::AgentFailed {
            message: "provider response reached the length limit".to_owned(),
            details: None,
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::Finished {
            finish_reason: FinishReason::ToolCalls,
        }),
        vec![ChatTuiAction::AgentFailed {
            message: "provider requested tool calls without completing the run".to_owned(),
            details: None,
        }]
    );
}

/// Provides a no-op command handler for registry projection tests.
fn noop_command<'a>(
    _context: &'a mut (),
    _args: Vec<String>,
) -> Pin<
    Box<
        dyn Future<Output = Result<CommandControl, ::commands::CommandError>> + Send + 'a,
    >,
> {
    Box::pin(async { Ok(CommandControl::Continue) })
}

/// Builds a chat model for TUI adapter projection tests.
fn tui_test_model() -> crate::chat::model::ChatModel {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let session = crate::chat::session::SessionManager::new_in(
        std::env::temp_dir().join(format!("tui-adapter-{suffix}")),
    )
    .expect("session manager should be created");
    crate::chat::model::ChatModel::new(
        session,
        RuntimeSelection {
            provider_type: "openrouter".to_owned(),
            provider_auth: Some(ProviderAuthMode::ApiKey),
            provider: "openrouter".to_owned(),
            api_key: "sk-test".to_owned(),
            model_key: "coding".to_owned(),
            model: "openai/gpt-5.5".to_owned(),
            reasoning: ::config::ReasoningLevel::Medium,
            context_window_tokens: None,
        },
    )
}
