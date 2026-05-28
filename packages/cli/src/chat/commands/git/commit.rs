//! `/git-commit` command implementation.
//!
//! Generates a conventional commit message using a standalone AI agent
//! and commits the currently staged changes.

use crate::chat::command_event::CommandStatus;
use crate::chat::commands::{ChatCommandContext, ChatCommandFuture, ChatCommandResult};
use crate::chat::selection::{
    SelectionPromptAnswer, SelectionPromptChoice, SelectionPromptRequest,
};
use crate::chat::ChatError;

use crate::chat::provider::provider_for_runtime;
use ::agent::{Agent, AgentConfig, AgentEvent, AgentRunStream};
use ::commands::CommandError;

use std::sync::Arc;

use super::helpers;

mod lifecycle;

use lifecycle::CommitLifecycle;

/// Maximum diff length sent to the agent. Diffs larger than this are truncated.
const MAX_DIFF_CHARS: usize = 15_000;

const MAX_COMMAND_TEXT_CHARS: usize = 80;
const MAX_COMMAND_DELTA_EVENTS: usize = 32;
const MAX_COMMAND_DELTA_BYTES: usize = 4_096;
const MAX_COMMAND_DELTA_CONTENT_CHARS: usize = 240;
const MAX_COMMAND_SUMMARY_CHARS: usize = 240;
const TEXT_TRUNCATION_MARKER: &str = "... [truncated]";
const COMMAND_DELTA_TRUNCATED_NOTICE: &str = "command output truncated: persistence limit reached";

/// System prompt for commit message generation
const COMMIT_SYSTEM_PROMPT: &str = include_str!("prompt/commit-system.md");

/// User prompt template for commit message generation
const COMMIT_USER_PROMPT_TEMPLATE: &str = include_str!("prompt/commit-user.md");

/// Internal execute function that can be called from the parent git command
pub fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        let mut lifecycle = CommitLifecycle::new(&context);

        if let Err(error) = lifecycle.start() {
            return ChatCommandResult::error(error);
        }

        run(&context, &mut lifecycle, args).await
    })
}

async fn run(
    context: &ChatCommandContext<'_>,
    lifecycle: &mut CommitLifecycle<'_, '_>,
    args: Vec<String>,
) -> ChatCommandResult {
    if let Err(result) = validate_args(lifecycle, &args) {
        return result;
    }

    let diff = match load_staged_diff(context, lifecycle).await {
        Ok(diff) => diff,
        Err(result) => return result,
    };

    let generated_message = match generate_message(context, lifecycle, &diff).await {
        Ok(message) => message,
        Err(result) => return result,
    };

    let commit_message = match choose_message(context, lifecycle, &generated_message).await {
        Ok(Some(message)) => message,
        Ok(None) => return ChatCommandResult::success(),
        Err(result) => return result,
    };

    apply_commit(context, lifecycle, commit_message).await
}

fn validate_args(
    lifecycle: &CommitLifecycle<'_, '_>,
    args: &[String],
) -> Result<(), ChatCommandResult> {
    if args.is_empty() {
        return Ok(());
    }

    let _ = lifecycle.finish(
        CommandStatus::Failed,
        "/git commit failed: invalid arguments",
    );
    Err(ChatCommandResult::error(
        CommandError::usage("/git commit").to_string(),
    ))
}

async fn load_staged_diff(
    context: &ChatCommandContext<'_>,
    lifecycle: &mut CommitLifecycle<'_, '_>,
) -> Result<String, ChatCommandResult> {
    ensure_staged_changes(context, lifecycle).await?;
    lifecycle
        .delta("loading staged diff")
        .map_err(ChatCommandResult::error)?;

    context
        .work(async { helpers::get_staged_diff().await })
        .await
        .map_err(|error| staged_diff_error(lifecycle, error.to_string()))
}

async fn ensure_staged_changes(
    context: &ChatCommandContext<'_>,
    lifecycle: &mut CommitLifecycle<'_, '_>,
) -> Result<(), ChatCommandResult> {
    lifecycle
        .delta("checking staged changes")
        .map_err(ChatCommandResult::error)?;

    let has_staged = context
        .work(async { helpers::has_staged_changes().await })
        .await
        .map_err(|error| staged_changes_error(lifecycle, error.to_string()))?;

    if has_staged {
        return Ok(());
    }

    let message = "no staged changes to commit. Use `git add` to stage changes first.";
    let _ = lifecycle.finish(CommandStatus::Failed, message);
    Err(ChatCommandResult::error(message.to_owned()))
}

fn staged_changes_error(
    lifecycle: &mut CommitLifecycle<'_, '_>,
    message: String,
) -> ChatCommandResult {
    let _ = lifecycle.delta(&format!("staged changes check failed: {message}"));
    let _ = lifecycle.finish(
        CommandStatus::Failed,
        format!("/git commit failed while checking staged changes: {message}"),
    );
    ChatCommandResult::error(message)
}

