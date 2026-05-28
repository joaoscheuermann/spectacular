use crate::chat::RuntimeSelection;

const CODING_AGENT_SYSTEM_PROMPT: &str = include_str!("prompt/coding-agent.md");
use ::agent::{Agent, AgentConfig, ContextPolicy, Store, ToolRegistrationError, ToolStorage};
use ::config::ReasoningLevel;
use ::llms::LlmProvider;
use std::path::PathBuf;

/// Builds the built-in tool storage scoped to the injected workspace root.
pub fn main_chat_tool_storage(
    workspace_root: impl Into<PathBuf>,
    trace_dir: impl Into<PathBuf>,
) -> Result<ToolStorage, ToolRegistrationError> {
    tools::built_in_tools_with_trace_dir(workspace_root, trace_dir)
}

/// Creates the main coding agent configured with runtime model, reasoning, and tools.
pub(crate) fn main_chat_agent<P>(
    provider: P,
    runtime: &RuntimeSelection,
    store: Store,
    tools: ToolStorage,
) -> Agent<P>
where
    P: LlmProvider,
{
    let reasoning_effort = runtime_reasoning_effort(runtime.reasoning);
    let context_window_tokens = runtime_context_window_tokens(&provider, runtime);
    let config = AgentConfig {
        model: Some(runtime.model.clone()),
        require_usage_metadata: false,
        include_reasoning: reasoning_effort.is_some(),
        reasoning_effort,
        system_prompt: CODING_AGENT_SYSTEM_PROMPT.to_string(),
        context_policy: context_policy_for_runtime(runtime, context_window_tokens),
        ..AgentConfig::default()
    };

    Agent::with_config_and_store(provider, config, store).with_tools(tools)
}

/// Resolves the context window from cached runtime metadata before consulting the provider.
fn runtime_context_window_tokens<P>(provider: &P, runtime: &RuntimeSelection) -> Option<usize>
where
    P: LlmProvider,
{
    runtime
        .context_window_tokens
        .or_else(|| provider.context_window_tokens(&runtime.model))
}

/// Builds the context policy used by normal chat runs for the selected runtime model.
fn context_policy_for_runtime(
    runtime: &RuntimeSelection,
    context_window_tokens: Option<usize>,
) -> ContextPolicy {
    ContextPolicy {
        model_context_window_tokens: context_window_tokens,
        reasoning_reserve_tokens: reasoning_reserve_tokens(runtime.reasoning),
        max_summary_passes_per_request: 4,
        ..ContextPolicy::default()
    }
}

/// Reserves additional input budget for models configured to spend reasoning tokens.
fn reasoning_reserve_tokens(reasoning: ReasoningLevel) -> usize {
    if reasoning.non_none() {
        return 8_192;
    }

    0
}

/// Converts a configured reasoning level into an optional provider effort string.
fn runtime_reasoning_effort(reasoning: ReasoningLevel) -> Option<String> {
    match reasoning {
        ReasoningLevel::None => None,
        level => Some(level.as_str().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/runner.rs"
    ));
}
