use crate::chat::session::{now, ChatEvent, ChatRecord};
use crate::chat::ChatError;
use ::tui::Session;
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SessionStore {
    dir: PathBuf,
    append_lock: Arc<Mutex<()>>,
}

impl SessionStore {
    pub fn new(dir: PathBuf) -> Result<Self, ChatError> {
        fs::create_dir_all(&dir).map_err(|error| ChatError::Session(error.to_string()))?;
        Ok(Self {
            dir,
            append_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Returns the append-only JSONL event path for a session identifier.
    pub fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.jsonl"))
    }

    /// Returns the durable semantic snapshot path for a session identifier.
    pub fn snapshot_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.snapshot.json"))
    }

    /// Returns whether either persisted session representation exists for an identifier.
    pub fn exists(&self, id: &str) -> bool {
        self.path(id).exists() || self.snapshot_path(id).exists()
    }

    /// Saves the durable semantic TUI session snapshot without terminal output replay data.
    #[allow(dead_code)]
    pub fn save_snapshot(&self, session: &Session) -> Result<(), ChatError> {
        let path = self.snapshot_path(session.id.as_str());
        let mut file = File::create(path).map_err(|error| ChatError::Session(error.to_string()))?;
        serde_json::to_writer_pretty(&mut file, session)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        writeln!(file).map_err(|error| ChatError::Session(error.to_string()))?;
        file.flush()
            .map_err(|error| ChatError::Session(error.to_string()))
    }

    /// Loads a durable semantic TUI session snapshot by session identifier.
    #[allow(dead_code)]
    pub fn load_snapshot(&self, id: &str) -> Result<Session, ChatError> {
        let path = self.snapshot_path(id);
        let file = File::open(path).map_err(|error| ChatError::Session(error.to_string()))?;
        serde_json::from_reader(file).map_err(|error| ChatError::Session(error.to_string()))
    }

    pub fn append(&self, path: &Path, event: &ChatEvent) -> Result<(), ChatError> {
        let _guard = self.append_lock.lock().map_err(|error| {
            ChatError::Session(format!("session append lock poisoned: {error}"))
        })?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        serde_json::to_writer(&mut file, event)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        writeln!(file).map_err(|error| ChatError::Session(error.to_string()))?;
        file.flush()
            .map_err(|error| ChatError::Session(error.to_string()))
    }

    pub fn read(&self, path: &Path) -> Result<Vec<ChatRecord>, ChatError> {
        read_records(path)
    }

    pub fn truncate_after_latest_user_prompt(&self, path: &Path) -> Result<String, ChatError> {
        let target = latest_prompt_target(&read_content(path)?)?;
        truncate_file(path, target.offset)?;
        Ok(target.prompt)
    }
}

fn read_records(path: &Path) -> Result<Vec<ChatRecord>, ChatError> {
    BufReader::new(open_file(path)?)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.map_err(to_session_error)?;
            Ok(record_from_line(index + 1, &line))
        })
        .collect()
}

fn record_from_line(line_number: usize, line: &str) -> ChatRecord {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return ChatRecord::Corrupt { line: line_number };
    };

    record_from_value(line_number, value)
}

fn record_from_value(line_number: usize, value: Value) -> ChatRecord {
    match ChatEvent::from_value(value) {
        Ok(event) => ChatRecord::Known {
            line: line_number,
            event,
        },
        Err(value) => ChatRecord::Unknown {
            line: line_number,
            value,
        },
    }
}

struct RetryTarget {
    prompt: String,
    offset: usize,
}

fn latest_prompt_target(content: &str) -> Result<RetryTarget, ChatError> {
    let mut latest = None;
    let mut offset = 0usize;

    for line in content.split_inclusive('\n') {
        offset += line.len();
        if let Some(prompt) = line_user_prompt(line) {
            latest = Some(RetryTarget { prompt, offset });
        }
    }

    latest.ok_or_else(|| ChatError::Session("no prompt to retry".to_owned()))
}

fn line_user_prompt(line: &str) -> Option<String> {
    let event = serde_json::from_str::<Value>(line.trim_end())
        .ok()
        .and_then(|value| ChatEvent::from_value(value).ok())?;
    event.user_prompt().map(str::to_owned)
}

fn read_content(path: &Path) -> Result<String, ChatError> {
    fs::read_to_string(path).map_err(to_session_error)
}

fn open_file(path: &Path) -> Result<File, ChatError> {
    File::open(path).map_err(to_session_error)
}

fn truncate_file(path: &Path, offset: usize) -> Result<(), ChatError> {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(to_session_error)?;
    file.set_len(offset as u64).map_err(to_session_error)
}

fn to_session_error(error: std::io::Error) -> ChatError {
    ChatError::Session(error.to_string())
}

pub fn session_started(id: &str, schema_version: u64, title: &str) -> ChatEvent {
    ChatEvent::SessionStarted {
        schema_version,
        id: id.to_owned(),
        title: title.to_owned(),
        created_at: now(),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/session/store.rs"
    ));
}

#[cfg(test)]
mod snapshot_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/session/snapshot.rs"
    ));
}
