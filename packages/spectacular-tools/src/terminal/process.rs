use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::Instant;

use spectacular_agent::Cancellation;

use super::CANCELLATION_POLL_MS;

/// Terminal command wait outcome after process completion, timeout, cancellation, or wait failure.
pub(crate) enum CommandCompletion {
    Exited(std::process::ExitStatus),
    WaitError(std::io::Error),
    TimedOut,
    Cancelled,
}

/// Waits for a child process while polling for cancellation and enforcing the timeout deadline.
pub(crate) async fn wait_for_completion(
    child: &mut Child,
    child_id: Option<u32>,
    timeout_ms: u64,
    cancellation: Cancellation,
) -> CommandCompletion {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        if cancellation.is_cancelled() {
            terminate_process_tree(child_id, child).await;
            let _ = child.wait().await;
            return CommandCompletion::Cancelled;
        }

        let now = Instant::now();
        if now >= deadline {
            terminate_process_tree(child_id, child).await;
            let _ = child.wait().await;
            return CommandCompletion::TimedOut;
        }

        let poll_duration = Duration::from_millis(CANCELLATION_POLL_MS)
            .min(deadline.saturating_duration_since(now));
        tokio::select! {
            result = child.wait() => {
                return match result {
                    Ok(status) => CommandCompletion::Exited(status),
                    Err(error) => CommandCompletion::WaitError(error),
                };
            }
            _ = tokio::time::sleep(poll_duration) => {}
        }
    }
}

/// Terminates a child process tree using platform-specific tree termination and direct-child fallback.
pub(crate) async fn terminate_process_tree(child_id: Option<u32>, child: &mut Child) {
    if let Some(child_id) = child_id {
        terminate_platform_process_tree(child_id).await;
    }

    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.start_kill();
    }
}

#[cfg(windows)]
/// Terminates a Windows child process tree with taskkill.
async fn terminate_platform_process_tree(child_id: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &child_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(unix)]
/// Terminates a Unix child process group with TERM followed by KILL.
async fn terminate_platform_process_tree(child_id: u32) {
    let process_group = format!("-{child_id}");
    let _ = Command::new("kill")
        .args(["-TERM", &process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = Command::new("kill")
        .args(["-KILL", &process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

#[cfg(not(any(unix, windows)))]
/// Ignores process-tree termination on platforms without a supported implementation.
async fn terminate_platform_process_tree(_child_id: u32) {}

#[cfg(unix)]
/// Starts Unix shells in a new process group so cancellation can target the group.
pub(crate) fn configure_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
/// Leaves process-group configuration unchanged on non-Unix platforms.
pub(crate) fn configure_process_group(_command: &mut Command) {}
