use super::*;
use crate::chat::command_event::{
    CommandDelta, CommandEvent, CommandFinished, CommandStart, CommandStatus,
};
use crate::chat::renderer::{ToolCallView, ToolResultView};
use serde_json::{json, Value};
use spectacular_agent::{AgentEvent, Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest, ToolStorage};
use spectacular_tools::edit::EditTool;
use spectacular_tui::{
    ChatTuiAction, CommandDisplayChunk, CommandDisplayStatus, DisplayLine, DisplayLineStyle,
    ToolDisplayStatus, TranscriptItemId,
};

#[derive(Clone, Debug)]
struct DisplayTool;

impl Tool for DisplayTool {
    /// Returns the stable tool name used in manifests and calls.
    fn name(&self) -> &str {
        "display_tool"
    }

    /// Builds the test tool manifest exposed to provider requests.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            self.name(),
            "Formats TUI adapter payloads.",
            json!({"type": "object", "additionalProperties": true}),
        )
    }

    /// Formats tool input arguments for display payloads.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        format!("registered input: {}", arguments["path"].as_str().unwrap())
    }

    /// Formats tool output for display payloads.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let state = parsed_output
            .and_then(|value| value.get("success"))
            .and_then(Value::as_bool)
            .map(|success| format!("success={success}"))
            .unwrap_or_else(|| "parsed=none".to_owned());
        format!("registered output: {state}; raw={raw_output}")
    }

    /// Executes the tool with the provided arguments and cancellation handle.
    fn execute<'a>(&'a self, _arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async { Ok(r#"{"success":true}"#.to_owned()) })
    }
}

/// Builds a tool storage containing the registered display test tool.
fn display_tools() -> ToolStorage {
    ToolStorage::try_with_tool(DisplayTool).unwrap()
}

/// Builds one display line for expected adapter payloads.
fn line(text: impl Into<String>, style: DisplayLineStyle) -> DisplayLine {
    DisplayLine::new(text, style)
}

/// Verifies a tool result payload is classified as failed by the adapter.
fn assert_tool_result_failed(content: &str) {
    let mut adapter = TuiEventAdapter::new();
    adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_start("call-1", "missing_tool", "{}"),
        &ToolStorage::default(),
    );

    let result = ToolResultView::from_parts_with_arguments(
        "missing_tool",
        content,
        &ToolStorage::default(),
        None,
    );
    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_finish("call-1", "missing_tool", content),
            &ToolStorage::default(),
        ),
        vec![
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                output: content.to_owned(),
            },
            ChatTuiAction::ToolDisplayFinished {
                tool_call_id: "call-1".to_owned(),
                status: ToolDisplayStatus::Failed,
                output_lines: vec![line(result.output, DisplayLineStyle::Error)],
            },
        ]
    );
}

/// Verifies a tool result payload is classified as successful by the adapter.
fn assert_tool_result_succeeded(content: &str) {
    let mut adapter = TuiEventAdapter::new();
    adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_start("call-1", "missing_tool", "{}"),
        &ToolStorage::default(),
    );

    let result = ToolResultView::from_parts_with_arguments(
        "missing_tool",
        content,
        &ToolStorage::default(),
        None,
    );
    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_finish("call-1", "missing_tool", content),
            &ToolStorage::default(),
        ),
        vec![
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                output: content.to_owned(),
            },
            ChatTuiAction::ToolDisplayFinished {
                tool_call_id: "call-1".to_owned(),
                status: ToolDisplayStatus::Succeeded,
                output_lines: vec![line(result.output, DisplayLineStyle::CommandOutput)],
            },
        ]
    );
}

/// Verifies registered tool call display is formatted before crossing into the TUI crate.
#[test]
fn adapter_tool_call_uses_registered_formatter() {
    let mut adapter = TuiEventAdapter::new();
    let tools = display_tools();
    let arguments = r#"{"path":"README.md"}"#;
    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_start("call-1", "display_tool", arguments),
            &tools,
        ),
        vec![
            ChatTuiAction::ToolCallStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                arguments: arguments.to_owned(),
            },
            ChatTuiAction::ToolDisplayStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                call_line: line("display_tool registered input: README.md", DisplayLineStyle::Tool),
                argument_lines: Vec::new(),
            },
        ]
    );
}

