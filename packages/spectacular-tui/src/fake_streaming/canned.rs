use super::plan::FakeStreamingPlan;
use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::metadata::ProviderUsageMetadata;
use std::time::Duration;

/// Builds the main deterministic fake streaming lifecycle used by tests and the example.
pub fn fake_streaming_plan() -> FakeStreamingPlan {
    FakeStreamingPlan::new(vec![
        (Duration::ZERO, ChatTuiAction::AgentStarted),
        (
            Duration::from_millis(10),
            ChatTuiAction::ReasoningStarted {
                id: item_id("reasoning-1"),
            },
        ),
        (
            Duration::from_millis(20),
            ChatTuiAction::ReasoningDelta {
                id: item_id("reasoning-1"),
                text: "Plan the response, ".to_owned(),
            },
        ),
        (
            Duration::from_millis(35),
            ChatTuiAction::ReasoningDelta {
                id: item_id("reasoning-1"),
                text: "then stream it.".to_owned(),
            },
        ),
        (
            Duration::from_millis(45),
            ChatTuiAction::ReasoningFinished {
                id: item_id("reasoning-1"),
            },
        ),
        (
            Duration::from_millis(55),
            ChatTuiAction::MessageStarted {
                id: item_id("assistant-1"),
            },
        ),
        (
            Duration::from_millis(65),
            ChatTuiAction::MessageDelta {
                id: item_id("assistant-1"),
                text: "Hello ".to_owned(),
            },
        ),
        (
            Duration::from_millis(110),
            ChatTuiAction::MessageDelta {
                id: item_id("assistant-1"),
                text: "from the fake ".to_owned(),
            },
        ),
        (
            Duration::from_millis(150),
            ChatTuiAction::MessageDelta {
                id: item_id("assistant-1"),
                text: "async stream.".to_owned(),
            },
        ),
        (
            Duration::from_millis(160),
            ChatTuiAction::MessageFinished {
                id: item_id("assistant-1"),
            },
        ),
        (
            Duration::from_millis(170),
            ChatTuiAction::ToolCallStarted {
                id: item_id("tool-item-1"),
                tool_call_id: "tool-call-1".to_owned(),
                name: "grep".to_owned(),
                arguments: "pattern: fake".to_owned(),
            },
        ),
        (
            Duration::from_millis(200),
            ChatTuiAction::ToolCallDelta {
                tool_call_id: "tool-call-1".to_owned(),
                text: "found match".to_owned(),
            },
        ),
        (
            Duration::from_millis(230),
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "tool-call-1".to_owned(),
                name: "grep".to_owned(),
                output: "found match in src/lib.rs".to_owned(),
            },
        ),
        (
            Duration::from_millis(240),
            ChatTuiAction::CommandStarted {
                id: item_id("command-item-1"),
                command_id: "command-1".to_owned(),
                command: "cargo check -p fake".to_owned(),
            },
        ),
        (
            Duration::from_millis(260),
            ChatTuiAction::CommandOutput {
                command_id: "command-1".to_owned(),
                text: "checking fake workspace\n".to_owned(),
            },
        ),
        (
            Duration::from_millis(300),
            ChatTuiAction::CommandOutput {
                command_id: "command-1".to_owned(),
                text: "finished\n".to_owned(),
            },
        ),
        (
            Duration::from_millis(320),
            ChatTuiAction::CommandFinished {
                command_id: "command-1".to_owned(),
                exit_code: Some(0),
            },
        ),
        (
            Duration::from_millis(330),
            ChatTuiAction::ProviderUsageReported(ProviderUsageMetadata::new(
                Some(21),
                Some(21),
                Some(42),
            )),
        ),
        (Duration::from_millis(340), ChatTuiAction::AgentFinished),
    ])
}

/// Builds a deterministic fake cancellation lifecycle for terminal-state validation.
pub fn fake_cancellation_plan() -> FakeStreamingPlan {
    FakeStreamingPlan::new(vec![
        (Duration::ZERO, ChatTuiAction::AgentStarted),
        (
            Duration::from_millis(10),
            ChatTuiAction::MessageStarted {
                id: item_id("cancel-message"),
            },
        ),
        (
            Duration::from_millis(20),
            ChatTuiAction::AgentCancelled {
                reason: "cancelled by fake runtime".to_owned(),
            },
        ),
    ])
}

/// Builds a deterministic fake failure lifecycle for terminal-state validation.
pub fn fake_failure_plan() -> FakeStreamingPlan {
    FakeStreamingPlan::new(vec![
        (Duration::ZERO, ChatTuiAction::AgentStarted),
        (
            Duration::from_millis(10),
            ChatTuiAction::AgentFailed {
                message: "fake runtime failure".to_owned(),
                details: None,
            },
        ),
    ])
}

/// Creates a transcript item ID for fake runtime actions.
fn item_id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}
