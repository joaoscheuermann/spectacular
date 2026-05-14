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

/// Placeholder transcript item until semantic transcript variants are introduced.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptItem {
    pub id: TranscriptItemId,
    pub text: String,
}

impl TranscriptItem {
    /// Creates a minimal transcript item for tests and early adapter work.
    pub fn new(id: TranscriptItemId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
        }
    }
}

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
    pub prompt: PromptState,
    pub usage: Option<crate::metadata::ContextTokenUsage>,
}

impl Session {
    /// Creates an empty session model for the supplied session identifier.
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            transcript: Vec::new(),
            prompt: PromptState::empty(),
            usage: None,
        }
    }
}
