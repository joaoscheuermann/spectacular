//! `/git-commit` command implementation (legacy).
//!
//! Generates a conventional commit message using a standalone AI agent
//! and commits the currently staged changes.

use crate::chat::commands::{ChatCommandContext, ChatCommandFuture, ChatCommandResult};
use crate::chat::prompt::{SelectionPromptAnswer, SelectionPromptChoice, SelectionPromptRequest};
use crate::chat::ChatError;

use crate::chat::provider::provider_for_runtime;
use spectacular_agent::{Agent, AgentConfig, AgentEvent, CommandStatus};
use spectacular_commands::CommandError;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::helpers;

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

        if !args.is_empty() {
            let _ = lifecycle.finish(
                CommandStatus::Failed,
                "/git commit failed: invalid arguments",
            );
            return ChatCommandResult::error(CommandError::usage("/git commit").to_string());
        }

        // 1. Check for staged changes
        if let Err(error) = lifecycle.delta("checking staged changes") {
            return ChatCommandResult::error(error);
        }
        let has_staged = match context
            .work(async { helpers::has_staged_changes().await })
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let message = e.to_string();
                let _ = lifecycle.delta(&format!("staged changes check failed: {message}"));
                let _ = lifecycle.finish(
                    CommandStatus::Failed,
                    format!("/git commit failed while checking staged changes: {message}"),
                );
                return ChatCommandResult::error(message);
            }
        };

        if !has_staged {
            let message = "no staged changes to commit. Use `git add` to stage changes first.";
            let _ = lifecycle.finish(CommandStatus::Failed, message);
            return ChatCommandResult::error(message.to_owned());
        }

        // 2. Get the staged diff
        if let Err(error) = lifecycle.delta("loading staged diff") {
            return ChatCommandResult::error(error);
        }
        let diff = match context
            .work(async { helpers::get_staged_diff().await })
            .await
        {
            Ok(v) => v,
            Err(e) => {
                let message = e.to_string();
                let _ = lifecycle.delta(&format!("staged diff load failed: {message}"));
                let _ = lifecycle.finish(
                    CommandStatus::Failed,
                    format!("/git commit failed while loading staged diff: {message}"),
                );
                return ChatCommandResult::error(message);
            }
        };

        let (diff_for_prompt, _) = truncate_diff_if_needed(&diff);
        let prompt = build_commit_prompt(&diff_for_prompt);

        // 3. Generate commit message using standalone agent
        if let Err(error) = lifecycle.delta("generating commit message.") {
            return ChatCommandResult::error(error);
        }
        let commit_message = match generate_commit_message_with_work(&context, prompt).await {
            Ok(msg) => msg,
            Err(CommitMessageGenerationError::Cancelled(reason)) => {
                let summary = format!("commit message generation cancelled: {reason}");
                let _ = lifecycle.delta(&summary);
                let _ = lifecycle.finish(CommandStatus::Cancelled, summary);
                return ChatCommandResult::success();
            }
            Err(CommitMessageGenerationError::Failed(message)) => {
                let _ = lifecycle.delta(&format!("commit message generation failed: {message}"));
                let _ = lifecycle.finish(
                    CommandStatus::Failed,
                    format!("failed to generate commit message: {message}"),
                );
                return ChatCommandResult::error(format!(
                    "failed to generate commit message: {}",
                    message
                ));
            }
        };

        if commit_message.trim().is_empty() {
            let message = "generated commit message is empty. Please commit manually.";
            let _ = lifecycle.finish(CommandStatus::Failed, message);
            return ChatCommandResult::error(message.to_owned());
        }

        // 4. Show generated message
        context.renderer.blank_line();
        let commit_message = match select_commit_message(&context, &commit_message) {
            Ok(Some(message)) => message,
            Ok(None) => {
                let _ = lifecycle.finish(CommandStatus::Cancelled, "commit cancelled");
                context.notice("commit cancelled");
                return ChatCommandResult::success();
            }
            Err(error) => return ChatCommandResult::error(error.to_string()),
        };

        // 5. Commit
        if let Err(error) = lifecycle.delta("committing changes") {
            return ChatCommandResult::error(error);
        }
        match context
            .work(async { helpers::commit_with_message(&commit_message).await })
            .await
        {
            Ok(output) => {
                let commit_output = output.trim();
                if !commit_output.is_empty() {
                    if let Err(error) = lifecycle.delta(commit_output) {
                        return ChatCommandResult::error(error);
                    }
                }

                let _ = lifecycle.finish(CommandStatus::Success, "changes committed successfully");
                ChatCommandResult::success()
            }
            Err(e) => {
                let message = e.to_string();
                let _ = lifecycle.delta(&format!("git commit failed: {message}"));
                let _ =
                    lifecycle.finish(CommandStatus::Failed, format!("commit failed: {message}"));
                ChatCommandResult::error(format!("commit failed: {}", e))
            }
        }
    })
}