fn staged_diff_error(
    lifecycle: &mut CommitLifecycle<'_, '_>,
    message: String,
) -> ChatCommandResult {
    let _ = lifecycle.delta(&format!("staged diff load failed: {message}"));
    let _ = lifecycle.finish(
        CommandStatus::Failed,
        format!("/git commit failed while loading staged diff: {message}"),
    );
    ChatCommandResult::error(message)
}

async fn generate_message(
    context: &ChatCommandContext<'_>,
    lifecycle: &mut CommitLifecycle<'_, '_>,
    diff: &str,
) -> Result<String, ChatCommandResult> {
    let (diff_for_prompt, _) = truncate_diff_if_needed(diff);
    let prompt = build_commit_prompt(&diff_for_prompt);

    lifecycle
        .delta("generating commit message.")
        .map_err(ChatCommandResult::error)?;

    let message = generate_commit_message_with_work(context, prompt)
        .await
        .map_err(|error| generation_error(lifecycle, error))?;

    if message.trim().is_empty() {
        let message = "generated commit message is empty. Please commit manually.";
        let _ = lifecycle.finish(CommandStatus::Failed, message);
        return Err(ChatCommandResult::error(message.to_owned()));
    }

    Ok(message)
}

fn generation_error(
    lifecycle: &mut CommitLifecycle<'_, '_>,
    error: CommitMessageGenerationError,
) -> ChatCommandResult {
    match error {
        CommitMessageGenerationError::Cancelled(reason) => {
            let summary = format!("commit message generation cancelled: {reason}");
            let _ = lifecycle.delta(&summary);
            let _ = lifecycle.finish(CommandStatus::Cancelled, summary);
            ChatCommandResult::success()
        }
        CommitMessageGenerationError::Failed(message) => {
            let _ = lifecycle.delta(&format!("commit message generation failed: {message}"));
            let _ = lifecycle.finish(
                CommandStatus::Failed,
                format!("failed to generate commit message: {message}"),
            );
            ChatCommandResult::error(format!("failed to generate commit message: {}", message))
        }
    }
}

async fn choose_message(
    context: &ChatCommandContext<'_>,
    lifecycle: &CommitLifecycle<'_, '_>,
    generated_message: &str,
) -> Result<Option<String>, ChatCommandResult> {
    context.blank_line();

    match select_commit_message(context, generated_message).await {
        Ok(Some(message)) => Ok(Some(message)),
        Ok(None) => {
            let _ = lifecycle.finish(CommandStatus::Cancelled, "commit cancelled");
            context.notice("commit cancelled");
            Ok(None)
        }
        Err(error) => Err(ChatCommandResult::error(error.to_string())),
    }
}

async fn apply_commit(
    context: &ChatCommandContext<'_>,
    lifecycle: &mut CommitLifecycle<'_, '_>,
    commit_message: String,
) -> ChatCommandResult {
    if let Err(error) = lifecycle.delta("committing changes") {
        return ChatCommandResult::error(error);
    }

    match context
        .work(async { helpers::commit_with_message(&commit_message).await })
        .await
    {
        Ok(output) => finish_commit(lifecycle, output),
        Err(error) => commit_error(lifecycle, error.to_string()),
    }
}

fn finish_commit(lifecycle: &mut CommitLifecycle<'_, '_>, output: String) -> ChatCommandResult {
    let commit_output = output.trim();
    if !commit_output.is_empty() {
        if let Err(error) = lifecycle.delta(commit_output) {
            return ChatCommandResult::error(error);
        }
    }

    let _ = lifecycle.finish(CommandStatus::Success, "changes committed successfully");
    ChatCommandResult::success()
}

fn commit_error(lifecycle: &mut CommitLifecycle<'_, '_>, message: String) -> ChatCommandResult {
    let _ = lifecycle.delta(&format!("git commit failed: {message}"));
    let _ = lifecycle.finish(CommandStatus::Failed, format!("commit failed: {message}"));
    ChatCommandResult::error(format!("commit failed: {}", message))
}

async fn select_commit_message(
    context: &ChatCommandContext<'_>,
    generated_message: &str,
) -> Result<Option<String>, ChatError> {
    match context
        .ask(commit_message_selection_request(generated_message))
        .await
    {
        Ok(answer) => commit_message_from_selection(answer, generated_message),
        Err(ChatError::Exit) => Ok(None),
        Err(error) => Err(error),
    }
}

fn commit_message_selection_request(generated_message: &str) -> SelectionPromptRequest {
    SelectionPromptRequest::new(
        "Use generated commit message?",
        format!("Message: \"{generated_message}\""),
        vec![
            "Use generated message".to_owned(),
            "Cancel commit".to_owned(),
        ],
    )
    .with_inputs(true, false)
}

fn commit_message_from_selection(
    answer: SelectionPromptAnswer,
    generated_message: &str,
) -> Result<Option<String>, ChatError> {
    match answer.choice {
        SelectionPromptChoice::Option { index: 0, .. } => Ok(Some(generated_message.to_owned())),
        SelectionPromptChoice::Option { index: 1, .. } => Ok(None),
        SelectionPromptChoice::Option { label, .. } => Err(ChatError::Session(format!(
            "unsupported commit message selection: {label}"
        ))),
        SelectionPromptChoice::Custom(message) => Ok(Some(format_commit_message_with_comment(
            &message,
            answer.comment.as_deref(),
        ))),
    }
}

