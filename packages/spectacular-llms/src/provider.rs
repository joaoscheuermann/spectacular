use crate::registry::ProviderMetadata;
use crate::types::{
    Cancellation, ProviderCall, ProviderCallFlags, ProviderCapabilities, ProviderError,
    ProviderMessage, ToolManifest, ValidationMode,
};

/// Provider capability used by setup flows and agent runs.
pub trait LlmProvider: Send + Sync {
    fn metadata(&self) -> ProviderMetadata;

    fn validate(&self, mode: ValidationMode, value: &str) -> Result<(), ProviderError>;

    fn models(&self, api_key: &str) -> Result<Vec<Model>, ProviderError>;

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

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
}

impl Model {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
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
    pub fn new(messages: Vec<ProviderMessage>) -> Self {
        Self {
            model: None,
            messages,
            tools: Vec::new(),
            capabilities: ProviderCapabilities::default(),
            flags: ProviderCallFlags::default(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolManifest>) -> Self {
        self.tools = tools;
        self
    }
}
