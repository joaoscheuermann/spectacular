/// Provider-visible function tool schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolManifest {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolManifest {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// Tool call requested by a provider before a `ToolCalls` finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl ProviderToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}
