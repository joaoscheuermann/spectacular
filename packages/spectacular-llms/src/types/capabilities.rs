/// Provider capabilities advertised before a call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calls: bool,
    pub structured_output: bool,
    pub reasoning: bool,
    pub cancellation: bool,
    pub usage_metadata: bool,
    pub reasoning_metadata: bool,
    pub context_limits: ProviderContextLimits,
}

/// Provider-advertised context bounds checked before provider I/O.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderContextLimits {
    pub max_messages: Option<usize>,
    pub max_chars: Option<usize>,
}

/// Per-call flags passed to provider implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCallFlags {
    pub stream: bool,
    pub allow_tools: bool,
    pub include_reasoning: bool,
    pub reasoning_effort: Option<String>,
}

impl Default for ProviderCallFlags {
    fn default() -> Self {
        Self {
            stream: true,
            allow_tools: false,
            include_reasoning: false,
            reasoning_effort: None,
        }
    }
}
