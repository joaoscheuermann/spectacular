use super::recorder::RunRecorder;
use super::Agent;
use crate::context::{ContextDiagnostics, ContextPolicy, TokenCounter};
use crate::error::AgentError;
use crate::event::AgentEvent;
use crate::usage::ContextTokenUsage;
use spectacular_llms::LlmProvider;

/// Builds and emits compact context token usage for the current run.
pub(super) async fn record_context_token_usage<P, C>(
    agent: &Agent<P, C>,
    recorder: &mut RunRecorder<'_, P, C>,
    diagnostics: &ContextDiagnostics,
) -> Result<(), AgentError>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    let usage = context_token_usage(diagnostics, &agent.config.context_policy);
    recorder.record(AgentEvent::ContextTokenUsage(usage)).await
}

/// Converts context diagnostics and policy into the UI-facing token usage DTO.
fn context_token_usage(
    diagnostics: &ContextDiagnostics,
    policy: &ContextPolicy,
) -> ContextTokenUsage {
    ContextTokenUsage {
        input_tokens: diagnostics.total_input_tokens as u64,
        context_window_tokens: policy
            .model_context_window_tokens
            .map(|tokens| tokens as u64),
    }
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/agent/context_usage.rs"
    ));
}
