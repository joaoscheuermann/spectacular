mod event;
mod index;
mod store;

use crate::chat::command_event::CommandEvent;
use crate::chat::{ChatError, RuntimeSelection};
use chrono::{DateTime, Local, Utc};
pub use event::ChatEvent;
use index::SessionIndex;
use serde_json::Value;
use spectacular_agent::AgentEvent;
use spectacular_config::TaskModelSlot;
use spectacular_tui::Session;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use store::{session_started, SessionStore};

const SCHEMA_VERSION: u64 = 2;
pub(super) const UNTITLED: &str = "Untitled session";

/// Owns durable chat-session storage and the currently active session.
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
    /// Opens the default session store under the Spectacular config directory.
    pub fn new() -> Result<Self, ChatError> {
        let dir = spectacular_config::config_dir()
            .map_err(ChatError::Config)?
            .join("sessions");
        Self::new_in(dir)
    }

    pub(super) fn new_in(dir: PathBuf) -> Result<Self, ChatError> {
        Ok(Self {
            store: SessionStore::new(dir)?,
            active: None,
        })
    }

    /// Returns the active session ID, or `none` before a session is active.
    pub fn current_id(&self) -> &str {
        self.active
            .as_ref()
            .map(|active| active.id.as_str())
            .unwrap_or("none")
    }

    /// Creates a new active session and records initial runtime defaults.
    pub fn create(&mut self, runtime: RuntimeSelection) -> Result<(), ChatError> {
        let id = self.unique_id();
        let path = self.store.path(&id);
        self.active = Some(ActiveSession {
            id: id.clone(),
            path,
        });
        self.append_event(&session_started(&id, SCHEMA_VERSION, UNTITLED))?;
        if !runtime.is_ready() {
            return Ok(());
        }

        self.append_runtime_defaults(&runtime, "global_default")
    }

    /// Appends provider and model defaults to the active session transcript.
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

    /// Appends a replayable agent event to the active session transcript.
    pub fn append_agent_event(&self, event: &AgentEvent) -> Result<(), ChatError> {
        let Some(event) = ChatEvent::from_agent_event(event, now()) else {
            return Ok(());
        };

        self.append_event(&event)
    }

    /// Appends an app-owned command lifecycle event to the active session transcript.
    pub fn append_command_event(&self, event: &CommandEvent) -> Result<(), ChatError> {
        self.append_event(&ChatEvent::from_command_event(event, now()))
    }

    /// Reads all records for the active session.
    pub fn records(&self) -> Result<Vec<ChatRecord>, ChatError> {
        let active = self.active()?;
        self.store.read(&active.path)
    }

    /// Saves the durable semantic TUI snapshot for the active session.
    #[allow(dead_code)]
    pub fn save_snapshot(&self, session: &Session) -> Result<(), ChatError> {
        self.store.save_snapshot(session)
    }

    /// Loads the durable semantic TUI snapshot for the active session.
    #[allow(dead_code)]
    pub fn load_snapshot(&self) -> Result<Session, ChatError> {
        let active = self.active()?;
        self.store.load_snapshot(&active.id)
    }

    /// Returns a page of stored session history summaries.
    pub fn history(&self, query: HistoryQuery) -> Result<HistoryPage, ChatError> {
        SessionIndex::new(self.store.dir()).history(query, |path| self.store.read(path))
    }

    /// Resumes the unique session whose ID starts with `prefix`.
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

    /// Removes events after the latest user prompt and returns that prompt.
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

/// One JSONL row read from a chat session transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatRecord {
    /// A recognized event with its source line number.
    Known { line: usize, event: ChatEvent },
    /// A syntactically valid row with an unknown or unsupported event shape.
    Unknown { line: usize, value: Value },
    /// A row that could not be parsed as JSON.
    Corrupt { line: usize },
}

impl ChatRecord {
    /// Returns the parsed event for recognized records.
    pub fn event(&self) -> Option<&ChatEvent> {
        match self {
            Self::Known { event, .. } => Some(event),
            Self::Unknown { .. } | Self::Corrupt { .. } => None,
        }
    }

    /// Returns whether this record could not be replayed as a known event.
    pub fn is_corrupt_or_unknown(&self) -> bool {
        !matches!(self, Self::Known { .. })
    }
}

/// Returns the records before the latest user prompt for retry context.
pub fn records_before_latest_user_prompt(records: &[ChatRecord]) -> &[ChatRecord] {
    let Some(index) = records
        .iter()
        .rposition(|record| record.event().is_some_and(ChatEvent::is_user_prompt))
    else {
        return records;
    };

    &records[..index]
}

/// Converts replayable chat records into agent events.
pub fn agent_events_from_records(records: &[ChatRecord]) -> Vec<AgentEvent> {
    records
        .iter()
        .filter_map(|record| record.event()?.to_agent_event())
        .collect()
}

/// Query shape for paginating or slicing persisted session history.
#[derive(Clone, Copy)]
pub enum HistoryQuery {
    /// The first page of history using the default page size.
    FirstPage,
    /// A one-based page number using the default page size.
    Page(usize),
    /// A one-based inclusive start and exclusive end range.
    Range(usize, usize),
}

/// Page of session history returned to command and model layers.
pub struct HistoryPage {
    /// Visible session summaries for this page.
    pub sessions: Vec<HistorySummary>,
    /// Number of sessions remaining after this page.
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

/// Compact session metadata used by history views.
pub struct HistorySummary {
    /// Session ID used for resume prefixes.
    pub id: String,
    /// Last observed event timestamp for ordering and display.
    pub updated: DateTime<Utc>,
    /// Display title inferred from title events or prompt content.
    pub title: String,
    /// Count of user and assistant messages in the session.
    pub messages: usize,
    /// Whether the session contains corrupt or unknown records.
    pub corrupt: bool,
}

/// Formats a UTC timestamp in the user's local timezone for history output.
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

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/session.rs"
    ));
}
