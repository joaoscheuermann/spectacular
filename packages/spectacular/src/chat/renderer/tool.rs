use super::json_preview::format_json_preview;
use serde_json::Value;
use spectacular_agent::ToolStorage;

/// Display model for a tool call line.
pub struct ToolCallView {
    pub name: String,
    pub input: String,
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
        let input = match (tools.get(name), parsed_arguments.as_ref()) {
            (Some(tool), Some(arguments)) => tool.format_input(arguments),
            _ => format_json_preview(arguments),
        };

        Self {
            name: name.to_owned(),
            input,
        }
    }
}

impl ToolResultView {
    /// Builds display data for a tool result, preferring registered tool formatting.
    pub fn from_parts(name: &str, content: &str, tools: &ToolStorage) -> Self {
        let parsed = serde_json::from_str::<Value>(content).ok();
        let output = match tools.get(name) {
            Some(tool) => tool.format_output(content, parsed.as_ref()),
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
