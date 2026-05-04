use crate::chat::ChatError;
use anstyle::{AnsiColor, Style};
use serde_json::Value;
use spectacular_agent::ToolStorage;
use std::io::{self, Write};
use std::time::Duration;

/// Number of visible characters the terminal typewriter writes per tick while
/// an assistant delta is streaming. Combined with the 50 ms tick this paces
/// revealed text at ~600 chars/sec. Each provider delta is drained before the
/// renderer handles the next event, so no backlog is retained after the run
/// finishes or fails.
pub(crate) const TYPEWRITER_CHARS_PER_TICK: usize = 30;
const TYPEWRITER_TICK_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Default)]
pub struct Renderer;

pub struct ToolCallView {
    pub name: String,
    pub input: String,
}

pub struct ToolResultView {
    pub output: String,
    pub status: ToolStatus,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ToolStatus {
    Done,
    Failed,
}

impl Renderer {
    pub fn session_created(&self, id: &str) {
        self.notice(format!("new session {id}"));
    }

    pub fn resumed(&self, id: &str) {
        self.notice(format!("resumed session {id}"));
    }

    pub fn clear_screen(&self) {
        print!("\x1b[2J\x1b[3J\x1b[H");
        let _ = io::stdout().flush();
    }

    pub fn user_prompt(&self, prompt: &str) {
        println!("{}", paint(user_style(), prompt));
        println!();
    }

    pub fn prompt(&self) {
        print!("{}", paint(user_style(), "> "));
    }

    pub fn working(&self) {
        self.working_frame(0);
    }

    pub fn working_frame(&self, frame: usize) {
        const FRAMES: &[&str] = &["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = FRAMES[frame % FRAMES.len()];
        print!(
            "\r{}",
            paint(dim_style(), format!("working {frame} (Ctrl+C to stop)"))
        );
        let _ = io::stdout().flush();
    }

    pub fn clear_working(&self) {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }

    pub async fn assistant_delta(&self, content: &str) -> Result<(), ChatError> {
        let mut characters = content.chars().peekable();
        while characters.peek().is_some() {
            let chunk = characters
                .by_ref()
                .take(TYPEWRITER_CHARS_PER_TICK)
                .collect::<String>();
            print!("{}", paint(assistant_style(), chunk));
            io::stdout().flush().map_err(ChatError::Io)?;
            if characters.peek().is_some() {
                tokio::time::sleep(TYPEWRITER_TICK_INTERVAL).await;
            }
        }
        Ok(())
    }

    pub fn assistant_text(&self, content: &str) {
        println!("{}", paint(assistant_style(), content));
        println!();
    }

    pub fn tool_call(&self, _tool_call_id: &str, name: &str, arguments: &str, tools: &ToolStorage) {
        self.render_tool_call(&ToolCallView::from_parts(name, arguments, tools));
    }

    pub fn render_tool_call(&self, view: &ToolCallView) {
        println!("{} {}", paint(tool_style(), &view.name), view.input);
    }

    pub fn tool_result(&self, name: &str, content: &str, tools: &ToolStorage) {
        self.render_tool_result(&ToolResultView::from_parts(name, content, tools));
    }

    pub fn render_tool_result(&self, view: &ToolResultView) {
        println!("{}", view.output);
        let status = match view.status {
            ToolStatus::Done => paint(success_style(), "done"),
            ToolStatus::Failed => paint(error_style(), "failed"),
        };
        println!("{} {}", status, paint(dim_style(), "- ran"));
        println!();
    }

    pub fn error(&self, message: &str) {
        println!("{}", paint(error_style(), format!("error: {message}")));
        println!();
    }

    pub fn command_error(&self, error: &impl std::fmt::Display) {
        self.error(&error.to_string());
    }

    pub fn warning(&self, message: &str) {
        println!("{}", paint(warning_style(), format!("warning: {message}")));
    }

    pub fn cancelled(&self, reason: &str) {
        println!("{}", paint(warning_style(), reason));
        println!();
    }

    pub fn dim(&self, message: &str) {
        println!("{}", paint(dim_style(), message));
    }

    pub fn success(&self, message: &str) {
        println!("{}", paint(success_style(), message));
    }

    fn notice(&self, message: impl Into<String>) {
        use iocraft::prelude::*;

        let content = message.into();
        element! {
            Text(color: Color::DarkGrey, content: content)
        }
        .print();
        println!();
    }
}

impl ToolCallView {
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

pub fn paint(style: Style, value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    format!("{style}{value}{style:#}")
}

fn format_json_preview(value: &str) -> String {
    let parsed = serde_json::from_str::<Value>(value);
    let value = match parsed {
        Ok(Value::Object(map)) => map
            .into_iter()
            .map(|(key, value)| format!("{key}: {}", compact_value(&value)))
            .collect::<Vec<_>>()
            .join(", "),
        Ok(value) => compact_value(&value),
        Err(_) => value.to_owned(),
    };

    const LIMIT: usize = 180;
    if value.chars().count() <= LIMIT {
        return value;
    }

    value.chars().take(LIMIT).collect::<String>() + "..."
}

fn compact_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

pub fn user_style() -> Style {
    AnsiColor::BrightGreen.on_default()
}

fn assistant_style() -> Style {
    AnsiColor::BrightWhite.on_default()
}

fn tool_style() -> Style {
    AnsiColor::BrightMagenta.on_default().bold()
}

fn success_style() -> Style {
    AnsiColor::BrightGreen.on_default().bold()
}

fn warning_style() -> Style {
    AnsiColor::BrightYellow.on_default().bold()
}

fn error_style() -> Style {
    AnsiColor::BrightRed.on_default().bold()
}

pub fn dim_style() -> Style {
    AnsiColor::BrightBlack.on_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};

