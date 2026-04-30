use crate::chat::session::{now, ChatEvent, ChatRecord};
use crate::chat::ChatError;
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

    pub fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.jsonl"))
    }

    pub fn exists(&self, id: &str) -> bool {
        self.path(id).exists()
    }

    pub fn append(&self, path: &Path, event: &ChatEvent) -> Result<(), ChatError> {
        let _guard = self.append_lock.lock().unwrap();
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
        let content =
            fs::read_to_string(path).map_err(|error| ChatError::Session(error.to_string()))?;
        let mut offset = 0usize;
        let mut latest_prompt = None;
        let mut truncate_at = None;

        for line in content.split_inclusive('\n') {
            offset += line.len();
            let parsed = serde_json::from_str::<Value>(line.trim_end())
                .ok()
                .and_then(|value| ChatEvent::from_value(value).ok());
            let Some(event) = parsed else {
                continue;
            };
            let Some(prompt) = event.user_prompt() else {
                continue;
            };

            latest_prompt = Some(prompt.to_owned());
            truncate_at = Some(offset);
        }

        let prompt =
            latest_prompt.ok_or_else(|| ChatError::Session("no prompt to retry".to_owned()))?;
        let truncate_at =
            truncate_at.ok_or_else(|| ChatError::Session("no prompt to retry".to_owned()))?;
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        file.set_len(truncate_at as u64)
            .map_err(|error| ChatError::Session(error.to_string()))?;
        Ok(prompt)
    }
}

fn read_records(path: &Path) -> Result<Vec<ChatRecord>, ChatError> {
    let file = File::open(path).map_err(|error| ChatError::Session(error.to_string()))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line.map_err(|error| ChatError::Session(error.to_string()))?;
        let value = match serde_json::from_str::<Value>(&line) {
            Ok(value) => value,
            Err(_) => {
                records.push(ChatRecord::Corrupt { line: line_number });
                continue;
            }
        };

        match ChatEvent::from_value(value) {
            Ok(event) => records.push(ChatRecord::Known {
                line: line_number,
                event,
            }),
            Err(value) => records.push(ChatRecord::Unknown {
                line: line_number,
                value,
            }),
        }
    }
    Ok(records)
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
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn read_preserves_corrupt_and_unknown_records() {
        let dir = temp_dir("read_preserves_corrupt_and_unknown_records");
        let store = SessionStore::new(dir.clone()).unwrap();
        let path = store.path("a83f19c2");
        fs::write(
            &path,
            concat!(
                "{\"type\":\"user_prompt\",\"content\":\"hello\",\"created_at\":\"2026-04-29T14:01:00Z\"}\n",
                "{\"type\":\"future_event\",\"payload\":true}\n",
                "not json\n"
            ),
        )
        .unwrap();

        let records = store.read(&path).unwrap();

        assert!(matches!(records[0], ChatRecord::Known { .. }));
        assert!(matches!(records[1], ChatRecord::Unknown { .. }));
        assert!(matches!(records[2], ChatRecord::Corrupt { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn truncate_after_latest_user_prompt_keeps_prompt_and_drops_later_events() {
        let dir = temp_dir("truncate_after_latest_user_prompt");
        let store = SessionStore::new(dir.clone()).unwrap();
        let path = store.path("a83f19c2");
        fs::write(
            &path,
            concat!(
                "{\"type\":\"user_prompt\",\"content\":\"hello\",\"created_at\":\"2026-04-29T14:01:00Z\"}\n",
                "{\"type\":\"assistant_delta\",\"role\":\"assistant\",\"content\":\"hi\",\"created_at\":\"2026-04-29T14:02:00Z\"}\n"
            ),
        )
        .unwrap();

        let prompt = store.truncate_after_latest_user_prompt(&path).unwrap();
        let content = fs::read_to_string(&path).unwrap();

        assert_eq!(prompt, "hello");
        assert!(content.contains("\"user_prompt\""));
        assert!(!content.contains("\"assistant_delta\""));

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("spectacular-chat-{test_name}-{nanos}"))
    }
}
