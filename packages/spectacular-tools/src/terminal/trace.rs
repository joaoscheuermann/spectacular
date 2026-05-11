use super::compact::{count_lines, CompactTraceMetadata, TERMINAL_COMPACT_SCHEMA};
use super::{TerminalExecution, TERMINAL_TOOL_NAME};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TRACE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// File-backed store for full raw terminal output traces.
#[derive(Clone, Debug)]
pub(crate) struct TerminalTraceStore {
    dir: PathBuf,
}

/// Successful terminal trace write details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TraceWriteSuccess {
    pub trace_id: String,
    pub path: PathBuf,
}

/// Failed terminal trace write details.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TraceWriteFailure {
    pub trace_id: String,
    pub error: String,
}

/// Result of trying to write a terminal trace file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TraceWriteResult {
    Written(TraceWriteSuccess),
    Failed(TraceWriteFailure),
}

impl TerminalTraceStore {
    /// Creates a raw terminal trace store rooted at the supplied directory.
    pub(crate) fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Writes a raw terminal trace JSON file, returning non-fatal write status.
    pub(crate) fn write(&self, execution: &TerminalExecution) -> TraceWriteResult {
        let trace_id = trace_id(execution.started_at);
        let directory = self
            .dir
            .join(execution.started_at.format("%Y-%m-%d").to_string());
        let path = directory.join(format!("{trace_id}.json"));

        if let Err(error) = fs::create_dir_all(&directory) {
            return TraceWriteResult::Failed(TraceWriteFailure {
                trace_id,
                error: error.to_string(),
            });
        }

        let file = match File::create(&path) {
            Ok(file) => file,
            Err(error) => {
                return TraceWriteResult::Failed(TraceWriteFailure {
                    trace_id,
                    error: error.to_string(),
                });
            }
        };
        let trace = RawTerminalTrace::from_execution(trace_id.clone(), execution);
        if let Err(error) = serde_json::to_writer(file, &trace) {
            return TraceWriteResult::Failed(TraceWriteFailure {
                trace_id,
                error: error.to_string(),
            });
        }

        TraceWriteResult::Written(TraceWriteSuccess { trace_id, path })
    }
}

impl TraceWriteResult {
    /// Returns the successful trace write details when the write succeeded.
    #[cfg(test)]
    pub(crate) fn written(&self) -> Option<&TraceWriteSuccess> {
        match self {
            Self::Written(success) => Some(success),
            Self::Failed(_) => None,
        }
    }

    /// Converts trace write status into compact payload metadata.
    pub(crate) fn into_compact_metadata(self) -> CompactTraceMetadata {
        match self {
            Self::Written(success) => {
                CompactTraceMetadata::written(success.trace_id, path_to_string(&success.path))
            }
            Self::Failed(failure) => CompactTraceMetadata::failed(failure.trace_id, failure.error),
        }
    }
}

#[derive(Serialize)]
struct RawTerminalTrace {
    schema_version: u8,
    tool: &'static str,
    provider_visible_schema: &'static str,
    trace_id: String,
    session_id: Option<String>,
    tool_call_id: Option<String>,
    command: String,
    working_directory: String,
    started_at: String,
    completed_at: String,
    duration_ms: u128,
    exit_code: i32,
    stdout: String,
    stderr: String,
    stdout_bytes: usize,
    stderr_bytes: usize,
    stdout_lines: usize,
    stderr_lines: usize,
}

impl RawTerminalTrace {
    /// Builds the raw terminal trace schema from a completed execution.
    fn from_execution(trace_id: String, execution: &TerminalExecution) -> Self {
        Self {
            schema_version: 1,
            tool: TERMINAL_TOOL_NAME,
            provider_visible_schema: TERMINAL_COMPACT_SCHEMA,
            trace_id,
            session_id: None,
            tool_call_id: None,
            command: execution.command.clone(),
            working_directory: path_to_string(&execution.working_directory),
            started_at: execution.started_at.to_rfc3339(),
            completed_at: execution.completed_at.to_rfc3339(),
            duration_ms: execution.duration_ms,
            exit_code: execution.exit_code,
            stdout: execution.stdout.clone(),
            stderr: execution.stderr.clone(),
            stdout_bytes: execution.stdout.len(),
            stderr_bytes: execution.stderr.len(),
            stdout_lines: count_lines(&execution.stdout),
            stderr_lines: count_lines(&execution.stderr),
        }
    }
}

/// Generates a Windows-safe trace id from execution time, process id, and a counter.
fn trace_id(started_at: DateTime<Utc>) -> String {
    let counter = TRACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "term_{}_{:09}_{}_{}",
        started_at.format("%Y%m%d_%H%M%S"),
        started_at.timestamp_subsec_nanos(),
        std::process::id(),
        counter
    )
}

/// Converts a path into the absolute local string used in trace references.
fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
