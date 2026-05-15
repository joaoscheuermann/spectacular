use super::*;
use spectacular_agent::AgentEvent;
use spectacular_commands::{Command, CommandControl, CommandRegistry};
use spectacular_config::ProviderAuthMode;
use spectacular_llms::{FinishReason, MessageDelta, ReasoningDelta, UsageMetadata};
use spectacular_tui::{
    ChatTuiAction, CommandDescriptor, ContextTokenUsage as TuiContextTokenUsage,
    DisplayMetadata as TuiDisplayMetadata, ReasoningLevel as TuiReasoningLevel,
    RuntimeSelection as TuiRuntimeSelection, SessionId, TranscriptItemId,
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

/// Verifies assistant lifecycle IDs are created once and reused for deltas and finish.
#[test]
fn assistant_lifecycle_events_reuse_the_same_transcript_id() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::MessageDelta(MessageDelta::assistant("hello "))),
        vec![
            ChatTuiAction::MessageStarted {
                id: TranscriptItemId::new("message-1"),
            },
            ChatTuiAction::MessageDelta {
                id: TranscriptItemId::new("message-1"),
                text: "hello ".to_owned(),
            },
        ]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::MessageDelta(MessageDelta::assistant("world"))),
        vec![ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("message-1"),
            text: "world".to_owned(),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        }),
        vec![
            ChatTuiAction::MessageFinished {
                id: TranscriptItemId::new("message-1"),
            },
            ChatTuiAction::AgentFinished,
        ]
    );
}

/// Verifies reasoning lifecycle IDs are created once and reused for deltas and finish.
#[test]
fn reasoning_lifecycle_events_reuse_the_same_transcript_id() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "thinking ".to_owned(),
            metadata: None,
        })),
        vec![
            ChatTuiAction::ReasoningStarted {
                id: TranscriptItemId::new("reasoning-1"),
            },
            ChatTuiAction::ReasoningDelta {
                id: TranscriptItemId::new("reasoning-1"),
                text: "thinking ".to_owned(),
            },
        ]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::ReasoningDelta(ReasoningDelta {
            content: "hard".to_owned(),
            metadata: None,
        })),
        vec![ChatTuiAction::ReasoningDelta {
            id: TranscriptItemId::new("reasoning-1"),
            text: "hard".to_owned(),
        }]
    );
    assert_eq!(
        adapter.adapt_agent_event(&AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        }),
        vec![
            ChatTuiAction::ReasoningFinished {
                id: TranscriptItemId::new("reasoning-1"),
            },
            ChatTuiAction::AgentFinished,
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
