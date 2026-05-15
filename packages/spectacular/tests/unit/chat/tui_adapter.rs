use super::*;
use spectacular_agent::AgentEvent;
use spectacular_commands::{Command, CommandControl, CommandRegistry};
use spectacular_config::ProviderAuthMode;
use spectacular_llms::{FinishReason, UsageMetadata};
use spectacular_tui::{
    ChatTuiAction, CommandDescriptor, ContextTokenUsage as TuiContextTokenUsage,
    DisplayLine, DisplayLineStyle, DisplayMetadata as TuiDisplayMetadata,
    ReasoningLevel as TuiReasoningLevel, RuntimeSelection as TuiRuntimeSelection, SessionId,
    ToolDisplayStatus, TranscriptItemId,
};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

mod tui_adapter_display;

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
                call_line: DisplayLine::new("read", DisplayLineStyle::Tool),
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
        reasoning: spectacular_config::ReasoningLevel::Medium,
        context_window_tokens: Some(128_000),
    };
    let usage = Some(spectacular_agent::ContextTokenUsage {
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

/// Verifies provider usage and context usage events map to TUI usage state updates.
#[test]
fn usage_events_map_to_usage_updated() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::ContextTokenUsage(
            spectacular_agent::ContextTokenUsage {
                input_tokens: 42,
                context_window_tokens: Some(100),
            },
        )),
        vec![ChatTuiAction::UsageUpdated(TuiContextTokenUsage::new(
            42,
            Some(100),
        ))]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::UsageMetadata(UsageMetadata {
            input_tokens: Some(7),
            output_tokens: Some(11),
            total_tokens: Some(18),
        })),
        vec![ChatTuiAction::UsageUpdated(TuiContextTokenUsage::new(
            18, None,
        ))]
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
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::Finished {
            finish_reason: FinishReason::ToolCalls,
        }),
        vec![ChatTuiAction::AgentFailed {
            message: "provider requested tool calls without completing the run".to_owned(),
        }]
    );
}

/// Provides a no-op command handler for registry projection tests.
fn noop_command<'a>(
    _context: &'a mut (),
    _args: Vec<String>,
) -> Pin<
    Box<
        dyn Future<Output = Result<CommandControl, spectacular_commands::CommandError>> + Send + 'a,
    >,
> {
    Box::pin(async { Ok(CommandControl::Continue) })
}
