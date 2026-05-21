use crate::ids::{SessionId, Timestamp};
use crate::prompt::normalize_paste;
use crate::transcript::TranscriptItem;
use serde::{Deserialize, Serialize};

/// Paste metadata carried with prompt state so paste handling remains reducer-visible.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptPasteBurstState {
    pub buffer: String,
}

/// One restorable prompt editing snapshot for undo and redo stacks.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptHistoryEntry {
    pub lines: Vec<String>,
    pub cursor: usize,
}

/// Logical-line textarea state owned by the reducer rather than terminal input code.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(from = "DurablePromptState")]
pub struct PromptState {
    pub lines: Vec<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub cursor: usize,
    #[serde(default)]
    pub preferred_column: Option<usize>,
    #[serde(default)]
    pub selection_anchor: Option<usize>,
    #[serde(default)]
    pub selected_completion: usize,
    #[serde(default)]
    pub kill_buffer: String,
    #[serde(default)]
    pub paste_burst: PromptPasteBurstState,
    #[serde(default)]
    pub scroll_top_row: usize,
    #[serde(default)]
    pub undo_stack: Vec<PromptHistoryEntry>,
    #[serde(default)]
    pub redo_stack: Vec<PromptHistoryEntry>,
    #[serde(default)]
    pub history_group: Option<PromptHistoryEntry>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DurablePromptState {
    Current {
        lines: Vec<String>,
        #[serde(default)]
        text: String,
        #[serde(default)]
        cursor: usize,
        #[serde(default)]
        preferred_column: Option<usize>,
        #[serde(default)]
        selection_anchor: Option<usize>,
        #[serde(default)]
        selected_completion: usize,
        #[serde(default)]
        kill_buffer: String,
        #[serde(default)]
        paste_burst: PromptPasteBurstState,
        #[serde(default)]
        scroll_top_row: usize,
        #[serde(default)]
        undo_stack: Vec<PromptHistoryEntry>,
        #[serde(default)]
        redo_stack: Vec<PromptHistoryEntry>,
        #[serde(default)]
        history_group: Option<PromptHistoryEntry>,
    },
    Legacy {
        #[serde(default)]
        text: String,
        #[serde(default)]
        cursor: usize,
        #[serde(default)]
        preferred_column: Option<usize>,
        #[serde(default)]
        selection_anchor: Option<usize>,
        #[serde(default)]
        selected_completion: usize,
        #[serde(default)]
        kill_buffer: String,
        #[serde(default)]
        paste_burst: PromptPasteBurstState,
    },
}

impl Default for PromptState {
    /// Creates a logical-line prompt with one empty editable line.
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            text: String::new(),
            cursor: 0,
            preferred_column: None,
            selection_anchor: None,
            selected_completion: 0,
            kill_buffer: String::new(),
            paste_burst: PromptPasteBurstState::default(),
            scroll_top_row: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            history_group: None,
        }
    }
}

impl From<DurablePromptState> for PromptState {
    /// Rehydrates current prompt state or migrates legacy single-string snapshots.
    fn from(value: DurablePromptState) -> Self {
        match value {
            DurablePromptState::Current {
                lines,
                text,
                cursor,
                preferred_column,
                selection_anchor,
                selected_completion,
                kill_buffer,
                paste_burst,
                scroll_top_row,
                undo_stack,
                redo_stack,
                history_group,
            } => {
                let text = normalize_paste(&text);
                let lines = if lines.is_empty() {
                    split_logical_lines(&text)
                } else {
                    normalize_lines(lines)
                };
                let text = lines.join("\n");
                Self {
                    lines,
                    text,
                    cursor,
                    preferred_column,
                    selection_anchor,
                    selected_completion,
                    kill_buffer,
                    paste_burst,
                    scroll_top_row,
                    undo_stack,
                    redo_stack,
                    history_group,
                }
            }
            DurablePromptState::Legacy {
                text,
                cursor,
                preferred_column,
                selection_anchor,
                selected_completion,
                kill_buffer,
                paste_burst,
            } => {
                let text = normalize_paste(&text);
                let lines = split_logical_lines(&text);
                let text = lines.join("\n");
                Self {
                    lines,
                    text,
                    cursor,
                    preferred_column,
                    selection_anchor,
                    selected_completion,
                    kill_buffer,
                    paste_burst,
                    ..Self::default()
                }
            }
        }
    }
}

/// Ensures deserialized prompt state always has at least one logical line.
fn non_empty_lines(lines: Vec<String>) -> Vec<String> {
    if lines.is_empty() {
        return vec![String::new()];
    }

    lines
}

/// Normalizes durable logical lines to the prompt's newline representation.
fn normalize_lines(lines: Vec<String>) -> Vec<String> {
    split_logical_lines(&normalize_paste(&non_empty_lines(lines).join("\n")))
}

/// Splits durable prompt text into logical lines while retaining trailing empty rows.
fn split_logical_lines(text: &str) -> Vec<String> {
    let lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
    non_empty_lines(lines)
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

/// Selection prompt state for modal option/custom/comment input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SelectionPromptState {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub options: Vec<String>,
    pub selected: usize,
    #[serde(default)]
    pub allow_custom: bool,
    #[serde(default)]
    pub allow_comment: bool,
    #[serde(default)]
    pub custom_input: String,
    #[serde(default)]
    pub custom_cursor: usize,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub comment_cursor: usize,
    #[serde(default)]
    pub input_mode: crate::prompt::SelectionInputMode,
}

/// Session-local TUI state for transcript, prompt, and token usage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Session {
    pub id: SessionId,
    pub transcript: Vec<TranscriptItem>,
    #[serde(skip_serializing)]
    pub next_timestamp: Timestamp,
    pub prompt: PromptState,
    #[serde(default, alias = "usage")]
    pub context_usage: Option<crate::metadata::ContextTokenUsage>,
    #[serde(default)]
    pub turn_usage: Option<crate::metadata::TurnTokenUsage>,
    #[serde(default)]
    pub total_usage: Option<crate::metadata::TokenUsageTotal>,
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
            #[serde(default, alias = "usage")]
            context_usage: Option<crate::metadata::ContextTokenUsage>,
            #[serde(default)]
            turn_usage: Option<crate::metadata::TurnTokenUsage>,
            #[serde(default)]
            total_usage: Option<crate::metadata::TokenUsageTotal>,
        }

        let durable = DurableSession::deserialize(deserializer)?;
        let mut session = Self {
            id: durable.id,
            transcript: durable.transcript,
            next_timestamp: default_next_timestamp(),
            prompt: durable.prompt,
            context_usage: durable.context_usage,
            turn_usage: durable.turn_usage,
            total_usage: durable.total_usage,
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
            context_usage: None,
            turn_usage: None,
            total_usage: None,
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
