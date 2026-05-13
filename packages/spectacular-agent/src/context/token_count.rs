use spectacular_llms::ProviderMessage;
use std::fmt;

const DEFAULT_TIKTOKEN_ENCODING: &str = "o200k_base";

/// Counts provider-visible tokens for context budgeting.
pub trait TokenCounter {
    /// Counts estimated tokens for arbitrary text content.
    fn count_text_tokens(&self, text: &str) -> usize;

    /// Counts estimated tokens for a full provider message including metadata.
    fn count_message_tokens(&self, message: &ProviderMessage) -> usize;
}

/// Default token estimator using tiktoken with approximate fallback support.
#[derive(Clone, Copy, Debug)]
pub enum TokenCounterChoice {
    /// Counts tokens with a tiktoken encoding.
    Tiktoken(TiktokenTokenCounter),
    /// Counts tokens with the repository fallback heuristic.
    Approximate(ApproximateTokenCounter),
}

impl TokenCounterChoice {
    /// Creates the default token counter for an optional provider model identifier.
    pub fn for_model(model: Option<&str>) -> Self {
        model
            .and_then(TiktokenTokenCounter::for_model)
            .or_else(|| TiktokenTokenCounter::for_encoding(DEFAULT_TIKTOKEN_ENCODING))
            .map(TokenCounterChoice::Tiktoken)
            .unwrap_or(TokenCounterChoice::Approximate(ApproximateTokenCounter))
    }
}

impl Default for TokenCounterChoice {
    /// Creates the default tiktoken-backed counter using a modern OpenAI encoding.
    fn default() -> Self {
        Self::for_model(None)
    }
}

impl TokenCounter for TokenCounterChoice {
    /// Counts estimated text tokens using the selected counter implementation.
    fn count_text_tokens(&self, text: &str) -> usize {
        match self {
            TokenCounterChoice::Tiktoken(counter) => counter.count_text_tokens(text),
            TokenCounterChoice::Approximate(counter) => counter.count_text_tokens(text),
        }
    }

    /// Counts estimated message tokens using the selected counter implementation.
    fn count_message_tokens(&self, message: &ProviderMessage) -> usize {
        match self {
            TokenCounterChoice::Tiktoken(counter) => counter.count_message_tokens(message),
            TokenCounterChoice::Approximate(counter) => counter.count_message_tokens(message),
        }
    }
}

/// Tiktoken-backed local token estimator.
#[derive(Clone, Copy)]
pub struct TiktokenTokenCounter {
    encoding: &'static tiktoken::CoreBpe,
}

impl TiktokenTokenCounter {
    /// Creates a tiktoken counter for a known model identifier.
    pub fn for_model(model: &str) -> Option<Self> {
        tiktoken::encoding_for_model(model).map(|encoding| Self { encoding })
    }

    /// Creates a tiktoken counter for an explicit encoding name.
    pub fn for_encoding(encoding: &str) -> Option<Self> {
        tiktoken::get_encoding(encoding).map(|encoding| Self { encoding })
    }
}

impl fmt::Debug for TiktokenTokenCounter {
    /// Formats the counter without exposing the tokenizer internals.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TiktokenTokenCounter")
            .finish_non_exhaustive()
    }
}

impl TokenCounter for TiktokenTokenCounter {
    /// Counts text tokens with the configured tiktoken encoding.
    fn count_text_tokens(&self, text: &str) -> usize {
        self.encoding.count(text)
    }

    /// Counts message tokens including tool metadata and explicit per-message overhead.
    fn count_message_tokens(&self, message: &ProviderMessage) -> usize {
        message_token_count(self, message)
    }
}

/// Conservative local token estimator used when tiktoken is unavailable.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApproximateTokenCounter;

impl TokenCounter for ApproximateTokenCounter {
    /// Estimates text tokens using a rounded-up four-character heuristic.
    fn count_text_tokens(&self, text: &str) -> usize {
        approximate_text_tokens(text)
    }

    /// Estimates message tokens including tool call metadata and per-message overhead.
    fn count_message_tokens(&self, message: &ProviderMessage) -> usize {
        message_token_count(self, message)
    }
}

/// Counts message text, tool metadata, and conservative chat wrapper overhead.
fn message_token_count(counter: &impl TokenCounter, message: &ProviderMessage) -> usize {
    let tool_call_tokens = message
        .tool_calls
        .iter()
        .map(|tool_call| {
            counter.count_text_tokens(&tool_call.id)
                + counter.count_text_tokens(&tool_call.name)
                + counter.count_text_tokens(&tool_call.arguments)
        })
        .sum::<usize>();
    let tool_result_tokens = message
        .tool_call_id
        .as_deref()
        .map(|tool_call_id| counter.count_text_tokens(tool_call_id))
        .unwrap_or_default();

    4 + counter.count_text_tokens(&message.content) + tool_call_tokens + tool_result_tokens
}

/// Estimates text tokens using the repository fallback chars-over-four heuristic.
pub(crate) fn approximate_text_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    if chars == 0 {
        return 0;
    }

    chars.div_ceil(4)
}
