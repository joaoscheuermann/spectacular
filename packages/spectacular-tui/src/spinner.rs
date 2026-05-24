const DEFAULT_FRAMES: &[&str] = &[
    "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}", "\u{2807}",
    "\u{280f}",
];

/// Deterministic spinner state rendered from model data instead of terminal output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpinnerState {
    frame: usize,
}

impl SpinnerState {
    /// Creates a spinner positioned at its first frame.
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    /// Returns the currently visible spinner frame.
    pub fn current_frame(&self) -> &'static str {
        DEFAULT_FRAMES[self.frame]
    }

    /// Advances the spinner by one frame with wraparound.
    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % DEFAULT_FRAMES.len();
    }
}

impl Default for SpinnerState {
    /// Creates a default spinner at the first frame.
    fn default() -> Self {
        Self::new()
    }
}