    #[derive(Clone, Debug)]
    struct DisplayTool;

    impl Tool for DisplayTool {
        fn name(&self) -> &str {
            "display_tool"
        }

        fn manifest(&self) -> ToolManifest {
            ToolManifest::new(
                self.name(),
                "Formats chat renderer payloads.",
                json!({"type": "object", "additionalProperties": true}),
            )
        }

        fn format_input(&self, arguments: &Value) -> ToolDisplay {
            format!("registered input: {}", arguments["path"].as_str().unwrap())
        }

        fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
            let state = parsed_output
                .and_then(|value| value.get("success"))
                .and_then(Value::as_bool)
                .map(|success| format!("success={success}"))
                .unwrap_or_else(|| "parsed=none".to_owned());
            format!("registered output: {state}; raw={raw_output}")
        }

        fn execute<'a>(
            &'a self,
            _arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
            Box::pin(async { Ok(r#"{"success":true}"#.to_owned()) })
        }
    }

    #[test]
    fn registered_tool_display_is_used_for_tool_call_and_result() {
        let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

        let call = ToolCallView::from_parts("display_tool", r#"{"path":"foo.txt"}"#, &tools);
        let result = ToolResultView::from_parts("display_tool", r#"{"success":true}"#, &tools);

        assert_eq!(call.name, "display_tool");
        assert_eq!(call.input, "registered input: foo.txt");
        assert_eq!(
            result.output,
            r#"registered output: success=true; raw={"success":true}"#
        );
        assert_eq!(result.status, ToolStatus::Done);
    }

    #[test]
    fn malformed_registered_tool_arguments_use_generic_fallback() {
        let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

        let call = ToolCallView::from_parts("display_tool", "{", &tools);

        assert_eq!(call.input, "{");
    }

    #[test]
    fn registered_tool_receives_none_for_non_json_output() {
        let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

        let result = ToolResultView::from_parts("display_tool", "not json", &tools);

        assert_eq!(
            result.output,
            "registered output: parsed=none; raw=not json"
        );
        assert_eq!(result.status, ToolStatus::Done);
    }

    #[test]
    fn result_status_marks_common_failures() {
        let tools = ToolStorage::default();

        assert_eq!(
            ToolResultView::from_parts("missing", r#"{"exit_code":1}"#, &tools).status,
            ToolStatus::Failed
        );
        assert_eq!(
            ToolResultView::from_parts("missing", r#"{"error_kind":"timeout"}"#, &tools).status,
            ToolStatus::Failed
        );
        assert_eq!(
            ToolResultView::from_parts("missing", "Error: failed", &tools).status,
            ToolStatus::Failed
        );
        assert_eq!(
            ToolResultView::from_parts("missing", r#"{"exit_code":0}"#, &tools).status,
            ToolStatus::Done
        );
    }

    #[test]
    fn missing_tool_uses_generic_preview_for_session_replay() {
        let tools = ToolStorage::default();

        let call = ToolCallView::from_parts("missing_tool", r#"{"path":"foo.txt"}"#, &tools);
        let result = ToolResultView::from_parts("missing_tool", r#"{"success":true}"#, &tools);

        assert_eq!(call.name, "missing_tool");
        assert_eq!(call.input, "path: foo.txt");
        assert_eq!(result.output, "success: true");
        assert_eq!(result.status, ToolStatus::Done);
    }
}
