//! `/git-commit` command implementation.
//!
//! Generates a conventional commit message using a standalone AI agent
//! and commits the currently staged changes.

use crate::chat::commands::{
    ChatCommand, ChatCommandContext, ChatCommandFuture, ChatCommandResult,
};
use crate::chat::provider::provider_for_runtime;
use spectacular_agent::{Agent, AgentConfig, AgentEvent};
use spectacular_commands::CommandError;
use spectacular_llms::ProviderMessageRole;
use std::sync::Arc;

use super::helpers;

/// Maximum diff length sent to the agent. Diffs larger than this are truncated.
const MAX_DIFF_CHARS: usize = 15_000;

pub fn command() -> ChatCommand {
    ChatCommand {
        name: "git-commit",
        usage: "/commit",
        summary: "Generate a conventional commit message and commit staged changes",
        execute,
    }
}

fn execute<'a>(context: ChatCommandContext<'a>, args: Vec<String>) -> ChatCommandFuture<'a> {
    Box::pin(async move {
        if !args.is_empty() {
            return ChatCommandResult::error(CommandError::usage("/commit").to_string());
        }

        // 1. Check for staged changes
        context.notice("checking for staged changes...");
        let has_staged = match helpers::has_staged_changes().await {
            Ok(v) => v,
            Err(e) => return ChatCommandResult::error(e.to_string()),
        };
        if !has_staged {
            return ChatCommandResult::error(
                "no staged changes to commit. Use `git add` to stage changes first.".to_string(),
            );
        }

        // 2. Get the staged diff
        context.notice("reading staged changes...");
        let diff = match helpers::get_staged_diff().await {
            Ok(v) => v,
            Err(e) => return ChatCommandResult::error(e.to_string()),
        };

        // 3. Truncate diff if needed
        let (diff_for_prompt, truncated) = truncate_diff_if_needed(&diff);
        if truncated {
            context.notice("⚠ diff is large and has been truncated for the commit message agent");
        }

        // 4. Build prompt
        let prompt = build_commit_prompt(&diff_for_prompt);

        // 5. Generate commit message using standalone agent
        context.notice("generating commit message...");
        let commit_message = match generate_commit_message(&context, prompt).await {
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
        context.notice("committing changes...");
        match helpers::commit_with_message(&commit_message).await {
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

// ─────────────────────────────────────────────────────────────────────────────
// Commit message generation
// ─────────────────────────────────────────────────────────────────────────────

async fn generate_commit_message(
    context: &ChatCommandContext<'_>,
    prompt: String,
) -> Result<String, String> {
    let provider = provider_for_runtime(context.model.runtime())
        .map_err(|e| format!("provider error: {}", e))?;

    let model_name = context.model.runtime().model.clone();
    let system_prompt = r#"You are a git commit message generator that follows the Conventional Commits specification.

Given a staged git diff, generate an appropriate conventional commit message.

Rules:
- Format: <type>(<scope>): <description>
- Types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert
- Scope: optional, derived from the affected module/component
- Description: imperative mood ("add" not "added"), lowercase first letter, no trailing period, max 50 chars
- Body: optional, explain WHAT and WHY (not HOW), wrap at 72 chars
- Breaking changes: prefix body with "BREAKING CHANGE: " if applicable
- Return ONLY the commit message. No explanations, no markdown formatting, no quotes."#
        .to_owned();

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

// ─────────────────────────────────────────────────────────────────────────────
// Prompt building
// ─────────────────────────────────────────────────────────────────────────────

fn build_commit_prompt(diff: &str) -> String {
    format!(
        r#"Analyze the following staged git diff and generate a conventional commit message:

```diff
{diff}
```

Consider:
- What files were changed?
- What is the nature of the changes?
- Is this a feature, fix, refactor, chore, etc.?
- Are there any breaking changes?

Return only the commit message."#,
        diff = diff
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Diff truncation
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Message sanitization
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_markdown_fences() {
        let input = "```\nfix(auth): resolve token issue\n```";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "fix(auth): resolve token issue");
    }

    #[test]
    fn sanitize_removes_conversational_prefix() {
        let input = "Here is the commit message:\n\nfeat: add user authentication";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "feat: add user authentication");
    }

    #[test]
    fn sanitize_removes_commit_message_label() {
        let input = "Commit message:\nfix: handle null pointer";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "fix: handle null pointer");
    }

    #[test]
    fn sanitize_trims_and_removes_quotes() {
        let input = "  \"feat: add new feature\"  ";
        let result = sanitize_commit_message(input);
        assert_eq!(result, "feat: add new feature");
    }

    #[test]
    fn truncate_diff_keeps_small_diffs_intact() {
        let diff = "small diff content";
        let (result, truncated) = truncate_diff_if_needed(diff);
        assert!(!truncated);
        assert_eq!(result, diff);
    }

    #[test]
    fn truncate_diff_truncates_large_diffs() {
        let diff = "x".repeat(MAX_DIFF_CHARS + 1000);
        let (result, truncated) = truncate_diff_if_needed(&diff);
        assert!(truncated);
        assert!(result.contains("[diff truncated"));
        assert!(result.len() < diff.len());
    }

    #[test]
    fn prompt_includes_diff_content() {
        let diff = "+ let x = 1;";
        let prompt = build_commit_prompt(diff);
        assert!(prompt.contains(diff));
        assert!(prompt.contains("conventional commit message"));
    }
}
