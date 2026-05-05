use std::time::{Duration, Instant};

const PASTE_ENTER_SUPPRESS_WINDOW: Duration = Duration::from_millis(120);

#[cfg(not(windows))]
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(8);
#[cfg(windows)]
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(30);

#[cfg(not(windows))]
const PASTE_BURST_ACTIVE_IDLE_TIMEOUT: Duration = Duration::from_millis(8);
#[cfg(windows)]
const PASTE_BURST_ACTIVE_IDLE_TIMEOUT: Duration = Duration::from_millis(60);

#[derive(Default)]
pub(crate) struct PasteBurst {
    last_plain_char_time: Option<Instant>,
    burst_window_until: Option<Instant>,
    buffer: String,
    active: bool,
    pending_first_char: Option<(char, Instant)>,
}

pub(crate) enum CharDecision {
    Buffered,
    Held,
}

pub(crate) enum FlushResult {
    Paste(String),
    Typed(char),
    None,
}

impl PasteBurst {
    pub(crate) fn recommended_flush_delay() -> Duration {
        PASTE_BURST_CHAR_INTERVAL + Duration::from_millis(1)
    }

    pub(crate) fn recommended_active_flush_delay() -> Duration {
        PASTE_BURST_ACTIVE_IDLE_TIMEOUT + Duration::from_millis(1)
    }

    pub(crate) fn poll_delay(&self) -> Duration {
        if self.is_active_internal() {
            return Self::recommended_active_flush_delay();
        }

        Self::recommended_flush_delay()
    }

    pub(crate) fn on_plain_char(&mut self, character: char, now: Instant) -> CharDecision {
        if self.is_active_internal() {
            self.note_plain_char(now);
            self.buffer.push(character);
            self.extend_window(now);
            return CharDecision::Buffered;
        }

        if let Some((held, held_at)) = self.pending_first_char {
            if now.duration_since(held_at) <= PASTE_BURST_CHAR_INTERVAL {
                self.note_plain_char(now);
                self.pending_first_char = None;
                self.active = true;
                self.buffer.push(held);
                self.buffer.push(character);
                self.extend_window(now);
                return CharDecision::Buffered;
            }
        }

        self.note_plain_char(now);
        self.pending_first_char = Some((character, now));
        CharDecision::Held
    }

    pub(crate) fn flush_if_due(&mut self, now: Instant) -> FlushResult {
        let Some(last_plain_char_time) = self.last_plain_char_time else {
            return FlushResult::None;
        };

        let timeout = if self.is_active_internal() {
            PASTE_BURST_ACTIVE_IDLE_TIMEOUT
        } else {
            PASTE_BURST_CHAR_INTERVAL
        };

        if now.duration_since(last_plain_char_time) <= timeout {
            return FlushResult::None;
        }

        if self.is_active_internal() {
            self.active = false;
            self.materialize_pending_first_char();
            return FlushResult::Paste(std::mem::take(&mut self.buffer));
        }

        if let Some((character, _)) = self.pending_first_char.take() {
            self.last_plain_char_time = None;
            return FlushResult::Typed(character);
        }

        FlushResult::None
    }

    pub(crate) fn append_newline_if_active(&mut self, now: Instant) -> bool {
        if !self.is_active() {
            return false;
        }

        self.materialize_pending_first_char();
        self.active = true;
        self.buffer.push('\n');
        self.extend_window(now);
        true
    }

    pub(crate) fn newline_should_insert_instead_of_submit(&self, now: Instant) -> bool {
        let in_burst_window = self.burst_window_until.is_some_and(|until| now <= until);
        self.is_active_internal() || in_burst_window
    }

    pub(crate) fn extend_window(&mut self, now: Instant) {
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    pub(crate) fn flush_before_modified_input(&mut self) -> Option<String> {
        if !self.is_active() {
            return None;
        }

        self.active = false;
        self.materialize_pending_first_char();
        self.last_plain_char_time = None;
        Some(std::mem::take(&mut self.buffer))
    }

    pub(crate) fn clear_window_after_non_char(&mut self) {
        self.last_plain_char_time = None;
        self.burst_window_until = None;
        self.buffer.clear();
        self.active = false;
        self.pending_first_char = None;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.is_active_internal() || self.pending_first_char.is_some()
    }

    pub(crate) fn starts_with(&self, character: char) -> bool {
        self.buffer
            .chars()
            .next()
            .or_else(|| self.pending_first_char.map(|(held, _)| held))
            == Some(character)
    }

    pub(crate) fn clear_after_explicit_paste(&mut self) {
        self.last_plain_char_time = None;
        self.burst_window_until = None;
        self.buffer.clear();
        self.active = false;
        self.pending_first_char = None;
    }

    fn is_active_internal(&self) -> bool {
        self.active || !self.buffer.is_empty()
    }

    fn materialize_pending_first_char(&mut self) {
        if let Some((character, _)) = self.pending_first_char.take() {
            self.buffer.push(character);
        }
    }

    fn note_plain_char(&mut self, now: Instant) {
        self.last_plain_char_time = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_plain_char_is_held_then_flushed_as_typed() {
        let mut burst = PasteBurst::default();
        let now = Instant::now();

        assert!(matches!(burst.on_plain_char('a', now), CharDecision::Held));

        let flush_at = now + PasteBurst::recommended_flush_delay();
        assert!(matches!(
            burst.flush_if_due(flush_at),
            FlushResult::Typed('a')
        ));
        assert!(!burst.is_active());
    }

    #[test]
    fn two_fast_plain_chars_start_buffered_paste() {
        let mut burst = PasteBurst::default();
        let now = Instant::now();

        assert!(matches!(burst.on_plain_char('a', now), CharDecision::Held));
        assert!(matches!(
            burst.on_plain_char('b', now + Duration::from_millis(1)),
            CharDecision::Buffered
        ));

        let flush_at =
            now + Duration::from_millis(1) + PasteBurst::recommended_active_flush_delay();
        assert!(matches!(
            burst.flush_if_due(flush_at),
            FlushResult::Paste(ref pasted) if pasted == "ab"
        ));
    }

    #[test]
    fn newline_materializes_pending_first_char_before_the_line_break() {
        let mut burst = PasteBurst::default();
        let now = Instant::now();

        assert!(matches!(burst.on_plain_char('a', now), CharDecision::Held));
        assert!(burst.append_newline_if_active(now + Duration::from_millis(1)));

        let flush_at =
            now + Duration::from_millis(1) + PasteBurst::recommended_active_flush_delay();
        assert!(matches!(
            burst.flush_if_due(flush_at),
            FlushResult::Paste(ref pasted) if pasted == "a\n"
        ));
    }
}
