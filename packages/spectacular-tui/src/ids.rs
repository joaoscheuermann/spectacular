/// Stable identifier for a chat session shown in the TUI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionId(String);

impl SessionId {
    /// Creates a session identifier from caller-owned runtime/session metadata.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the raw session identifier for display and persistence adapters.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identifier for one transcript item in the TUI model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptItemId(String);

impl TranscriptItemId {
    /// Creates a transcript item identifier from a runtime event identifier.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the raw transcript item identifier for event correlation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Monotonic reducer-owned timestamp for transcript ordering.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Creates a transcript timestamp from a caller-owned sequence value.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw ordering value for snapshots and tests.
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Returns the next timestamp using saturating arithmetic.
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
