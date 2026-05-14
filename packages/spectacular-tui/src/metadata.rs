use serde::{Deserialize, Serialize};

/// Runtime policy input selected by configuration or session commands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeSelection {
    pub provider_type: String,
    pub provider: String,
    pub model: String,
    pub reasoning: ReasoningLevel,
    pub context_window_tokens: Option<u64>,
}

impl RuntimeSelection {
    /// Creates a runtime selection projection for the TUI state model.
    pub fn new(
        provider_type: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
        reasoning: ReasoningLevel,
        context_window_tokens: Option<u64>,
    ) -> Self {
        Self {
            provider_type: provider_type.into(),
            provider: provider.into(),
            model: model.into(),
            reasoning,
            context_window_tokens,
        }
    }
}

/// Reasoning effort displayed in the header/footer metadata.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ReasoningLevel {
    None,
    Low,
    Medium,
    High,
}

/// UI-safe header/footer metadata derived outside rendering components.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplayMetadata {
    pub provider_label: String,
    pub model_label: String,
    pub reasoning_label: String,
    pub current_directory: String,
    pub session_label: String,
    pub usage: Option<ContextTokenUsage>,
}

impl DisplayMetadata {
    /// Creates visible metadata for header and footer components.
    pub fn new(
        provider_label: impl Into<String>,
        model_label: impl Into<String>,
        reasoning_label: impl Into<String>,
        current_directory: impl Into<String>,
        session_label: impl Into<String>,
        usage: Option<ContextTokenUsage>,
    ) -> Self {
        Self {
            provider_label: provider_label.into(),
            model_label: model_label.into(),
            reasoning_label: reasoning_label.into(),
            current_directory: current_directory.into(),
            session_label: session_label.into(),
            usage,
        }
    }
}

/// Compact token usage displayed by the TUI without querying runtime state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextTokenUsage {
    pub input_tokens: u64,
    pub context_window_tokens: Option<u64>,
}

impl ContextTokenUsage {
    /// Creates token usage metadata from runtime context assembly results.
    pub fn new(input_tokens: u64, context_window_tokens: Option<u64>) -> Self {
        Self {
            input_tokens,
            context_window_tokens,
        }
    }
}

/// Command metadata used by prompt and command palette UI surfaces.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandDescriptor {
    pub name: String,
    pub summary: String,
}

impl CommandDescriptor {
    /// Creates a command descriptor from command registry metadata.
    pub fn new(name: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            summary: summary.into(),
        }
    }
}
