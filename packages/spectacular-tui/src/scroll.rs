/// Scroll state for transcript viewport positioning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptScrollState {
    pub offset: u32,
    pub follow_tail: bool,
    pub visible_rows: u32,
}

impl TranscriptScrollState {
    /// Creates scroll state that follows the transcript tail.
    pub fn follow_tail() -> Self {
        Self {
            offset: 0,
            follow_tail: true,
            visible_rows: 0,
        }
    }

    /// Applies a relative scroll delta where positive moves toward older content.
    pub fn scroll_by(&mut self, delta: i32) {
        if delta > 0 {
            self.offset = self.offset.saturating_add(delta.unsigned_abs());
            self.follow_tail = false;
            return;
        }

        if delta < 0 {
            self.offset = self.offset.saturating_sub(delta.unsigned_abs());
            self.follow_tail = self.offset == 0;
        }
    }
}

impl Default for TranscriptScrollState {
    /// Creates default transcript scroll state following the tail.
    fn default() -> Self {
        Self::follow_tail()
    }
}
