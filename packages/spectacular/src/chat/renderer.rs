mod banner;
mod directory;
mod footer;
mod json_preview;
mod reasoning;
mod style;
mod tool;
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
use anstyle::Style;
use banner::{format_opening_banner, OpeningBannerView};
use footer::{format_user_prompt_footer, UserPromptFooterView};
use reasoning::{
    format_reasoning_text, has_visible_reasoning_text as contains_visible_reasoning_text,
};
use spectacular_agent::AgentEvent;
use spectacular_agent::ToolStorage;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use style::{assistant_style, error_style, success_style, tool_style, warning_style};
pub(crate) use style::{dim_style, paint, selection_style, user_style};
pub use tool::{ToolCallView, ToolResultView};

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
        self.render_user_prompt(prompt, None);
    }

    /// Renders a newly submitted user prompt followed by its dim context footer.
    pub fn user_prompt_with_footer(&self, prompt: &str, footer: &ChatPromptFooterModel) {
        self.render_user_prompt(prompt, Some(footer));
    }

    /// Renders the interactive input prompt marker after interrupting the working line.
    pub fn prompt(&self) {
        self.interrupt_working_line();
        print!("{}", paint(user_style(), "> "));
    }

    /// Starts the default working indicator frame for an in-flight model response.
    pub fn working(&self) {
        self.working_frame(0);
    }

    /// Renders or records the current working indicator animation frame.
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

    /// Clears the current working indicator and marks it inactive.
    pub fn clear_working(&self) {
        self.reset_working();
        Self::clear_working_line();
    }

    /// Streams assistant text with typewriter pacing while pausing the working indicator.
    pub async fn assistant_delta(&self, content: &str) -> Result<(), ChatError> {
        self.stream_styled_delta(content, assistant_style()).await
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
    pub fn tool_call(&self, _tool_call_id: &str, name: &str, arguments: &str, tools: &ToolStorage) {
        self.render_tool_call(&ToolCallView::from_parts(name, arguments, tools));
    }

    /// Renders a preformatted tool call view without resolving tool metadata.
    pub fn render_tool_call(&self, view: &ToolCallView) {
        self.with_interrupted_working_line(|| {
            println!("{} {}", paint(tool_style(), &view.name), view.input);
        });
    }

    /// Renders a tool result using the registered tool display formatter when available.
    pub fn tool_result(&self, name: &str, content: &str, tools: &ToolStorage) {
        self.render_tool_result(&ToolResultView::from_parts(name, content, tools));
    }

    /// Renders a preformatted tool result view without resolving tool metadata.
    pub fn render_tool_result(&self, view: &ToolResultView) {
        let _ = &view.status;
        self.with_interrupted_working_line(|| {
            println!("{TOOL_RESULT_PREFIX} {}", view.output);
            println!();
        });
    }

    /// Streams visible model reasoning with typewriter pacing.
    pub async fn reasoning_delta(&self, content: &str) -> Result<(), ChatError> {
        self.stream_styled_delta(content, dim_style()).await
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

    /// Renders a successful status line while preserving working indicator state.
    pub fn success(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(success_style(), message));
        });
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

            let Some(event) = event.to_agent_event() else {
                continue;
            };

            if let AgentEvent::MessageDelta(delta) = &event {
                self.flush_reasoning(&mut reasoning_buffer);
                assistant_buffer.push_str(&delta.content);
                continue;
            }

            if let AgentEvent::ReasoningDelta(delta) = &event {
                self.flush_assistant(&mut assistant_buffer);
                reasoning_buffer.push_str(&delta.content);
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
        if !contains_visible_reasoning_text(buffer) {
            buffer.clear();
            return;
        }

        self.reasoning_text(buffer);
        buffer.clear();
    }

    /// Streams styled text with typewriter pacing while pausing the working indicator.
    async fn stream_styled_delta(&self, content: &str, style: Style) -> Result<(), ChatError> {
        let paused = self.pause_working();
        let result = async {
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
        .await;
        if paused {
            self.resume_working();
        }

        result
    }

    /// Renders a dim notice line while preserving working indicator state.
    fn notice(&self, message: impl Into<String>) {
        let content = message.into();
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), content));
        });
    }

    /// Locks the mutable working-line state, recovering poisoned locks for terminal output.
    fn working_state(&self) -> MutexGuard<'_, WorkingLineState> {
        self.working
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Marks the working indicator inactive and unpaused.
    fn reset_working(&self) {
        let mut state = self.working_state();
        state.active = false;
        state.pause_depth = 0;
    }

    /// Returns the current visible working frame when it can be rendered.
    fn renderable_working_frame(&self) -> Option<usize> {
        let state = self.working_state();
        if state.active && state.pause_depth == 0 {
            return Some(state.frame);
        }

        None
    }

    /// Temporarily clears the working line, runs a write, and restores the frame if still active.
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

    /// Clears the currently visible working line without changing active state.
    fn interrupt_working_line(&self) {
        if self.renderable_working_frame().is_some() {
            Self::clear_working_line();
        }
    }

    /// Pauses working indicator rendering while streamed output writes directly to stdout.
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

    /// Decrements the working pause depth so future frames can render when unpaused.
    fn resume_working(&self) {
        let mut state = self.working_state();
        if state.pause_depth > 0 {
            state.pause_depth -= 1;
        }
    }

    /// Writes one spinner frame in-place to the terminal working line.
    fn write_working_frame(frame: usize) {
        let frame = WORKING_FRAMES[frame % WORKING_FRAMES.len()];
        print!(
            "\r\x1b[2K{}",
            paint(dim_style(), format!("{frame} working (Ctrl+C to stop)"))
        );
        let _ = io::stdout().flush();
    }

    /// Clears the terminal line used by the working indicator.
    fn clear_working_line() {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }
}

/// Reports whether assistant content contains non-whitespace visible text.
pub(crate) fn has_visible_assistant_text(content: &str) -> bool {
    !content.trim().is_empty()
}

/// Reports whether reasoning content contains non-whitespace visible text.
pub(crate) fn has_visible_reasoning_text(content: &str) -> bool {
    contains_visible_reasoning_text(content)
}

/// Formats prompt footer data with the dim terminal style used by chat context rows.
pub(crate) fn format_prompt_footer(footer: &ChatPromptFooterModel) -> String {
    let view = UserPromptFooterView::from_model(footer);
    paint(dim_style(), format_user_prompt_footer(&view))
}
