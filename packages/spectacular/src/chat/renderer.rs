use crate::chat::ChatError;
use anstyle::{AnsiColor, Style};
use serde_json::Value;
use std::io::{self, Write};
use std::time::Duration;

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
        for character in content.chars() {
            print!("{}", paint(assistant_style(), character.to_string()));
            io::stdout().flush().map_err(ChatError::Io)?;
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        Ok(())
    }

    pub fn assistant_text(&self, content: &str) {
        println!("{}", paint(assistant_style(), content));
        println!();
    }

    pub fn tool_call(&self, content: &str) {
        self.render_tool_call(&ToolCallView::from_content(content));
    }

    pub fn render_tool_call(&self, view: &ToolCallView) {
        println!("{} <- {}", paint(tool_style(), &view.name), view.input);
    }

    pub fn tool_result(&self, content: &str) {
        self.render_tool_result(&ToolResultView::from_content(content));
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
    fn from_content(content: &str) -> Self {
        let value = serde_json::from_str::<Value>(content).unwrap_or(Value::String(content.into()));
        let name = value.get("name").and_then(Value::as_str).unwrap_or("tool");
        let input = value
            .get("arguments")
            .and_then(Value::as_str)
            .map(format_json_preview)
            .unwrap_or_else(|| format_json_preview(&value.to_string()));
        Self {
            name: name.to_owned(),
            input,
        }
    }
}

impl ToolResultView {
    fn from_content(content: &str) -> Self {
        let parsed = serde_json::from_str::<Value>(content);
        let failed = parsed
            .as_ref()
            .ok()
            .and_then(|value| value.get("error"))
            .is_some();
        Self {
            output: format_json_preview(content),
            status: if failed {
                ToolStatus::Failed
            } else {
                ToolStatus::Done
            },
        }
    }
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
