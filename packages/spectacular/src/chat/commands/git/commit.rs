//! `/git-commit` command implementation (legacy).
//!
//! Generates a conventional commit message using a standalone AI agent
//! and commits the currently staged changes.

use crate::chat::commands::{
    ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};

use crate::chat::provider::provider_for_runtime;
use spectacular_agent::{Agent, AgentConfig, AgentEvent};
use spectacular_commands::CommandError;
use spectacular_llms::ProviderMessageRole;
use std::sync::Arc;

use super::helpers;

/// Maximum diff length sent to the agent. Diffs larger than this are truncated.
const MAX_DIFF_CHARS: usize = 15_000;
const TRUNCATED_DIFF_NOTICE: &str =
    "warning: diff is large and has been truncated for the commit message agent";

/// System prompt for commit message generation
const COMMIT_SYSTEM_PROMPT: &str = include_str!("prompt/commit-system.md");

/// User prompt template for commit message generation
const COMMIT_USER_PROMPT_TEMPLATE: &str = include_str!("prompt/commit-user.md");

/// Internal execute function that can be called from the parent git command
pub fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/git commit").to_string());
        }

        // 1. Check for staged changes
        let has_staged = match context
            .work(async { helpers::has_staged_changes().await })
            .await
        {
            Ok(v) => v,
            Err(e) => return ChatCommandResult::error(e.to_string()),
        };
        if !has_staged {
            return ChatCommandResult::error(
                "no staged changes to commit. Use `git add` to stage changes first.".to_string(),
            );
        }

        // 2. Get the staged diff
        let diff = match context
            .work(async { helpers::get_staged_diff().await })
            .await
        {
            Ok(v) => v,
            Err(e) => return ChatCommandResult::error(e.to_string()),
        };

        // 3. Truncate diff if needed
        let (diff_for_prompt, truncated) = truncate_diff_if_needed(&diff);
        if truncated {
            context.notice(TRUNCATED_DIFF_NOTICE);
        }

        // 4. Build prompt
        let prompt = build_commit_prompt(&diff_for_prompt);

        // 5. Generate commit message using standalone agent
        let commit_message = match generate_commit_message_with_work(&context, prompt).await {
            Ok(msg) => msg,
            Err(e) => {
                return ChatCommandResult::error(format!(
                    "failed to generate commit message: {}",
                    e
                ))
            }
        };

        if commit_message.trim().is_empty() {
            return ChatCommandResult::error(
                "generated commit message is empty. Please commit manually.".to_string(),
            );
        }

        // 6. Show generated message
        context.success("generated commit message:");
        context.notice(&format!(
            "  {}",
            commit_message.lines().next().unwrap_or("")
        ));
        if commit_message.lines().count() > 1 {
            for line in commit_message.lines().skip(1) {
                context.notice(&format!("  {}", line));
            }
        }

        // 7. Commit
        match context
            .work(async { helpers::commit_with_message(&commit_message).await })
            .await
        {
            Ok(output) => {
                context.success("changes committed successfully!");
                if !output.trim().is_empty() {
                    context.notice(&output);
                }
                ChatCommandResult::success()
            }
            Err(e) => ChatCommandResult::error(format!("commit failed: {}", e)),
        }
    })
}

async fn generate_commit_message_with_work(
    context: &ChatCommandContext<'_>,
    prompt: String,
) -> Result<String, String> {
    context
        .work(async { generate_commit_message(context, prompt).await })
        .await
}

async fn generate_commit_message(
    context: &ChatCommandContext<'_>,
    prompt: String,
) -> Result<String, String> {
    let provider = provider_for_runtime(
        context.model.runtime(),
        context.model.debug_logger().clone(),
    )
    .map_err(|e| format!("provider error: {}", e))?;

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
            AgentEvent::MessageDelta(delta) => {
                if delta.role == ProviderMessageRole::Assistant {
                    message.push_str(&delta.content);
                }
            }
            AgentEvent::Finished { .. } => break,
            AgentEvent::Error { message: err } => {
                return Err(format!("agent error: {}", err));
            }
            AgentEvent::Cancelled { reason } => {
                return Err(format!("agent cancelled: {}", reason));
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
