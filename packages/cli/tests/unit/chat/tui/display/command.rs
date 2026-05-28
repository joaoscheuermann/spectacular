use super::*;
use crate::chat::command_event::{
    CommandDelta, CommandEvent, CommandFinished, CommandStart, CommandStatus,
};
use ::tui::{CommandDisplayChunk, CommandDisplayStatus, TranscriptItemId};

/// Verifies command events emit display-ready start, output, and finish payloads.
#[test]
fn adapter_command_lifecycle_emits_start_delta_finish() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Start(CommandStart {
            command_id: "cmd-1".to_owned(),
            source: "slash_command".to_owned(),
            name: "git".to_owned(),
            title: "Git status".to_owned(),
            command: "/git status".to_owned(),
            working_directory: None,
        })),
        vec![ChatTuiAction::CommandDisplayStarted {
            id: TranscriptItemId::new("command-cmd-1"),
            command_id: "cmd-1".to_owned(),
            command_line: line("/git status", DisplayLineStyle::Command),
        }]
    );
    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Delta(CommandDelta {
            command_id: "cmd-1".to_owned(),
            channel: "stdout".to_owned(),
            content: "ok".to_owned(),
            sequence: 1,
        })),
        vec![ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• ok", DisplayLineStyle::CommandOutput),
        }]
    );
    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Finished(CommandFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandStatus::Success,
            summary: "done".to_owned(),
        })),
        vec![ChatTuiAction::CommandDisplayFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandDisplayStatus::Succeeded,
            exit_code: Some(0),
            summary_line: Some(line("done", DisplayLineStyle::Success)),
        }]
    );
}

/// Verifies command output chunks preserve original renderer line-oriented payloads.
#[test]
fn adapter_command_output_preserves_partial_chunk_shape() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Delta(CommandDelta {
            command_id: "cmd-1".to_owned(),
            channel: "stdout".to_owned(),
            content: "part".to_owned(),
            sequence: 1,
        })),
        vec![ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• part", DisplayLineStyle::CommandOutput),
        }]
    );
    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Delta(CommandDelta {
            command_id: "cmd-1".to_owned(),
            channel: "stdout".to_owned(),
            content: "ial".to_owned(),
            sequence: 2,
        })),
        vec![ChatTuiAction::CommandDisplayOutput {
            command_id: "cmd-1".to_owned(),
            chunk: CommandDisplayChunk::new("• ial", DisplayLineStyle::CommandOutput),
        }]
    );
}

/// Verifies failed commands keep original error-prefixed summary styling.
#[test]
fn adapter_command_failed_uses_original_error_summary_display() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Finished(CommandFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandStatus::Failed,
            summary: "failed".to_owned(),
        })),
        vec![ChatTuiAction::CommandDisplayFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandDisplayStatus::Failed,
            exit_code: Some(1),
            summary_line: Some(line("error: failed", DisplayLineStyle::Error)),
        }]
    );
}

/// Verifies cancelled commands keep cancellation status and original warning-prefixed styling.
#[test]
fn adapter_command_cancelled_keeps_cancelled_warning_display() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_command_event(&CommandEvent::Finished(CommandFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandStatus::Cancelled,
            summary: "cancelled".to_owned(),
        })),
        vec![ChatTuiAction::CommandDisplayFinished {
            command_id: "cmd-1".to_owned(),
            status: CommandDisplayStatus::Cancelled,
            exit_code: Some(1),
            summary_line: Some(line("warning: cancelled", DisplayLineStyle::Warning)),
        }]
    );
}