fn format_commit_message_with_comment(message: &str, comment: Option<&str>) -> String {
    let message = sanitize_commit_message(message);
    let Some(comment) = comment.and_then(non_empty_selection_comment) else {
        return message;
    };

    format!("{message}\n\n{comment}")
}

fn non_empty_selection_comment(comment: &str) -> Option<&str> {
    let comment = comment.trim();
    if comment.is_empty() {
        return None;
    }

    Some(comment)
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let bounded = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_none() {
        return bounded;
    }

    let marker_chars = TEXT_TRUNCATION_MARKER.chars().count();
    if max_chars <= marker_chars {
        return ".".repeat(max_chars);
    }

    let prefix_chars = max_chars - marker_chars;
    let mut bounded = value.chars().take(prefix_chars).collect::<String>();
    bounded.push_str(TEXT_TRUNCATION_MARKER);
    bounded
}

enum CommitMessageGenerationError {
    Cancelled(String),
    Failed(String),
}

async fn generate_commit_message_with_work(
    context: &ChatCommandContext<'_>,
    prompt: String,
) -> Result<String, CommitMessageGenerationError> {
    context
        .work(async { generate_commit_message(context, prompt).await })
        .await
}

async fn generate_commit_message(
    context: &ChatCommandContext<'_>,
    prompt: String,
) -> Result<String, CommitMessageGenerationError> {
    let provider = provider_for_runtime(
        context.model.runtime(),
        context.model.debug_logger().clone(),
        context.model.config_io(),
    )
    .map_err(|e| CommitMessageGenerationError::Failed(format!("provider error: {}", e)))?;

    let agent = Arc::new(Agent::with_config(provider, commit_agent_config(context)));
    collect_commit_message(agent.run_stream(prompt)).await
}

fn commit_agent_config(context: &ChatCommandContext<'_>) -> AgentConfig {
    AgentConfig {
        system_prompt: COMMIT_SYSTEM_PROMPT.to_owned(),
        model: Some(context.model.runtime().model.clone()),
        require_usage_metadata: false,
        ..AgentConfig::default()
    }
}

async fn collect_commit_message(
    mut stream: AgentRunStream,
) -> Result<String, CommitMessageGenerationError> {
    let mut message = String::new();

    while let Some(event) = stream.next().await {
        if handle_commit_message_event(event, &mut message)? {
            break;
        }
    }

    Ok(sanitize_commit_message(&message))
}

fn handle_commit_message_event(
    event: AgentEvent,
    message: &mut String,
) -> Result<bool, CommitMessageGenerationError> {
    match event {
        AgentEvent::MessageDelta { content, .. } => {
            message.push_str(&content);
            Ok(false)
        }
        AgentEvent::Finished { .. } => Ok(true),
        AgentEvent::Error { message, .. } => Err(CommitMessageGenerationError::Failed(format!(
            "agent error: {}",
            message
        ))),
        AgentEvent::Cancelled { reason } => Err(CommitMessageGenerationError::Cancelled(reason)),
        _ => Ok(false),
    }
}

fn build_commit_prompt(diff: &str) -> String {
    COMMIT_USER_PROMPT_TEMPLATE.replace("{diff}", diff)
}

fn truncate_diff_if_needed(diff: &str) -> (String, bool) {
    if diff.len() <= MAX_DIFF_CHARS {
        return (diff.to_owned(), false);
    }

    let truncated: String = diff.chars().take(MAX_DIFF_CHARS).collect();
    let omitted = diff.chars().count() - MAX_DIFF_CHARS;

    (
        format!(
            "{}\n\n... [diff truncated - {} characters omitted]",
            truncated, omitted
        ),
        true,
    )
}

fn sanitize_commit_message(message: &str) -> String {
    let without_fences = message.replace("```", "");
    let without_prefix = remove_conversational_prefix(&without_fences);
    let without_label = remove_introductory_line(&without_prefix);

    normalize_commit_message(&without_label)
}

fn remove_conversational_prefix(message: &str) -> String {
    let lower = message.to_lowercase();
    let Some(pos) = lower.find("here is ") else {
        return message.to_owned();
    };

    if pos >= 60 {
        return message.to_owned();
    }

    let Some(colon) = message[pos..].find(':') else {
        return message.to_owned();
    };

    message[pos + colon + 1..].to_owned()
}

fn remove_introductory_line(message: &str) -> String {
    let mut lines = message.lines();
    let Some(first_line) = lines.clone().next() else {
        return message.to_owned();
    };

    if !is_introductory_line(first_line) {
        return message.to_owned();
    }

    lines.next();
    lines.collect::<Vec<_>>().join("\n")
}

fn is_introductory_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("message") && line.ends_with(':')
        || lower.contains("here's")
        || lower.contains("here is")
}

fn normalize_commit_message(message: &str) -> String {
    message
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .trim_start_matches('"')
        .trim_end_matches('"')
        .to_owned()
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/commands/git/commit.rs"
    ));
}
