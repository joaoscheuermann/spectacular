mod event;
mod index;
mod store;

use crate::chat::{ChatError, RuntimeSelection};
use chrono::{DateTime, Local, Utc};
pub use event::ChatEvent;
use index::SessionIndex;
use serde_json::Value;
use spectacular_agent::AgentEvent;
use spectacular_config::TaskModelSlot;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use store::{session_started, SessionStore};

const SCHEMA_VERSION: u64 = 1;
pub(super) const UNTITLED: &str = "Untitled session";

#[derive(Clone)]
pub struct SessionManager {
    store: SessionStore,
    active: Option<ActiveSession>,
}

#[derive(Clone)]
struct ActiveSession {
    id: String,
    path: PathBuf,
}

impl SessionManager {
    pub fn new() -> Result<Self, ChatError> {
        let dir = spectacular_config::config_dir()
            .map_err(ChatError::Config)?
            .join("sessions");
        Ok(Self {
            store: SessionStore::new(dir)?,
            active: None,
        })
    }

    pub fn current_id(&self) -> &str {
        self.active
            .as_ref()
            .map(|active| active.id.as_str())
            .unwrap_or("none")
    }

    pub fn create(&mut self, runtime: RuntimeSelection) -> Result<(), ChatError> {
        let id = self.unique_id();
        let path = self.store.path(&id);
        self.active = Some(ActiveSession {
            id: id.clone(),
            path,
        });
        self.append_event(&session_started(&id, SCHEMA_VERSION, UNTITLED))?;
        self.append_runtime_defaults(&runtime, "global_default")
    }

    pub fn append_runtime_defaults(
        &self,
        runtime: &RuntimeSelection,
        source: &str,
    ) -> Result<(), ChatError> {
        self.append_event(&ChatEvent::ProviderChanged {
            provider: runtime.provider.clone(),
            source: Some(source.to_owned()),
            created_at: now(),
        })?;
        self.append_event(&ChatEvent::ModelChanged {
            slot: TaskModelSlot::Coding.as_str().to_owned(),
            provider: runtime.provider.clone(),
            model: runtime.model.clone(),
            reasoning: runtime.reasoning.as_str().to_owned(),
            source: Some(source.to_owned()),
            created_at: now(),
        })
    }

    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        let Some(event) = ChatEvent::from_agent_event(event, now()) else {
            return Ok(());
        };

        self.append_event(&event)
    }

    pub fn append_title(
        &self,
        title: &str,
        slot: TaskModelSlot,
        model: &str,
        fallback: bool,
    ) -> Result<(), ChatError> {
        self.append_event(&ChatEvent::SessionTitleUpdated {
            title: title.to_owned(),
            slot: slot.as_str().to_owned(),
            model: model.to_owned(),
            source: fallback.then(|| "fallback_coding".to_owned()),
            created_at: now(),
        })
    }

    pub fn has_title(&self) -> Result<bool, ChatError> {
        Ok(self
            .records()?
            .iter()
            .any(|record| matches!(record.event(), Some(ChatEvent::SessionTitleUpdated { .. }))))
    }

    pub fn records(&self) -> Result<Vec<ChatRecord>, ChatError> {
        let active = self.active()?;
        self.store.read(&active.path)
    }

    pub fn history(&self, query: HistoryQuery) -> Result<HistoryPage, ChatError> {
        SessionIndex::new(self.store.dir()).history(query, |path| self.store.read(path))
    }

    pub fn resume(&mut self, prefix: &str) -> Result<Vec<ChatRecord>, ChatError> {
        let matches = SessionIndex::new(self.store.dir()).matching_ids(prefix)?;
        if matches.is_empty() {
            return Err(ChatError::Session("session not found".to_owned()));
        }
        if matches.len() > 1 {
            return Err(ChatError::Session(format!(
                "ambiguous session id: {prefix}; matches: {}",
                matches.join(", ")
            )));
        }

        let id = matches[0].clone();
        let path = self.store.path(&id);
        self.active = Some(ActiveSession {
            id,
            path: path.clone(),
        });
        self.store.read(&path)
    }

    pub fn truncate_after_latest_user_prompt(&self) -> Result<String, ChatError> {
        let active = self.active()?;
        self.store.truncate_after_latest_user_prompt(&active.path)
    }

    fn append_event(&self, event: &ChatEvent) -> Result<(), ChatError> {
        let active = self.active()?;
        self.store.append(&active.path, event)
    }

    fn active(&self) -> Result<&ActiveSession, ChatError> {
        self.active
            .as_ref()
            .ok_or_else(|| ChatError::Session("no active chat session is available".to_owned()))
    }

    fn unique_id(&self) -> String {
        loop {
            let id = generate_id();
            if !self.store.exists(&id) {
                return id;
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChatRecord {
    Known { line: usize, event: ChatEvent },
    Unknown { line: usize, value: Value },
    Corrupt { line: usize },
}

impl ChatRecord {
    pub fn line(&self) -> usize {
        match self {
            Self::Known { line, .. } | Self::Unknown { line, .. } | Self::Corrupt { line } => *line,
        }
    }

    pub fn event(&self) -> Option<&ChatEvent> {
        match self {
            Self::Known { event, .. } => Some(event),
            Self::Unknown { .. } | Self::Corrupt { .. } => None,
        }
    }

    pub fn is_corrupt_or_unknown(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }
}

pub fn records_before_latest_user_prompt(records: &[ChatRecord]) -> &[ChatRecord] {
    let Some(index) = records
        .iter()
        .rposition(|record| record.event().is_some_and(ChatEvent::is_user_prompt))
    else {
        return records;
    };

    &records[..index]
}

pub fn agent_events_from_records(records: &[ChatRecord]) -> Vec<AgentEvent> {
    records
        .iter()
        .filter_map(|record| record.event()?.to_agent_event())
        .collect()
}

#[derive(Clone, Copy)]
pub enum HistoryQuery {
    FirstPage,
    Page(usize),
    Range(usize, usize),
}

pub struct HistoryPage {
    pub sessions: Vec<HistorySummary>,
    pub remaining: usize,
}

impl HistoryPage {
    pub(super) fn from_sessions(sessions: Vec<HistorySummary>, query: HistoryQuery) -> Self {
        let total = sessions.len();
        let (start, end) = match query {
            HistoryQuery::FirstPage => (0, 10),
            HistoryQuery::Page(page) => {
                let page = page.max(1);
                ((page - 1) * 10, page * 10)
            }
            HistoryQuery::Range(start, end) => (start.saturating_sub(1), end),
        };
        let visible = sessions
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect::<Vec<_>>();
        let consumed = (start + visible.len()).min(total);
        Self {
            sessions: visible,
            remaining: total.saturating_sub(consumed),
        }
    }
}

pub struct HistorySummary {
    pub id: String,
    pub updated: DateTime<Utc>,
    pub title: String,
    pub messages: usize,
    pub corrupt: bool,
}

pub fn format_local_time(value: DateTime<Utc>) -> String {
    value
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

pub(super) fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    nanos.hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("{:08x}", hasher.finish() as u32)
}
