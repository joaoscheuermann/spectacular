use crate::ids::{SessionId, Timestamp};
use crate::transcript::TranscriptItem;

/// Editable prompt state owned by the reducer rather than terminal input code.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromptState {
    pub text: String,
    pub cursor: usize,
}

impl PromptState {
    /// Creates an empty prompt with the cursor at the start of the buffer.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a prompt from text with the cursor placed at the end.
    pub fn from_text(value: impl Into<String>) -> Self {
        let text = value.into();
        Self {
            cursor: text.len(),
            text,
        }
    }

    /// Clears editable prompt content and resets the cursor.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }
}

/// Minimal selection prompt state for modal pickers that will be rendered later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPromptState {
    pub title: String,
    pub options: Vec<String>,
    pub selected: usize,
}

impl SelectionPromptState {
    /// Creates a selection prompt with the first option selected.
    pub fn new(title: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            title: title.into(),
            options,
            selected: 0,
        }
    }
}

/// Session-local TUI state for transcript, prompt, and context usage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub transcript: Vec<TranscriptItem>,
    pub next_timestamp: Timestamp,
    pub prompt: PromptState,
    pub usage: Option<crate::metadata::ContextTokenUsage>,
}

impl Session {
    /// Creates an empty session model for the supplied session identifier.
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            transcript: Vec::new(),
            next_timestamp: Timestamp::default(),
            prompt: PromptState::empty(),
            usage: None,
        }
    }

    /// Allocates the next transcript timestamp for deterministic reducer ordering.
    pub fn allocate_timestamp(&mut self) -> Timestamp {
        let timestamp = self.next_timestamp;
        self.next_timestamp = self.next_timestamp.next();
        timestamp
    }
}
