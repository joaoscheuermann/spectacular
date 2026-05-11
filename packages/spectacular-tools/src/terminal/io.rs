use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinHandle;

/// Captures child-process stream bytes while allowing callers to stop waiting for EOF.
pub(crate) struct OutputReader {
    buffer: Arc<Mutex<Vec<u8>>>,
    task: JoinHandle<()>,
}

/// Spawns an async task that drains an optional child-process stream into a shared buffer.
pub(crate) fn spawn_reader<R>(reader: Option<R>) -> OutputReader
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let task_buffer = Arc::clone(&buffer);
    let task = tokio::spawn(async move {
        let Some(mut reader) = reader else {
            return;
        };
        let mut chunk = [0_u8; 8192];

        loop {
            match reader.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(bytes_read) => append_bytes(&task_buffer, &chunk[..bytes_read]),
            }
        }
    });

    OutputReader { buffer, task }
}

/// Resolves a reader task into text, aborting it when inherited pipes keep the stream open.
pub(crate) async fn read_joined(reader: OutputReader, drain_timeout: Duration) -> String {
    let OutputReader { buffer, mut task } = reader;
    if tokio::time::timeout(drain_timeout, &mut task)
        .await
        .is_err()
    {
        task.abort();
        let _ = task.await;
    }

    let bytes = read_buffer(&buffer);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Clones buffered stream bytes, returning empty output if the reader task panicked while locked.
fn read_buffer(buffer: &Arc<Mutex<Vec<u8>>>) -> Vec<u8> {
    buffer
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default()
}

/// Appends bytes read from a process stream to the shared output buffer.
fn append_bytes(buffer: &Arc<Mutex<Vec<u8>>>, bytes: &[u8]) {
    if let Ok(mut buffer) = buffer.lock() {
        buffer.extend_from_slice(bytes);
    }
}

/// Appends a status message to an existing stderr string, inserting a newline when needed.
pub(crate) fn append_message(mut existing: String, message: String) -> String {
    if existing.is_empty() {
        return message;
    }

    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&message);
    existing
}
