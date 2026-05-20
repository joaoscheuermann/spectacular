use super::json_preview::format_json_preview;
use serde_json::Value;
use spectacular_agent::ToolStorage;

/// Display model for a tool call line.
pub struct ToolCallView {
    pub line: String,
}

/// Display model for a tool result block.
pub struct ToolResultView {
    pub output: String,
    pub status: ToolStatus,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ToolStatus {
    Done,
    Failed,
}

impl ToolCallView {
    /// Builds display data for a tool call, preferring registered tool formatting.
    pub fn from_parts(name: &str, arguments: &str, tools: &ToolStorage) -> Self {
        let name = if name.trim().is_empty() { "tool" } else { name };
        let parsed_arguments = serde_json::from_str::<Value>(arguments).ok();
        let line = match (tools.get(name), parsed_arguments.as_ref()) {
            (Some(tool), Some(arguments)) => tool.format_call(arguments),
            _ => format_fallback_call(name, arguments),
        };

        Self { line }
    }
}

impl ToolResultView {
    /// Builds display data for a tool result, passing original call arguments when available.
    pub fn from_parts_with_arguments(
        name: &str,
        content: &str,
        tools: &ToolStorage,
        arguments: Option<&Value>,
    ) -> Self {
        let parsed = serde_json::from_str::<Value>(content).ok();
        let output = match tools.get(name) {
            Some(tool) => tool.format_output_with_input(content, parsed.as_ref(), arguments),
            None => format_json_preview(content),
        };

        Self {
            output,
            status: if result_failed(content, parsed.as_ref()) {
                ToolStatus::Failed
            } else {
                ToolStatus::Done
            },
        }
    }
}

/// Formats unregistered or malformed tool calls with generic display parts.
fn format_fallback_call(name: &str, arguments: &str) -> String {
    let preview = format_json_preview(arguments);
    format_tool_call_parts(name, &preview, None)
}

/// Formats compatibility display parts with renderer styles.
pub(crate) fn format_tool_call_parts(label: &str, input: &str, metadata: Option<&str>) -> String {
    spectacular_tui::tool_line(label, input, metadata)
}

/// Reports whether a tool output matches the renderer's common failure markers.
fn result_failed(content: &str, parsed: Option<&Value>) -> bool {
    if content.trim_start().starts_with("Error:") {
        return true;
    }

    let Some(value) = parsed else {
        return false;
    };

    if value.get("error").is_some() || value.get("error_kind").is_some() {
        return true;
    }

    value
        .get("exit_code")
        .and_then(Value::as_i64)
        .is_some_and(|exit_code| exit_code != 0)
}