/// Verifies repeated tool-call updates retain the same provider id and transcript id.
#[test]
fn adapter_tool_call_updates_keep_same_tool_call_id() {
    let mut adapter = TuiEventAdapter::new();
    let tools = display_tools();
    let first = r#"{"path":"README.md"}"#;
    let updated = r#"{"path":"src/lib.rs"}"#;

    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_start("call-1", "display_tool", first),
            &tools,
        ),
        vec![
            ChatTuiAction::ToolCallStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                arguments: first.to_owned(),
            },
            ChatTuiAction::ToolDisplayStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                call_line: line(
                    ToolCallView::from_parts("display_tool", first, &tools).line,
                    DisplayLineStyle::Tool,
                ),
                argument_lines: Vec::new(),
            },
        ]
    );
    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_start("call-1", "display_tool", updated),
            &tools,
        ),
        vec![
            ChatTuiAction::ToolCallStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                arguments: updated.to_owned(),
            },
            ChatTuiAction::ToolDisplayStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                call_line: line(
                    ToolCallView::from_parts("display_tool", updated, &tools).line,
                    DisplayLineStyle::Tool,
                ),
                argument_lines: Vec::new(),
            },
        ]
    );
}

/// Verifies unknown tools retain the original fallback preview shape.
#[test]
fn adapter_unknown_tool_uses_original_fallback_preview() {
    let mut adapter = TuiEventAdapter::new();

    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_start(
                "call-1",
                "missing_tool",
                r#"{"path":"README.md"}"#,
            ),
            &ToolStorage::default(),
        ),
        vec![
            ChatTuiAction::ToolCallStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                arguments: r#"{"path":"README.md"}"#.to_owned(),
            },
            ChatTuiAction::ToolDisplayStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                call_line: line("missing_tool path: README.md", DisplayLineStyle::Tool),
                argument_lines: Vec::new(),
            },
        ]
    );
}

/// Verifies fallback tool previews keep the original 180-character truncation limit.
#[test]
fn adapter_tool_preview_truncates_at_original_limit() {
    let mut adapter = TuiEventAdapter::new();
    let value = "a".repeat(181);
    let arguments = format!(r#"{{"value":"{value}"}}"#);
    let actions = adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_start("call-1", "missing_tool", &arguments),
        &ToolStorage::default(),
    );

    assert_eq!(
        actions,
        vec![
            ChatTuiAction::ToolCallStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                arguments,
            },
            ChatTuiAction::ToolDisplayStarted {
                id: TranscriptItemId::new("call-1"),
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                call_line: line(
                    format!("missing_tool value: {}...", "a".repeat(173)),
                    DisplayLineStyle::Tool,
                ),
                argument_lines: Vec::new(),
            },
        ]
    );
}

/// Verifies text Error-prefix outputs are classified as failed.
#[test]
fn adapter_tool_result_error_prefix_is_failed() {
    assert_tool_result_failed("Error: denied");
}

/// Verifies JSON error-field outputs are classified as failed.
#[test]
fn adapter_tool_result_error_field_is_failed() {
    assert_tool_result_failed(r#"{"error":"denied"}"#);
}

/// Verifies JSON error-kind outputs are classified as failed.
#[test]
fn adapter_tool_result_error_kind_field_is_failed() {
    assert_tool_result_failed(r#"{"error_kind":"timeout"}"#);
}

/// Verifies nonzero exit code outputs are classified as failed.
#[test]
fn adapter_tool_result_nonzero_exit_code_is_failed() {
    assert_tool_result_failed(r#"{"exit_code":1}"#);
}

/// Verifies zero and absent exit-code outputs remain successful.
#[test]
fn adapter_tool_result_zero_or_absent_exit_code_is_successful() {
    assert_tool_result_succeeded(r#"{"exit_code":0,"message":"ok"}"#);
    assert_tool_result_succeeded("normal output");
}

/// Verifies registered result display text matches the original renderer helper.
#[test]
fn adapter_tool_result_uses_registered_formatter_output() {
    let mut adapter = TuiEventAdapter::new();
    let tools = display_tools();
    let arguments = r#"{"path":"README.md"}"#;
    let content = r#"{"success":true}"#;
    adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_start("call-1", "display_tool", arguments),
        &tools,
    );

    let parsed_arguments = serde_json::from_str::<Value>(arguments).unwrap();
    let result = ToolResultView::from_parts_with_arguments(
        "display_tool",
        content,
        &tools,
        Some(&parsed_arguments),
    );
    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_finish("call-1", "display_tool", content),
            &tools,
        ),
        vec![
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "call-1".to_owned(),
                name: "display_tool".to_owned(),
                output: content.to_owned(),
            },
            ChatTuiAction::ToolDisplayFinished {
                tool_call_id: "call-1".to_owned(),
                status: ToolDisplayStatus::Succeeded,
                output_lines: vec![line(result.output, DisplayLineStyle::CommandOutput)],
            },
        ]
    );
}

