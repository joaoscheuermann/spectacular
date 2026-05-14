use crate::ids::{SessionId, Timestamp};
use crate::transcript::TranscriptItem;
use serde::{Deserialize, Serialize};

/// Editable prompt state owned by the reducer rather than terminal input code.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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

/// Returns the initial session timestamp used when loading legacy snapshots.
fn default_next_timestamp() -> Timestamp {
    Timestamp::default()
}

/// Computes the next reducer timestamp from loaded durable transcript items.
fn next_timestamp_after(transcript: &[TranscriptItem]) -> Timestamp {
    transcript
        .iter()
        .map(|item| item.timestamp)
        .max()
        .map(Timestamp::next)
        .unwrap_or_default()
}

/// Minimal selection prompt state for modal pickers that will be rendered later.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Session {
    pub id: SessionId,
    pub transcript: Vec<TranscriptItem>,
    #[serde(skip_serializing)]
    pub next_timestamp: Timestamp,
    pub prompt: PromptState,
    pub usage: Option<crate::metadata::ContextTokenUsage>,
}

impl<'de> Deserialize<'de> for Session {
    /// Deserializes durable session data and reconstructs transient timestamp state.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DurableSession {
            id: SessionId,
            transcript: Vec<TranscriptItem>,
            prompt: PromptState,
            usage: Option<crate::metadata::ContextTokenUsage>,
        }

        let durable = DurableSession::deserialize(deserializer)?;
        let mut session = Self {
            id: durable.id,
            transcript: durable.transcript,
            next_timestamp: default_next_timestamp(),
            prompt: durable.prompt,
            usage: durable.usage,
        };
        session.refresh_next_timestamp();
        Ok(session)
    }
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

    /// Recalculates transient timestamp allocation state after snapshot loading.
    pub fn refresh_next_timestamp(&mut self) {
        self.next_timestamp = next_timestamp_after(&self.transcript);
    }

    /// Allocates the next transcript timestamp for deterministic reducer ordering.
    pub fn allocate_timestamp(&mut self) -> Timestamp {
        let timestamp = self.next_timestamp;
        self.next_timestamp = self.next_timestamp.next();
        timestamp
    }
}
