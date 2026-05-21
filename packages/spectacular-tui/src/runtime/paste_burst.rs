use std::time::{Duration, Instant};

const PASTE_ENTER_SUPPRESS_WINDOW: Duration = Duration::from_millis(120);

#[cfg(not(windows))]
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(8);
#[cfg(windows)]
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(30);

/// Tracks fast key streams that terminals emit when bracketed paste is unavailable.
#[derive(Default)]
pub(crate) struct PasteBurst {
    last_text_key: Option<Instant>,
    suppress_enter_until: Option<Instant>,
}

impl PasteBurst {
    /// Records an inserted text character and opens a short newline suppression window.
    pub(crate) fn note_text_key(&mut self, now: Instant) {
        let fast = self
            .last_text_key
            .is_some_and(|last| now.duration_since(last) <= PASTE_BURST_CHAR_INTERVAL);
        self.last_text_key = Some(now);
        self.suppress_enter_until = Some(now + newline_window(fast));
    }

    /// Records a line break that was treated as pasted text and keeps the burst active.
    pub(crate) fn note_line_break(&mut self, now: Instant) {
        self.last_text_key = Some(now);
        self.suppress_enter_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    /// Returns true when an Enter-like key should be text, not prompt submission.
    pub(crate) fn should_insert_line_break(&self, now: Instant) -> bool {
        self.suppress_enter_until.is_some_and(|until| now <= until)
    }

    /// Clears burst state after explicit paste or non-text editing keys.
    pub(crate) fn clear(&mut self) {
        self.last_text_key = None;
        self.suppress_enter_until = None;
    }
}

/// Keeps single-key paste cases protected briefly while extending confirmed bursts longer.
fn newline_window(fast: bool) -> Duration {
    if fast {
        return PASTE_ENTER_SUPPRESS_WINDOW;
    }

    PASTE_BURST_CHAR_INTERVAL
}
