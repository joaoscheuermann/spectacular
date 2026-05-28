use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// UI-safe Git worktree metadata derived outside rendering components.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorktreeMetadata {
    pub label: String,
}

impl WorktreeMetadata {
    /// Creates display-safe worktree metadata for footer rendering.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

/// UI-safe header/footer metadata derived outside rendering components.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplayMetadata {
    pub provider_label: String,
    pub model_label: String,
    pub reasoning_label: String,
    pub current_directory: String,
    pub session_label: String,
    pub context_usage: Option<ContextTokenUsage>,
    pub turn_usage: Option<TurnTokenUsage>,
    pub total_usage: Option<TokenUsageTotal>,
    #[serde(default)]
    pub worktree: Option<WorktreeMetadata>,
}

impl DisplayMetadata {
    /// Creates visible metadata for header and footer components.
    pub fn new(
        provider_label: impl Into<String>,
        model_label: impl Into<String>,
        reasoning_label: impl Into<String>,
        current_directory: impl Into<String>,
        session_label: impl Into<String>,
        context_usage: Option<ContextTokenUsage>,
    ) -> Self {
        Self {
            provider_label: provider_label.into(),
            model_label: model_label.into(),
            reasoning_label: reasoning_label.into(),
            current_directory: current_directory.into(),
            session_label: session_label.into(),
            context_usage,
            turn_usage: None,
            total_usage: None,
            worktree: None,
        }
    }
}

/// Estimated token usage for the provider-visible context assembled for a run.
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

    /// Creates default usage when only the runtime context window is known.
    pub fn default_for_window(context_window_tokens: Option<u64>) -> Option<Self> {
        context_window_tokens.map(|tokens| Self::new(0, Some(tokens)))
    }
}

/// One provider-reported token usage payload from a terminal provider response.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderUsageMetadata {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

impl ProviderUsageMetadata {
    /// Creates provider usage metadata while preserving provider field optionality.
    pub fn new(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens,
        }
    }
}

/// Accumulated provider-reported token consumption.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TokenUsageTotal {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub has_provider_metadata: bool,
}

impl TokenUsageTotal {
    /// Adds one provider-reported usage payload using saturating arithmetic.
    pub fn record_provider_usage(&mut self, reported: ProviderUsageMetadata) {
        self.has_provider_metadata = true;

        if let Some(input_tokens) = reported.input_tokens {
            self.input_tokens = self.input_tokens.saturating_add(input_tokens);
        }
        if let Some(output_tokens) = reported.output_tokens {
            self.output_tokens = self.output_tokens.saturating_add(output_tokens);
        }
        if let Some(total_tokens) = reported.total_tokens {
            self.total_tokens = self.total_tokens.saturating_add(total_tokens);
            return;
        }

        if let (Some(input_tokens), Some(output_tokens)) =
            (reported.input_tokens, reported.output_tokens)
        {
            self.total_tokens = self
                .total_tokens
                .saturating_add(input_tokens.saturating_add(output_tokens));
        }
    }
}

/// Accumulated provider-reported token consumption for the active agent turn.
pub type TurnTokenUsage = TokenUsageTotal;

/// Command metadata used by prompt and command palette UI surfaces.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandDescriptor {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub usage: String,
    #[serde(default)]
    pub subcommands: Vec<CommandSubcommandDescriptor>,
}

impl CommandDescriptor {
    /// Creates a command descriptor from command registry metadata.
    pub fn new(name: impl Into<String>, summary: impl Into<String>) -> Self {
        Self::with_usage(name, summary, "")
    }

    /// Creates a command descriptor with usage guidance from command registry metadata.
    pub fn with_usage(
        name: impl Into<String>,
        summary: impl Into<String>,
        usage: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            summary: summary.into(),
            usage: usage.into(),
            subcommands: Vec::new(),
        }
    }

    /// Returns this descriptor with structured completion metadata attached.
    pub fn with_subcommands(
        mut self,
        subcommands: impl IntoIterator<Item = CommandSubcommandDescriptor>,
    ) -> Self {
        self.subcommands = subcommands.into_iter().collect();
        self
    }
}

/// UI-safe subcommand completion metadata for a slash command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandSubcommandDescriptor {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub fields: Vec<CommandFieldDescriptor>,
}

impl CommandSubcommandDescriptor {
    /// Creates a subcommand descriptor with structured field metadata.
    pub fn new(
        name: impl Into<String>,
        summary: impl Into<String>,
        fields: impl IntoIterator<Item = CommandFieldDescriptor>,
    ) -> Self {
        Self {
            name: name.into(),
            summary: summary.into(),
            fields: fields.into_iter().collect(),
        }
    }
}

/// UI-safe field completion metadata for a slash subcommand.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandFieldDescriptor {
    pub name: String,
    pub summary: String,
    pub required: bool,
    #[serde(default)]
    pub values: CompletionValues,
    pub validation: CommandValueValidation,
}

impl CommandFieldDescriptor {
    /// Creates a field descriptor for command composition.
    pub fn new(
        name: impl Into<String>,
        summary: impl Into<String>,
        required: bool,
        values: CompletionValues,
        validation: CommandValueValidation,
    ) -> Self {
        Self {
            name: name.into(),
            summary: summary.into(),
            required,
            values,
            validation,
        }
    }
}

/// Describes how command field values are resolved by the reducer-safe composer.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum CompletionValues {
    #[default]
    None,
    Static(Vec<String>),
    ConfiguredProviders(Vec<String>),
    SavedModels(Vec<String>),
    CachedModelIds(CachedModelValues),
    Unavailable(String),
}

/// Snapshot of cached model values needed for provider-scoped model completion.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CachedModelValues {
    pub model_ids_by_provider: BTreeMap<String, Vec<String>>,
    pub saved_model_providers: BTreeMap<String, String>,
}

impl CachedModelValues {
    /// Creates cached model completion values from provider and saved-model maps.
    pub fn new(
        model_ids_by_provider: BTreeMap<String, Vec<String>>,
        saved_model_providers: BTreeMap<String, String>,
    ) -> Self {
        Self {
            model_ids_by_provider,
            saved_model_providers,
        }
    }
}

/// Defines whether a field value must match one of the resolved completion values.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandValueValidation {
    #[default]
    None,
    OneOfValues,
}
