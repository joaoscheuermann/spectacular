use spectacular_llms::ProviderMessage;

/// Counts provider-visible tokens for context budgeting.
pub trait TokenCounter {
    /// Counts estimated tokens for arbitrary text content.
    fn count_text_tokens(&self, text: &str) -> usize;

    /// Counts estimated tokens for a full provider message including metadata.
    fn count_message_tokens(&self, message: &ProviderMessage) -> usize;
}

/// Conservative local token estimator used until provider-specific counters exist.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApproximateTokenCounter;

impl TokenCounter for ApproximateTokenCounter {
    /// Estimates text tokens using a rounded-up four-character heuristic.
    fn count_text_tokens(&self, text: &str) -> usize {
        let chars = text.chars().count();
        if chars == 0 {
            return 0;
        }

        chars.div_ceil(4)
    }

    /// Estimates message tokens including tool call metadata and per-message overhead.
    fn count_message_tokens(&self, message: &ProviderMessage) -> usize {
        let tool_call_tokens = message
            .tool_calls
            .iter()
            .map(|tool_call| {
                self.count_text_tokens(&tool_call.id)
                    + self.count_text_tokens(&tool_call.name)
                    + self.count_text_tokens(&tool_call.arguments)
            })
            .sum::<usize>();
        let tool_result_tokens = message
            .tool_call_id
            .as_deref()
            .map(|tool_call_id| self.count_text_tokens(tool_call_id))
            .unwrap_or_default();

        4 + self.count_text_tokens(&message.content) + tool_call_tokens + tool_result_tokens
    }
}
