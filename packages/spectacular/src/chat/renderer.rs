mod banner;
mod directory;
mod footer;
mod style;
#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/renderer.rs"
    ));
}

use crate::chat::model::{ChatPromptFooterModel, HistoryTableModel};
use crate::chat::runner::render_agent_event;
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
use banner::{format_opening_banner, OpeningBannerView};
use footer::{format_user_prompt_footer, UserPromptFooterView};
use serde_json::Value;
use spectacular_agent::AgentEvent;
use spectacular_agent::ToolStorage;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use style::{assistant_style, error_style, success_style, tool_style, warning_style};
pub(crate) use style::{dim_style, paint, selection_style, user_style};

use super::RuntimeSelection;

/// Number of visible characters the terminal typewriter writes per tick while
/// an assistant delta is streaming. Combined with the 50 ms tick this paces
/// revealed text at ~600 chars/sec. Each provider delta is drained before the
/// renderer handles the next event, so no backlog is retained after the run
/// finishes or fails.
pub(crate) const TYPEWRITER_CHARS_PER_TICK: usize = 30;
const TYPEWRITER_TICK_INTERVAL: Duration = Duration::from_millis(50);
const OPENING_BANNER_MIN_WIDTH: usize = 52;
const TOOL_RESULT_PREFIX: &str = "└";
const WORKING_FRAMES: &[&str] = &["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Default)]
pub struct Renderer {
    working: Mutex<WorkingLineState>,
}

#[derive(Default)]
struct WorkingLineState {
    active: bool,
    frame: usize,
    pause_depth: usize,
}

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
    pub fn session_created(&self, id: &str, runtime: &RuntimeSelection, directory: &Path) {
        let banner = OpeningBannerView::from_runtime(id, runtime, directory);
        self.with_interrupted_working_line(|| {
            println!("{}", format_opening_banner(&banner));
            println!();
        });
    }

    pub fn resumed(&self, id: &str) {
        self.notice(format!("resumed session {id}"));
    }

    pub fn clear_screen(&self) {
        self.reset_working();
        print!("\x1b[2J\x1b[3J\x1b[H");
        let _ = io::stdout().flush();
    }

    pub fn user_prompt(&self, prompt: &str) {
        self.render_user_prompt(prompt, None);
    }

    /// Renders a newly submitted user prompt followed by its dim context footer.
    pub fn user_prompt_with_footer(&self, prompt: &str, footer: &ChatPromptFooterModel) {
        self.render_user_prompt(prompt, Some(footer));
    }

    pub fn prompt(&self) {
        self.interrupt_working_line();
        print!("{}", paint(user_style(), "> "));
    }

    pub fn working(&self) {
        self.working_frame(0);
    }

    pub fn working_frame(&self, frame: usize) {
        let should_render = {
            let mut state = self.working_state();
            state.active = true;
            state.frame = frame;
            state.pause_depth == 0
        };

        if should_render {
            Self::write_working_frame(frame);
        }
    }

    pub fn clear_working(&self) {
        self.reset_working();
        Self::clear_working_line();
    }

    pub async fn assistant_delta(&self, content: &str) -> Result<(), ChatError> {
        let paused = self.pause_working();
        let result = async {
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
        .await;
        if paused {
            self.resume_working();
        }

        result
    }

    pub fn assistant_text(&self, content: &str) {
        if !has_visible_assistant_text(content) {
            return;
        }

        self.with_interrupted_working_line(|| {
            println!("{}", paint(assistant_style(), content));
            println!();
        });
    }

    pub fn tool_call(&self, _tool_call_id: &str, name: &str, arguments: &str, tools: &ToolStorage) {
        self.render_tool_call(&ToolCallView::from_parts(name, arguments, tools));
    }

    pub fn render_tool_call(&self, view: &ToolCallView) {
        self.with_interrupted_working_line(|| {
            println!("{} {}", paint(tool_style(), &view.name), view.input);
        });
    }

    pub fn tool_result(&self, name: &str, content: &str, tools: &ToolStorage) {
        self.render_tool_result(&ToolResultView::from_parts(name, content, tools));
    }

    pub fn render_tool_result(&self, view: &ToolResultView) {
        let _ = &view.status;
        self.with_interrupted_working_line(|| {
            println!("{TOOL_RESULT_PREFIX} {}", view.output);
            println!();
        });
    }

    pub fn error(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(error_style(), format!("error: {message}")));
            println!();
        });
    }

    pub fn command_error(&self, error: &impl std::fmt::Display) {
        self.error(&error.to_string());
    }

    pub fn warning(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(warning_style(), format!("warning: {message}")));
        });
    }

    pub fn cancelled(&self, reason: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(warning_style(), reason));
            println!();
        });
    }

    pub fn dim(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), message));
        });
    }

    pub fn success(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(success_style(), message));
        });
    }

    pub async fn render_records(
        &self,
        records: &[ChatRecord],
        tools: &ToolStorage,
    ) -> Result<(), ChatError> {
        let mut assistant_buffer = String::new();
        for record in records {
            if matches!(record, ChatRecord::Corrupt { .. }) {
                self.flush_assistant(&mut assistant_buffer);
                self.warning(&format!(
                    "unreadable session event at line {}",
                    record.line()
                ));
                continue;
            }

            let Some(event) = record.event() else {
                self.flush_assistant(&mut assistant_buffer);
                let event_type = match record {
                    ChatRecord::Unknown { value, .. } => value
                        .get("type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown"),
                    ChatRecord::Corrupt { .. } | ChatRecord::Known { .. } => "unknown",
                };
                self.warning(&format!(
                    "unknown session event `{event_type}` at line {}",
                    record.line()
                ));
                continue;
            };

            let Some(event) = event.to_agent_event() else {
                continue;
            };

            if let AgentEvent::MessageDelta(delta) = &event {
                assistant_buffer.push_str(&delta.content);
                continue;
            }

            self.flush_assistant(&mut assistant_buffer);
            render_agent_event(self, tools, &event).await?;
        }

        self.flush_assistant(&mut assistant_buffer);
        Ok(())
    }

    pub fn history_table(&self, table: &HistoryTableModel) {
        self.with_interrupted_working_line(|| {
            println!("sessions");
            println!("hash      updated           title                  messages");
            for session in &table.rows {
                let marker = if session.corrupt { "*" } else { " " };
                println!(
                    "{:<8}  {:<16}  {:<22}  {}{}",
                    session.id, session.updated, session.title, session.messages, marker
                );
            }
        });
        if table.remaining > 0 {
            self.with_interrupted_working_line(|| {
                println!();
            });
            self.dim(&format!("{} more sessions", table.remaining));
        }
    }

    /// Renders a user prompt with an optional footer while preserving working-line state.
    fn render_user_prompt(&self, prompt: &str, footer: Option<&ChatPromptFooterModel>) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(user_style(), prompt));
            if let Some(footer) = footer {
                println!("{}", format_prompt_footer(footer));
            }
            println!();
        });
    }

    fn flush_assistant(&self, buffer: &mut String) {
        if !has_visible_assistant_text(buffer) {
            buffer.clear();
            return;
        }

        self.assistant_text(buffer);
        buffer.clear();
    }

    fn notice(&self, message: impl Into<String>) {
        let content = message.into();
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), content));
        });
    }

    fn working_state(&self) -> MutexGuard<'_, WorkingLineState> {
        self.working
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn reset_working(&self) {
        let mut state = self.working_state();
        state.active = false;
        state.pause_depth = 0;
    }

    fn renderable_working_frame(&self) -> Option<usize> {
        let state = self.working_state();
        if state.active && state.pause_depth == 0 {
            return Some(state.frame);
        }

        None
    }

    fn with_interrupted_working_line(&self, write: impl FnOnce()) {
        let frame = self.renderable_working_frame();
        if frame.is_some() {
            Self::clear_working_line();
        }

        write();

        if let Some(frame) = frame {
            if self.renderable_working_frame().is_some() {
                Self::write_working_frame(frame);
            }
        }
    }

    fn interrupt_working_line(&self) {
        if self.renderable_working_frame().is_some() {
            Self::clear_working_line();
        }
    }

    fn pause_working(&self) -> bool {
        let (did_pause, should_clear) = {
            let mut state = self.working_state();
            let should_clear = state.active && state.pause_depth == 0;
            let did_pause = state.active;
            if state.active {
                state.pause_depth += 1;
            }
            (did_pause, should_clear)
        };

        if should_clear {
            Self::clear_working_line();
        }

        did_pause
    }

    fn resume_working(&self) {
        let mut state = self.working_state();
        if state.pause_depth > 0 {
            state.pause_depth -= 1;
        }
    }

    fn write_working_frame(frame: usize) {
        let frame = WORKING_FRAMES[frame % WORKING_FRAMES.len()];
        print!(
            "\r\x1b[2K{}",
            paint(dim_style(), format!("{frame} working (Ctrl+C to stop)"))
        );
        let _ = io::stdout().flush();
    }

    fn clear_working_line() {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
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

pub(crate) fn has_visible_assistant_text(content: &str) -> bool {
    !content.trim().is_empty()
}

/// Formats prompt footer data with the dim terminal style used by chat context rows.
pub(crate) fn format_prompt_footer(footer: &ChatPromptFooterModel) -> String {
    let view = UserPromptFooterView::from_model(footer);
    paint(dim_style(), format_user_prompt_footer(&view))
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
