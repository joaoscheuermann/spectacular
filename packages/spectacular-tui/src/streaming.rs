use crate::ids::TranscriptItemId;
use serde::{Deserialize, Serialize};

pub const ASSISTANT_REVEAL_CHARS_PER_TICK: usize = 30;
pub const ASSISTANT_REVEAL_TICK_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(50);

/// Reducer-owned assistant stream reveal state for typewriter-paced display.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssistantStreamState {
    pub active_id: Option<TranscriptItemId>,
    pub streams: Vec<AssistantRevealState>,
}

impl AssistantStreamState {
    /// Starts or resets reveal bookkeeping for one assistant transcript item.
    pub fn start(&mut self, id: TranscriptItemId) {
        if let Some(stream) = self.find_mut(&id) {
            stream.received.clear();
            stream.revealed_chars = 0;
            stream.finished = false;
            self.active_id = Some(id);
            return;
        }

        self.streams.push(AssistantRevealState::new(id.clone()));
        self.active_id = Some(id);
    }

    /// Appends newly received assistant text without revealing it immediately.
    pub fn append_delta(&mut self, id: &TranscriptItemId, text: &str) {
        let Some(stream) = self.find_mut(id) else {
            return;
        };
        stream.received.push_str(text);
    }

    /// Marks the stream complete while preserving any unrevealed received text.
    pub fn finish(&mut self, id: &TranscriptItemId) {
        let Some(stream) = self.find_mut(id) else {
            return;
        };
        stream.finished = true;
        if self.active_id.as_ref() == Some(id) {
            self.active_id = None;
        }
    }

    /// Reveals up to the configured typewriter chunk size and returns visible text.
    pub fn reveal_tick(&mut self, id: &TranscriptItemId) -> Option<String> {
        let stream = self.find_mut(id)?;
        stream.reveal_next_chunk();
        Some(stream.visible_text())
    }

    /// Returns whether reveal bookkeeping exists for the supplied item.
    pub fn contains(&self, id: &TranscriptItemId) -> bool {
        self.find(id).is_some()
    }

    /// Returns the stream that has queued text ready for the next reveal tick.
    pub fn active_reveal_id(&self) -> Option<TranscriptItemId> {
        self.streams
            .iter()
            .find(|stream| stream.has_queued_text())
            .map(|stream| stream.id.clone())
    }

    /// Removes reveal bookkeeping for the supplied item.
    pub fn remove(&mut self, id: &TranscriptItemId) {
        self.streams.retain(|stream| stream.id != *id);
        if self.active_id.as_ref() == Some(id) {
            self.active_id = None;
        }
    }

    /// Finds immutable reveal state by transcript item id.
    fn find(&self, id: &TranscriptItemId) -> Option<&AssistantRevealState> {
        self.streams.iter().find(|stream| stream.id == *id)
    }

    /// Finds mutable reveal state by transcript item id.
    fn find_mut(&mut self, id: &TranscriptItemId) -> Option<&mut AssistantRevealState> {
        self.streams.iter_mut().find(|stream| stream.id == *id)
    }
}

/// Typewriter reveal bookkeeping for one assistant transcript item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AssistantRevealState {
    pub id: TranscriptItemId,
    pub received: String,
    pub revealed_chars: usize,
    pub finished: bool,
}

impl AssistantRevealState {
    /// Creates empty reveal state for one assistant item.
    pub fn new(id: TranscriptItemId) -> Self {
        Self {
            id,
            received: String::new(),
            revealed_chars: 0,
            finished: false,
        }
    }

    /// Advances visible text by the original renderer's character chunk size.
    pub fn reveal_next_chunk(&mut self) {
        let received_chars = self.received.chars().count();
        self.revealed_chars = self
            .revealed_chars
            .saturating_add(ASSISTANT_REVEAL_CHARS_PER_TICK)
            .min(received_chars);
    }

    /// Returns whether this stream has received text hidden from display.
    pub fn has_queued_text(&self) -> bool {
        self.revealed_chars < self.received.chars().count()
    }

    /// Returns visible text without splitting a UTF-8 character boundary.
    pub fn visible_text(&self) -> String {
        self.received.chars().take(self.revealed_chars).collect()
    }
}
