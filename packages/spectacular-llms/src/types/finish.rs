use super::ProviderToolCall;
use serde::Deserialize;

/// Provider stream terminal reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Cancelled,
    ContentFilter,
    Error,
}

/// Token usage metadata returned when a provider exposes it.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct UsageMetadata {
    #[serde(alias = "prompt_tokens")]
    pub input_tokens: Option<u64>,
    #[serde(alias = "completion_tokens")]
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

/// Reasoning metadata returned when a provider exposes it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReasoningMetadata {
    pub effort: Option<String>,
    pub summary: Option<String>,
}

/// Terminal provider event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFinished {
    pub finish_reason: FinishReason,
    pub tool_calls: Vec<ProviderToolCall>,
    pub usage: Option<UsageMetadata>,
    pub reasoning: Option<ReasoningMetadata>,
}

impl ProviderFinished {
    pub fn stopped() -> Self {
        Self {
            finish_reason: FinishReason::Stop,
            tool_calls: Vec::new(),
            usage: None,
            reasoning: None,
        }
    }

    pub fn tool_calls(tool_calls: Vec<ProviderToolCall>) -> Self {
        Self {
            finish_reason: FinishReason::ToolCalls,
            tool_calls,
            usage: None,
            reasoning: None,
        }
    }
}
