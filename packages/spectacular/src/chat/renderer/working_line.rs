use super::{Renderer, WORKING_FRAMES, dim_style, paint};
use std::io::{self, Write};
use std::sync::MutexGuard;
use std::time::Duration;

#[derive(Default)]
pub(super) struct WorkingLineState {
    pub(super) active: bool,
    pub(super) frame: usize,
    pub(super) turn_tokens: Option<u64>,
    pub(super) pause_depth: usize,
}

impl Renderer {
    /// Starts the default working indicator frame for an in-flight model response.
    pub fn working(&self) {
        self.working_frame(0, None);
    }

    /// Renders or records the current working indicator animation frame.
    pub fn working_frame(&self, frame: usize, turn_tokens: Option<u64>) {
        let should_render = {
            let mut state = self.working_state();
            state.active = true;
            state.frame = frame;
            state.turn_tokens = turn_tokens;
            state.pause_depth == 0
        };

        if !should_render {
            return;
        }

        Self::write_inline_working_frame(frame, turn_tokens);
    }

    /// Clears the current working indicator when visible and marks it inactive.
    pub fn clear_working(&self) {
        if self.reset_working() {
            Self::clear_inline_working_line();
        }
    }

    /// Renders a completed response timing and token summary line.
    pub fn worked(&self, duration: Duration, turn_tokens: Option<u64>) {
        println!(
            "{}",
            paint(dim_style(), format_worked_line(duration, turn_tokens))
        );
    }

    /// Returns the current visible working frame when it can be rendered.
    pub(super) fn renderable_working_frame(&self) -> Option<(usize, Option<u64>)> {
        let state = self.working_state();
        if state.active && state.pause_depth == 0 {
            return Some((state.frame, state.turn_tokens));
        }

        None
    }

    /// Temporarily clears the working line, runs a write, and restores the frame if still active.
    pub(super) fn with_interrupted_working_line(&self, write: impl FnOnce()) {
        let frame = self.renderable_working_frame();
        if frame.is_some() {
            Self::clear_inline_working_line();
        }

        write();

        if let Some((frame, turn_tokens)) = frame {
            if self.renderable_working_frame().is_some() {
                Self::write_inline_working_frame(frame, turn_tokens);
            }
        }
    }

    /// Hides the working indicator while streamed content owns the terminal cursor.
    pub fn pause_working_line(&self) {
        let should_clear = {
            let mut state = self.working_state();
            let should_clear = state.active && state.pause_depth == 0;
            state.pause_depth = state.pause_depth.saturating_add(1);
            should_clear
        };

        if should_clear {
            Self::clear_inline_working_line();
        }
    }

    /// Restores the working indicator after streamed content closes, if it is still active.
    pub fn resume_working_line(&self) {
        let Some((frame, turn_tokens)) = self.resume_working_frame() else {
            return;
        };

        Self::write_inline_working_frame(frame, turn_tokens);
    }

    /// Clears the currently visible working line without changing active state.
    pub(super) fn interrupt_working_line(&self) {
        if self.renderable_working_frame().is_some() {
            Self::clear_inline_working_line();
        }
    }

    /// Locks the mutable working-line state, recovering poisoned locks for terminal output.
    fn working_state(&self) -> MutexGuard<'_, WorkingLineState> {
        self.working
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Marks the working indicator inactive and returns whether it was visible.
    pub(super) fn reset_working(&self) -> bool {
        let mut state = self.working_state();
        let was_visible = state.active && state.pause_depth == 0;
        state.active = false;
        state.pause_depth = 0;
        was_visible
    }

    /// Decrements pause depth and returns the frame that should be rendered again.
    fn resume_working_frame(&self) -> Option<(usize, Option<u64>)> {
        let mut state = self.working_state();
        if state.pause_depth == 0 {
            return None;
        }

        state.pause_depth -= 1;
        if !state.active || state.pause_depth > 0 {
            return None;
        }

        Some((state.frame, state.turn_tokens))
    }

    /// Writes one spinner frame in-place to the terminal working line.
    fn write_inline_working_frame(frame: usize, turn_tokens: Option<u64>) {
        let frame = WORKING_FRAMES[frame % WORKING_FRAMES.len()];
        print!(
            "\r\x1b[2K{}",
            paint(dim_style(), format_working_line(frame, turn_tokens))
        );
        let _ = io::stdout().flush();
    }

    /// Clears the terminal line used by the inline working indicator.
    fn clear_inline_working_line() {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }
}

/// Formats the working spinner text with an optional assistant-turn token count.
pub(super) fn format_working_line(frame: &str, turn_tokens: Option<u64>) -> String {
    match turn_tokens {
        Some(turn_tokens) => format!("{frame} Working (CTRL + C to stop · {turn_tokens} tokens)"),
        None => format!("{frame} Working (CTRL + C to stop)"),
    }
}

/// Formats the completed working summary line.
pub(super) fn format_worked_line(duration: Duration, turn_tokens: Option<u64>) -> String {
    format!(
        "Worked for {} · total {} tokens",
        format_worked_duration(duration),
        turn_tokens.unwrap_or(0)
    )
}

/// Formats elapsed response time for compact terminal output.
fn format_worked_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{minutes}m {remaining_seconds}s")
}