/// Verifies registered result display receives original call arguments when formatting output.
#[test]
fn adapter_tool_result_uses_registered_formatter_with_input_context() {
    let mut adapter = TuiEventAdapter::new();
    let tools = ToolStorage::try_with_tool(EditTool::new(".")).unwrap();
    let arguments = r#"{"path":"src/lib.rs","edits":[{"oldText":"old","newText":"new"}]}"#;
    let content = r#"{"success":true,"diff":"1 -old\n1 +new"}"#;
    adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_start("call-1", "edit", arguments),
        &tools,
    );

    let parsed_arguments = serde_json::from_str::<Value>(arguments).unwrap();
    let result =
        ToolResultView::from_parts_with_arguments("edit", content, &tools, Some(&parsed_arguments));
    assert_eq!(result.output, "Edited src/lib.rs\n1 -old\n1 +new");
    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_finish("call-1", "edit", content),
            &tools
        ),
        vec![
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "call-1".to_owned(),
                name: "edit".to_owned(),
                output: content.to_owned(),
            },
            ChatTuiAction::ToolDisplayFinished {
                tool_call_id: "call-1".to_owned(),
                status: ToolDisplayStatus::Succeeded,
                output_lines: vec![
                    line("Edited src/lib.rs", DisplayLineStyle::CommandOutput),
                    line("1 -old", DisplayLineStyle::DiffRemoved),
                    line("1 +new", DisplayLineStyle::DiffAdded),
                ],
            },
        ]
    );
}

/// Verifies edited tool output rows get adapter-owned diff styles.
#[test]
fn adapter_tool_output_diff_lines_are_annotated() {
    let mut adapter = TuiEventAdapter::new();
    adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_start("call-1", "missing_tool", "{}"),
        &ToolStorage::default(),
    );

    assert_eq!(
        adapter.adapt_agent_event_with_tools(
            &AgentEvent::tool_call_finish(
                "call-1",
                "missing_tool",
                "Edited src/lib.rs\n1 -old\n1 +new"
            ),
            &ToolStorage::default(),
        ),
        vec![
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "call-1".to_owned(),
                name: "missing_tool".to_owned(),
                output: "Edited src/lib.rs\n1 -old\n1 +new".to_owned(),
            },
            ChatTuiAction::ToolDisplayFinished {
                tool_call_id: "call-1".to_owned(),
                status: ToolDisplayStatus::Succeeded,
                output_lines: vec![
                    line("Edited src/lib.rs", DisplayLineStyle::CommandOutput),
                    line("1 -old", DisplayLineStyle::DiffRemoved),
                    line("1 +new", DisplayLineStyle::DiffAdded),
                ],
            },
        ]
    );
}

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

/// Verifies cancelled commands keep cancellation status and original warning styling.
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
            summary_line: Some(line("cancelled", DisplayLineStyle::Warning)),
        }]
    );
}

/// Verifies TUI adapter formatting uses pure payload builders rather than terminal renderer writes.
#[test]
fn tui_path_does_not_write_tool_output_directly() {
    let mut adapter = TuiEventAdapter::new();

    let actions = adapter.adapt_agent_event_with_tools(
        &AgentEvent::tool_call_finish("call-1", "missing_tool", "contents"),
        &ToolStorage::default(),
    );

    assert!(matches!(
        actions.as_slice(),
        [ChatTuiAction::ToolCallFinished { .. }, ChatTuiAction::ToolDisplayFinished { .. }]
    ));
}
