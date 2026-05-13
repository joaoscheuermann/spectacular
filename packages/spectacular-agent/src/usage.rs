/// Compact token usage for the provider context assembled for a run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextTokenUsage {
    /// Estimated provider-visible input tokens in the assembled context.
    pub input_tokens: u64,
    /// Context window for the selected model, when known.
    pub context_window_tokens: Option<u64>,
}
