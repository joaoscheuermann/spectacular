use crate::ids::{SessionId, Timestamp};
use crate::transcript::TranscriptItem;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Paste metadata carried with prompt state so paste handling remains reducer-visible.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptPasteBurstState {
    pub buffer: String,
}

/// Editable prompt state owned by the reducer rather than terminal input code.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptState {
    pub text: String,
    pub cursor: usize,
    #[serde(default)]
    pub selection_anchor: Option<usize>,
    #[serde(default)]
    pub selected_completion: usize,
    #[serde(default)]
    pub paste_burst: PromptPasteBurstState,
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
            selection_anchor: None,
            selected_completion: 0,
            paste_burst: PromptPasteBurstState::default(),
        }
    }

    /// Clears editable prompt content and resets cursor, selection, and paste metadata.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.selection_anchor = None;
        self.selected_completion = 0;
        self.paste_burst.buffer.clear();
    }

    /// Inserts normalized text at the cursor after replacing any active selection.
    pub fn insert_text(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }

        self.delete_selection();
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
        self.selected_completion = 0;
    }

    /// Inserts pasted text with CRLF/CR normalization tracked in prompt paste metadata.
    pub fn insert_paste(&mut self, value: &str) {
        let normalized = normalize_paste(value);
        self.paste_burst.buffer = normalized.clone();
        self.insert_text(&normalized);
    }

    /// Moves the cursor one character left and optionally extends selection state.
    pub fn move_left(&mut self, selecting: bool) {
        let cursor = previous_boundary(&self.text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor one character right and optionally extends selection state.
    pub fn move_right(&mut self, selecting: bool) {
        let cursor = next_boundary(&self.text, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Moves the cursor to the prompt start and optionally extends selection state.
    pub fn move_to_start(&mut self, selecting: bool) {
        self.move_to(0, selecting);
    }

    /// Moves the cursor to the prompt end and optionally extends selection state.
    pub fn move_to_end(&mut self, selecting: bool) {
        self.move_to(self.text.len(), selecting);
    }

    /// Returns the active selection range in byte offsets when text is selected.
    pub fn selection_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }

        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    /// Deletes the active selection and moves the cursor to the selection start.
    pub fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection_range() else {
            self.selection_anchor = None;
            return false;
        };

        self.text.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.selection_anchor = None;
        true
    }

    /// Deletes one character before the cursor or the active selected range.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }

        let previous = previous_boundary(&self.text, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.text.replace_range(previous..self.cursor, "");
        self.cursor = previous;
    }

    /// Deletes one character after the cursor or the active selected range.
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }

        let next = next_boundary(&self.text, self.cursor);
        if next == self.cursor {
            return;
        }

        self.text.replace_range(self.cursor..next, "");
    }

    /// Moves the cursor to a byte offset while preserving character-boundary safety.
    fn move_to(&mut self, cursor: usize, selecting: bool) {
        if selecting && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }
        if !selecting {
            self.selection_anchor = None;
        }
        self.cursor = clamp_boundary(&self.text, cursor);
    }
}

/// Normalizes terminal paste content to LF-only line breaks.
fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

/// Returns the nearest valid character boundary at or before an offset.
fn clamp_boundary(value: &str, offset: usize) -> usize {
    let mut cursor = offset.min(value.len());
    while cursor > 0 && !value.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

/// Returns the previous character boundary from the supplied cursor.
fn previous_boundary(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    if cursor == 0 {
        return 0;
    }

    value[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

/// Returns the next character boundary from the supplied cursor.
fn next_boundary(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    if cursor >= value.len() {
        return value.len();
    }

    value[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(value.len())
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
