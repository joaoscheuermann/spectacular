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
            let Some(id) = entry
                .path()
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
    let mut title = UNTITLED.to_owned();
    let mut messages = 0usize;
    let mut in_assistant = false;
    let mut updated = DateTime::<Utc>::from(UNIX_EPOCH);
    let mut corrupt = false;

    for record in records {
        corrupt |= record.is_corrupt_or_unknown();
        let Some(event) = record.event() else {
            continue;
        };
        if let Some(created_at) = event.created_at() {
            updated = updated.max(created_at);
        }

        match event {
            ChatEvent::SessionTitleUpdated {
                title: event_title, ..
            } => title = event_title.clone(),
            ChatEvent::UserPrompt { .. } => {
                messages += 1;
                in_assistant = false;
            }
            ChatEvent::AssistantDelta { .. } if !in_assistant => {
                messages += 1;
                in_assistant = true;
            }
            ChatEvent::Finished { .. } | ChatEvent::Error { .. } | ChatEvent::Cancelled { .. } => {
                in_assistant = false;
            }
            _ => {}
        }
    }

    HistorySummary {
        id: id.to_owned(),
        updated,
        title,
        messages,
        corrupt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_conversation_turns_and_groups_assistant_deltas() {
        let records = vec![
            known(ChatEvent::SessionStarted {
                schema_version: 1,
                id: "a83f19c2".to_owned(),
                title: UNTITLED.to_owned(),
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            known(ChatEvent::UserPrompt {
                content: "hello".to_owned(),
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
            known(ChatEvent::AssistantDelta {
                role: "assistant".to_owned(),
                content: "one".to_owned(),
                created_at: "2026-04-29T14:02:00Z".to_owned(),
            }),
            known(ChatEvent::AssistantDelta {
                role: "assistant".to_owned(),
                content: " two".to_owned(),
                created_at: "2026-04-29T14:03:00Z".to_owned(),
            }),
            known(ChatEvent::Finished {
                reason: "stop".to_owned(),
                created_at: "2026-04-29T14:04:00Z".to_owned(),
            }),
        ];

        let summary = summarize("a83f19c2", &records);

        assert_eq!(summary.messages, 2);
        assert_eq!(summary.updated.to_rfc3339(), "2026-04-29T14:04:00+00:00");
    }

    #[test]
    fn summary_uses_latest_title_event() {
        let records = vec![
            known(ChatEvent::SessionTitleUpdated {
                title: "Old".to_owned(),
                slot: "labeling".to_owned(),
                model: "label/model".to_owned(),
                source: None,
                created_at: "2026-04-29T14:00:00Z".to_owned(),
            }),
            known(ChatEvent::SessionTitleUpdated {
                title: "New Title".to_owned(),
                slot: "labeling".to_owned(),
                model: "label/model".to_owned(),
                source: None,
                created_at: "2026-04-29T14:01:00Z".to_owned(),
            }),
            known(ChatEvent::UserPrompt {
                content: "hello".to_owned(),
                created_at: "2026-04-29T14:02:00Z".to_owned(),
            }),
        ];

        let summary = summarize("a83f19c2", &records);

        assert_eq!(summary.title, "New Title");
    }

    fn known(event: ChatEvent) -> ChatRecord {
        ChatRecord::Known { line: 1, event }
    }
}
