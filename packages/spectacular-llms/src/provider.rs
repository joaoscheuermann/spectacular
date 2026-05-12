use crate::registry::ProviderMetadata;
use crate::types::{
    Cancellation, ProviderCall, ProviderCallFlags, ProviderCapabilities, ProviderError,
    ProviderMessage, ToolManifest, ValidationMode,
};

/// Provider capability used by setup flows and agent runs.
pub trait LlmProvider: Send + Sync {
    /// Returns provider metadata for this implementation.
    fn metadata(&self) -> ProviderMetadata;

    /// Validates provider-specific input for the requested validation mode.
    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError>;

    /// Fetches model metadata available to the supplied API key.
    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError>;

    /// Returns provider capabilities advertised by this implementation.
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    /// Resolves the context window for a provider model when the provider can determine it.
    fn context_window_tokens(&self, _model: &str) -> Option<usize> {
        None
    }

    /// Starts a streaming completion request and returns the provider call future.
    fn stream_completion<'a>(
        &'a self,
        request: ProviderRequest,
        cancellation: Cancellation,
    ) -> ProviderCall<'a>;
}

/// Model metadata exposed by a provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Model {
    id: String,
    display_name: String,
    supported_parameters: Vec<String>,
    context_window_tokens: Option<usize>,
}

impl Model {
    /// Creates a new value from the supplied inputs.
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self::with_supported_parameters(id, display_name, Vec::<String>::new())
    }

    /// Returns this value with supported parameters.
    pub fn with_supported_parameters(
        id: impl Into<String>,
        display_name: impl Into<String>,
        supported_parameters: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            supported_parameters: supported_parameters.into_iter().collect(),
            context_window_tokens: None,
        }
    }

    /// Returns this model with provider-reported context-window metadata attached.
    pub fn with_context_window_tokens(mut self, context_window_tokens: Option<usize>) -> Self {
        self.context_window_tokens = context_window_tokens.filter(|tokens| *tokens > 0);
        self
    }

    /// Handles ID for this module.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Handles display name for this module.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Handles supported parameters for this module.
    pub fn supported_parameters(&self) -> &[String] {
        &self.supported_parameters
    }

    /// Returns the provider-reported context window for this model when available.
    pub fn context_window_tokens(&self) -> Option<usize> {
        self.context_window_tokens
    }

    /// Handles supports parameter for this module.
    pub fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|candidate| candidate == parameter)
    }
}

/// Provider completion request consumed by async provider implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRequest {
    pub model: Option<String>,
    pub messages: Vec<ProviderMessage>,
    pub tools: Vec<ToolManifest>,
    pub capabilities: ProviderCapabilities,
    pub flags: ProviderCallFlags,
}

impl ProviderRequest {
    /// Creates a new value from the supplied inputs.
    pub fn new(messages: Vec<ProviderMessage>) -> Self {
        Self {
            model: None,
            messages,
            tools: Vec::new(),
            capabilities: ProviderCapabilities::default(),
            flags: ProviderCallFlags::default(),
        }
    }

    /// Returns this value with model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Returns this value with tools.
    pub fn with_tools(mut self, tools: Vec<ToolManifest>) -> Self {
        self.tools = tools;
        self
    }
}
