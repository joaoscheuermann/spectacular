use super::{ProviderToolCall, ReasoningMetadata};

/// Chat message sent to provider completion calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: String,
    pub tool_calls: Vec<ProviderToolCall>,
    pub tool_call_id: Option<String>,
}

impl ProviderMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_call(tool_call: ProviderToolCall) -> Self {
        Self::assistant_tool_calls(vec![tool_call])
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ProviderToolCall>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: String::new(),
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Incremental assistant content returned by provider streams.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageDelta {
    pub role: ProviderMessageRole,
    pub content: String,
}

impl MessageDelta {
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Incremental reasoning content returned by providers that expose it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningDelta {
    pub content: String,
    pub metadata: Option<ReasoningMetadata>,
}
