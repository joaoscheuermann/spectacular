use serde_json::{Map, Value};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEBUG_LOG_FILE_NAME: &str = "spectacular-debug.log";

#[derive(Clone)]
pub struct LlmDebugLogger {
    sink: Option<Arc<DebugLogSink>>,
}

struct DebugLogSink {
    path: PathBuf,
    file: Mutex<File>,
}

impl LlmDebugLogger {
    pub fn create_for_current_exe() -> io::Result<Self> {
        let current_exe = std::env::current_exe()?;
        let Some(directory) = current_exe.parent() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "current executable has no parent directory",
            ));
        };

        Self::create_at_path(directory.join(DEBUG_LOG_FILE_NAME))
    }

    pub fn create_at_path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;

        Ok(Self {
            sink: Some(Arc::new(DebugLogSink {
                path,
                file: Mutex::new(file),
            })),
        })
    }

    pub fn disabled() -> Self {
        Self { sink: None }
    }

    pub fn path(&self) -> Option<&Path> {
        self.sink.as_ref().map(|sink| sink.path.as_path())
    }

    pub fn write_event(&self, target: &str, event: &str, fields: Value) -> io::Result<()> {
        let Some(sink) = &self.sink else {
            return Ok(());
        };

        let mut record = Map::new();
        record.insert("ts_ms".to_owned(), Value::from(timestamp_millis()));
        record.insert("target".to_owned(), Value::from(target));
        record.insert("event".to_owned(), Value::from(event));
        append_fields(&mut record, fields);

        let line = serde_json::to_string(&record).map_err(io::Error::other)?;
        let mut file = sink
            .file
            .lock()
            .map_err(|_| io::Error::other("debug log lock poisoned"))?;
        writeln!(file, "{line}")?;
        file.flush()?;
        file.sync_all()
    }

    pub fn write_raw_json(&self, target: &str, event: &str, raw_json: Value) -> io::Result<()> {
        self.write_event(
            target,
            event,
            Value::Object(Map::from_iter([("raw_json".to_owned(), raw_json)])),
        )
    }

    pub fn write_raw_text(&self, target: &str, event: &str, raw_text: &str) -> io::Result<()> {
        self.write_event(
            target,
            event,
            Value::Object(Map::from_iter([(
                "raw_text".to_owned(),
                Value::from(raw_text),
            )])),
        )
    }
}

impl Default for LlmDebugLogger {
    fn default() -> Self {
        Self::disabled()
    }
}

fn append_fields(record: &mut Map<String, Value>, fields: Value) {
    let Value::Object(fields) = fields else {
        record.insert("value".to_owned(), fields);
        return;
    };

    record.extend(fields);
}

fn timestamp_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    millis.min(u128::from(u64::MAX)) as u64
}
