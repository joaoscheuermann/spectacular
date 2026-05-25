use crate::chat::session::{
    ChatEvent, ChatRecord, HistoryPage, HistoryQuery, HistorySummary, UNTITLED,
};
use crate::chat::ChatError;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub struct SessionIndex<'a> {
    dir: &'a Path,
}

impl<'a> SessionIndex<'a> {
    pub fn new(dir: &'a Path) -> Self {
        Self { dir }
    }

    pub fn history(
        &self,
        query: HistoryQuery,
        read: impl Fn(&Path) -> Result<Vec<ChatRecord>, ChatError>,
    ) -> Result<HistoryPage, ChatError> {
        let mut sessions = Vec::new();
        for entry in
            fs::read_dir(self.dir).map_err(|error| ChatError::Session(error.to_string()))?
        {
            let entry = entry.map_err(|error| ChatError::Session(error.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let records = read(&path)?;
            let summary = summarize(id, &records);
            if summary.messages == 0 {
                continue;
            }
            sessions.push(summary);
        }

        sessions.sort_by(|left, right| right.updated.cmp(&left.updated));
        Ok(HistoryPage::from_sessions(sessions, query))
    }

    pub fn matching_ids(&self, prefix: &str) -> Result<Vec<String>, ChatError> {
        let mut matches = Vec::new();
        for entry in
            fs::read_dir(self.dir).map_err(|error| ChatError::Session(error.to_string()))?
        {
            let entry = entry.map_err(|error| ChatError::Session(error.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
            else {
                continue;
            };
            if id.starts_with(prefix) {
                matches.push(id);
            }
        }
        matches.sort();
        Ok(matches)
    }
}

fn summarize(id: &str, records: &[ChatRecord]) -> HistorySummary {
    let mut draft = SummaryDraft::new(id);
    for record in records {
        draft.record(record);
    }

    draft.finish()
}

struct SummaryDraft {
    id: String,
    updated: DateTime<Utc>,
    title: String,
    messages: usize,
    corrupt: bool,
    in_assistant: bool,
}

impl SummaryDraft {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            updated: DateTime::<Utc>::from(UNIX_EPOCH),
            title: UNTITLED.to_owned(),
            messages: 0,
            corrupt: false,
            in_assistant: false,
        }
    }

    fn record(&mut self, record: &ChatRecord) {
        self.corrupt |= record.is_corrupt_or_unknown();
        if let Some(event) = record.event() {
            self.event(event);
        }
    }

    fn event(&mut self, event: &ChatEvent) {
        if let Some(created_at) = event.created_at() {
            self.updated = self.updated.max(created_at);
        }

        match event {
            ChatEvent::SessionTitleUpdated { title, .. } => self.title = title.clone(),
            ChatEvent::UserPrompt { .. } => self.user_prompt(),
            ChatEvent::AssistantDelta { .. } => self.assistant_delta(),
            ChatEvent::Finished { .. } | ChatEvent::Error { .. } | ChatEvent::Cancelled { .. } => {
                self.in_assistant = false;
            }
            _ => {}
        }
    }

    fn user_prompt(&mut self) {
        self.messages += 1;
        self.in_assistant = false;
    }

    fn assistant_delta(&mut self) {
        if self.in_assistant {
            return;
        }

        self.messages += 1;
        self.in_assistant = true;
    }

    fn finish(self) -> HistorySummary {
        HistorySummary {
            id: self.id,
            updated: self.updated,
            title: self.title,
            messages: self.messages,
            corrupt: self.corrupt,
        }
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/session/index.rs"
    ));
}
