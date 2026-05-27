use crate::chat::commands::{ChatCommandContext, ChatCommandFuture, ChatCommandResult};
use spectacular_commands::CommandError;

use std::io;
use std::process::{Command, Output};
use tokio::task::JoinError;

const STATUS_ARGS: &[&str] = &["status", "--short"];
const STAGED_ARGS: &[&str] = &["diff", "--cached", "--stat"];

/// Internal execute function that can be called from the parent git command
pub fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move { run(context, args).await })
}

async fn run(context: ChatCommandContext<'_>, args: Vec<String>) -> ChatCommandResult {
    if let Err(result) = validate_args(&args) {
        return result;
    }

    if let Err(result) = show_working_changes(&context).await {
        return result;
    }

    if let Err(result) = show_staged_changes(&context).await {
        return result;
    }

    ChatCommandResult::success()
}

fn validate_args(args: &[String]) -> Result<(), ChatCommandResult> {
    if args.is_empty() {
        return Ok(());
    }

    Err(ChatCommandResult::error(
        CommandError::usage("/git status").to_string(),
    ))
}

async fn show_working_changes(context: &ChatCommandContext<'_>) -> Result<(), ChatCommandResult> {
    let output = git_output(context, STATUS_ARGS, "git status").await?;
    report_output(
        context,
        &output,
        "Working directory changes:",
        "No changes in working directory.",
    );
    Ok(())
}

async fn show_staged_changes(context: &ChatCommandContext<'_>) -> Result<(), ChatCommandResult> {
    let output = git_output(context, STAGED_ARGS, "git diff --cached").await?;
    report_output(context, &output, "Staged changes:", "No staged changes.");
    Ok(())
}

fn report_output(
    context: &ChatCommandContext<'_>,
    output: &Output,
    heading: &str,
    empty_message: &str,
) {
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        context.notice(empty_message);
    } else {
        context.notice(heading);
        context.notice(&text);
    }
}

async fn git_output(
    context: &ChatCommandContext<'_>,
    args: &'static [&'static str],
    label: &'static str,
) -> Result<Output, ChatCommandResult> {
    let result = context
        .work(async move { tokio::task::spawn_blocking(move || command_output(args)).await })
        .await;

    map_command_result(result, label)
}

fn command_output(args: &[&str]) -> io::Result<Output> {
    Command::new("git").args(args).output()
}

fn map_command_result(
    result: Result<io::Result<Output>, JoinError>,
    label: &str,
) -> Result<Output, ChatCommandResult> {
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(command_error(label, error)),
        Err(error) => Err(ChatCommandResult::error(format!(
            "Failed to run {label}: {error}"
        ))),
    }
}

fn command_error(label: &str, error: io::Error) -> ChatCommandResult {
    ChatCommandResult::error(format!("Failed to run {label}: {error}"))
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/git/status.rs"
    ));
}
