mod banner;
mod directory;
mod footer;
mod json_preview;
mod reasoning;
mod style;
mod terminal_output;
mod token_usage;
mod tool;
mod working_line;
#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/renderer.rs"
    ));
}

use crate::chat::command_event::{CommandEvent, CommandStatus};
use crate::chat::model::HistoryTableModel;
use crate::chat::runner::render_agent_event;
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
use anstyle::Style;
use banner::{format_opening_banner, OpeningBannerView};
use reasoning::format_reasoning_text;
pub(crate) use reasoning::has_visible_reasoning_text;
use serde_json::Value;
use spectacular_agent::ToolStorage;
use spectacular_agent::AgentEvent;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use style::{assistant_style, command_style, error_style, success_style, warning_style};
pub(crate) use style::{
    command_output_style, dim_style, paint, selection_style, title_style, user_style,
};
pub(crate) use terminal_output::{
    format_prompt_footer, has_visible_assistant_text, styled_tool_output_lines, ToolOutputLineStyle,
};
use terminal_output::{format_tool_call_view, print_tool_output};
pub use tool::{ToolCallView, ToolResultView, ToolStatus};
use working_line::WorkingLineState;

use super::RuntimeSelection;

/// Number of visible characters the terminal typewriter writes per tick while
/// an assistant delta is streaming. Combined with the 50 ms tick this paces
/// revealed text at ~600 chars/sec. Each provider delta is drained before the
/// renderer handles the next event, so no backlog is retained after the run
/// finishes or fails.
pub(crate) const TYPEWRITER_CHARS_PER_TICK: usize = 30;
const TYPEWRITER_TICK_INTERVAL: Duration = Duration::from_millis(50);
const OPENING_BANNER_MIN_WIDTH: usize = 52;
const WORKING_FRAMES: &[&str] = &["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Default)]
pub struct Renderer {
    working: Mutex<WorkingLineState>,
    tool_call_arguments: Mutex<BTreeMap<String, Value>>,
}

impl Renderer {
    /// Renders the opening session banner for a newly created chat session.
    pub fn session_created(&self, id: &str, runtime: &RuntimeSelection, directory: &Path) {
        let banner = OpeningBannerView::from_runtime(id, runtime, directory);
        self.with_interrupted_working_line(|| {
            println!("{}", format_opening_banner(&banner));
            println!();
        });
    }

    /// Renders a short notice that an existing session was resumed.
    pub fn resumed(&self, id: &str) {
        self.notice(format!("resumed session {id}"));
    }

    /// Clears the terminal viewport and resets any active working indicator state.
    pub fn clear_screen(&self) {
        self.reset_working();
        print!("\x1b[2J\x1b[3J\x1b[H");
        let _ = io::stdout().flush();
    }

    /// Renders a submitted user prompt without contextual footer metadata.
    pub fn user_prompt(&self, prompt: &str) {
        self.render_user_prompt(prompt);
    }

    /// Renders the interactive input prompt marker after interrupting the working line.
    pub fn prompt(&self) {
        self.interrupt_working_line();
        print!("{}", paint(user_style(), "> "));
    }

    /// Streams assistant text with typewriter pacing while the runner owns working-line pauses.
    pub async fn assistant_delta(&self, content: &str) -> Result<(), ChatError> {
        self.write_styled_delta(content, assistant_style()).await
    }

    /// Renders a complete assistant response block when replaying persisted records.
    pub fn assistant_text(&self, content: &str) {
        if !has_visible_assistant_text(content) {
            return;
        }

        self.with_interrupted_working_line(|| {
            println!("{}", paint(assistant_style(), content));
            println!();
        });
    }

    /// Renders a tool call request using the registered tool display formatter when available.
    pub fn tool_call(&self, tool_call_id: &str, name: &str, arguments: &str, tools: &ToolStorage) {
        self.remember_tool_call_arguments(tool_call_id, arguments);
        self.render_tool_call(&ToolCallView::from_parts(name, arguments, tools));
    }

    /// Renders a preformatted tool call view without resolving tool metadata.
    pub fn render_tool_call(&self, view: &ToolCallView) {
        self.with_interrupted_working_line(|| {
            println!("{}", format_tool_call_view(view));
        });
    }

    /// Renders a tool result using the registered tool display formatter when available.
    pub fn tool_result(&self, tool_call_id: &str, name: &str, content: &str, tools: &ToolStorage) {
        let arguments = self.take_tool_call_arguments(tool_call_id);
        self.render_tool_result(&ToolResultView::from_parts_with_arguments(
            name,
            content,
            tools,
            arguments.as_ref(),
        ));
    }

    /// Renders a preformatted tool result view without resolving tool metadata.
    pub fn render_tool_result(&self, view: &ToolResultView) {
        let _ = &view.status;
        self.with_interrupted_working_line(|| {
            print_tool_output(&view.output);
            println!();
        });
    }

    /// Streams visible model reasoning with typewriter pacing while preserving cursor ownership.
    pub async fn reasoning_delta(&self, content: &str) -> Result<(), ChatError> {
        self.write_styled_delta(content, dim_style()).await
    }

    /// Renders a complete reasoning block when replaying persisted records.
    pub fn reasoning_text(&self, content: &str) {
        let Some(output) = format_reasoning_text(content) else {
            return;
        };

        self.with_interrupted_working_line(|| {
            println!("{output}");
            println!();
        });
    }

    /// Renders an error message with error styling and spacing.
    pub fn error(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(error_style(), format!("error: {message}")));
            println!();
        });
    }

    /// Renders any displayable command error through the standard error path.
    pub fn command_error(&self, error: &impl std::fmt::Display) {
        self.error(&error.to_string());
    }

    /// Renders a warning message while preserving working indicator state.
    pub fn warning(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(warning_style(), format!("warning: {message}")));
        });
    }

    /// Renders a cancellation reason with warning styling and block spacing.
    pub fn cancelled(&self, reason: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(warning_style(), reason));
            println!();
        });
    }

    /// Renders a dim secondary status line while preserving working indicator state.
    pub fn dim(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), message));
        });
    }

    /// Renders streamed-response spacing while preserving any active working indicator.
    pub fn response_spacer(&self) {
        self.with_interrupted_working_line(|| {
            println!("\n");
        });
    }

    /// Renders a successful status line while preserving working indicator state.
    pub fn success(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(success_style(), message));
        });
    }

    /// Renders a blank line while preserving working indicator state.
    pub fn blank_line(&self) {
        self.with_interrupted_working_line(|| {
            println!();
        });
    }

    /// Renders a command lifecycle start record.
    pub fn command_start(&self, _title: &str, command: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(command_style(), command));
        });
    }

    /// Renders a command lifecycle progress record.
    pub fn command_delta(&self, content: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(command_output_style(), format!("• {content}")));
        });
    }

    /// Renders a command lifecycle completion record.
    pub fn command_finished(&self, status: CommandStatus, summary: &str) {
        match status {
            CommandStatus::Success => self.success(summary),
            CommandStatus::Failed | CommandStatus::Error => self.error(summary),
            CommandStatus::Cancelled | CommandStatus::TimedOut => self.warning(summary),
        }
    }

    /// Replays persisted chat records with assistant deltas coalesced into readable blocks.
    pub async fn render_records(
        &self,
        records: &[ChatRecord],
        tools: &ToolStorage,
    ) -> Result<(), ChatError> {
        let mut assistant_buffer = String::new();
        let mut reasoning_buffer = String::new();
        for record in records {
            if matches!(record, ChatRecord::Corrupt { .. }) {
                self.flush_assistant(&mut assistant_buffer);
                self.flush_reasoning(&mut reasoning_buffer);
                self.warning(&format!(
                    "unreadable session event at line {}",
                    record.line()
                ));
                continue;
            }

            let Some(event) = record.event() else {
                self.flush_assistant(&mut assistant_buffer);
                self.flush_reasoning(&mut reasoning_buffer);
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

            if let Some(command_event) = event.to_command_event() {
                self.flush_assistant(&mut assistant_buffer);
                self.flush_reasoning(&mut reasoning_buffer);
                self.render_command_event(&command_event);
                continue;
            }

            let Some(event) = event.to_agent_event() else {
                continue;
            };

            if let AgentEvent::MessageDelta { content, .. } = &event {
                self.flush_reasoning(&mut reasoning_buffer);
                assistant_buffer.push_str(content);
                continue;
            }

            if let AgentEvent::ReasoningDelta { content, .. } = &event {
                self.flush_assistant(&mut assistant_buffer);
                reasoning_buffer.push_str(content);
                continue;
            }

            self.flush_assistant(&mut assistant_buffer);
            self.flush_reasoning(&mut reasoning_buffer);
            render_agent_event(self, tools, &event).await?;
        }

        self.flush_assistant(&mut assistant_buffer);
        self.flush_reasoning(&mut reasoning_buffer);
        Ok(())
    }

    /// Renders a replayed app-owned command lifecycle event.
    fn render_command_event(&self, event: &CommandEvent) {
        match event {
            CommandEvent::Start(start) => {
                self.clear_working();
                self.command_start(&start.title, &start.command);
                self.working();
            }
            CommandEvent::Delta(delta) => {
                self.clear_working();
                self.command_delta(&delta.content);
                self.working();
            }
            CommandEvent::Finished(finished) => {
                self.clear_working();
                self.command_finished(finished.status, &finished.summary);
                self.working();
            }
        }
    }

    /// Renders the session history table and any remaining-session count.
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

    /// Renders a user prompt while preserving working-line state.
    fn render_user_prompt(&self, prompt: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(user_style(), prompt));
            println!();
        });
    }

    /// Flushes accumulated assistant replay text and clears the caller-owned buffer.
    fn flush_assistant(&self, buffer: &mut String) {
        if !has_visible_assistant_text(buffer) {
            buffer.clear();
            return;
        }

        self.assistant_text(buffer);
        buffer.clear();
    }

    /// Flushes accumulated reasoning replay text and clears the caller-owned buffer.
    fn flush_reasoning(&self, buffer: &mut String) {
        if !has_visible_reasoning_text(buffer) {
            buffer.clear();
            return;
        }

        self.reasoning_text(buffer);
        buffer.clear();
    }

    /// Writes styled stream chunks while caller-controlled pauses protect response text.
    async fn write_styled_delta(&self, content: &str, style: Style) -> Result<(), ChatError> {
        let mut characters = content.chars().peekable();
        while characters.peek().is_some() {
            let chunk = characters
                .by_ref()
                .take(TYPEWRITER_CHARS_PER_TICK)
                .collect::<String>();
            print!("{}", paint(style, chunk));
            io::stdout().flush().map_err(ChatError::Io)?;
            if characters.peek().is_some() {
                tokio::time::sleep(TYPEWRITER_TICK_INTERVAL).await;
            }
        }

        Ok(())
    }

    /// Renders a dim notice line while preserving working indicator state.
    fn notice(&self, message: impl Into<String>) {
        let content = message.into();
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), content));
        });
    }

    /// Caches parsed tool-call arguments so result renderers can include input context.
    fn remember_tool_call_arguments(&self, tool_call_id: &str, arguments: &str) {
        let Ok(arguments) = serde_json::from_str::<Value>(arguments) else {
            return;
        };

        self.tool_call_arguments
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(tool_call_id.to_owned(), arguments);
    }

    /// Removes and returns cached arguments for a completed tool call.
    fn take_tool_call_arguments(&self, tool_call_id: &str) -> Option<Value> {
        self.tool_call_arguments
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(tool_call_id)
    }
}