fn select_commit_message(
    context: &ChatCommandContext<'_>,
    generated_message: &str,
) -> Result<Option<String>, ChatError> {
    match context.ask(commit_message_selection_request(generated_message)) {
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

struct CommitLifecycle<'a, 'context> {
    context: &'a ChatCommandContext<'context>,
    command_id: String,
    sequence: u64,
    persisted_delta_bytes: usize,
    persisted_delta_events: usize,
    delta_truncated: bool,
}

impl<'a, 'context> CommitLifecycle<'a, 'context> {
    fn new(context: &'a ChatCommandContext<'context>) -> Self {
        Self {
            context,
            command_id: command_id(),
            sequence: 0,
            persisted_delta_bytes: 0,
            persisted_delta_events: 0,
            delta_truncated: false,
        }
    }

    fn start(&self) -> Result<(), String> {
        let command = bounded_text("/git commit", MAX_COMMAND_TEXT_CHARS);
        self.context
            .append_agent_event(&AgentEvent::command_start(
                self.command_id.clone(),
                "slash_command",
                "/git commit",
                "Git commit",
                command.clone(),
                working_directory(),
            ))
            .map_err(|error| error.to_string())?;
        self.context.renderer.command_start("Git commit", &command);
        Ok(())
    }

    fn delta(&mut self, content: &str) -> Result<(), String> {
        if self.delta_truncated {
            return Ok(());
        }

        let content = bounded_text(content, MAX_COMMAND_DELTA_CONTENT_CHARS);
        if self.should_append_truncation_notice(content.len()) {
            return self.append_delta_truncation_notice();
        }

        self.append_delta_record(content)
    }

    fn should_append_truncation_notice(&self, next_delta_bytes: usize) -> bool {
        if self.persisted_delta_events.saturating_add(1) >= MAX_COMMAND_DELTA_EVENTS {
            return true;
        }

        self.persisted_delta_bytes
            .saturating_add(next_delta_bytes)
            .saturating_add(COMMAND_DELTA_TRUNCATED_NOTICE.len())
            > MAX_COMMAND_DELTA_BYTES
    }

    fn append_delta_truncation_notice(&mut self) -> Result<(), String> {
        self.delta_truncated = true;
        if self.persisted_delta_events >= MAX_COMMAND_DELTA_EVENTS {
            return Ok(());
        }

        let remaining_bytes = MAX_COMMAND_DELTA_BYTES.saturating_sub(self.persisted_delta_bytes);
        if remaining_bytes == 0 {
            return Ok(());
        }

        self.append_delta_record(bounded_text(
            COMMAND_DELTA_TRUNCATED_NOTICE,
            remaining_bytes.min(MAX_COMMAND_DELTA_CONTENT_CHARS),
        ))
    }

    fn append_delta_record(&mut self, content: String) -> Result<(), String> {
        let bytes = content.len();
        self.sequence += 1;
        self.context
            .append_agent_event(&AgentEvent::command_delta(
                self.command_id.clone(),
                "status",
                content.clone(),
                self.sequence,
            ))
            .map_err(|error| error.to_string())?;
        self.context.renderer.command_delta(&content);
        self.persisted_delta_bytes += bytes;
        self.persisted_delta_events += 1;
        Ok(())
    }

    fn finish(&self, status: CommandStatus, summary: impl AsRef<str>) -> Result<(), String> {
        let summary = bounded_text(summary.as_ref(), MAX_COMMAND_SUMMARY_CHARS);
        self.context
            .append_agent_event(&AgentEvent::command_finished(
                self.command_id.clone(),
                status,
                summary.clone(),
            ))
            .map_err(|error| error.to_string())?;
        self.context.renderer.command_finished(status, &summary);
        Ok(())
    }
}

fn command_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("git-commit-{nanos}")
}

fn working_directory() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
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

    let model_name = context.model.runtime().model.clone();
    let system_prompt = COMMIT_SYSTEM_PROMPT.to_owned();

    let config = AgentConfig {
        system_prompt,
        model: Some(model_name),
        require_usage_metadata: false,
        ..AgentConfig::default()
    };

    let agent = Arc::new(Agent::with_config(provider, config));
    let mut stream = agent.run_stream(prompt);
    let mut message = String::new();

    while let Some(event) = stream.next().await {
        match event {
            AgentEvent::MessageDelta { content, .. } => message.push_str(&content),
            AgentEvent::Finished { .. } => break,
            AgentEvent::Error { message: err } => {
                return Err(CommitMessageGenerationError::Failed(format!(
                    "agent error: {}",
                    err
                )));
            }
            AgentEvent::Cancelled { reason } => {
                return Err(CommitMessageGenerationError::Cancelled(reason));
            }
            _ => {}
        }
    }

    Ok(sanitize_commit_message(&message))
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
    let mut sanitized = message.to_owned();

    // Remove markdown code fences that some models wrap output in
    sanitized = sanitized.replace("```", "");

    // Remove common conversational prefixes
    let lower = sanitized.to_lowercase();
    if let Some(pos) = lower.find("here is ") {
        if pos < 60 {
            if let Some(colon) = sanitized[pos..].find(':') {
                sanitized = sanitized[pos + colon + 1..].to_owned();
            }
        }
    }

    // Lines like 'Commit message:' or 'Suggested message:'
    let mut lines = sanitized.lines();
    if let Some(first_line) = lines.clone().next() {
        let lower_first = first_line.to_lowercase();
        if lower_first.contains("message") && first_line.ends_with(':')
            || lower_first.contains("here's")
            || lower_first.contains("here is")
        {
            lines.next();
        }
    }

    sanitized = lines
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    // Final cleanup
    sanitized
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
