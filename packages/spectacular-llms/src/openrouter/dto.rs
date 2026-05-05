use crate::{
    ProviderError, ProviderMessage, ProviderMessageRole, ProviderRequest, ProviderToolCall,
    ToolManifest, UsageMetadata,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct OpenRouterChatRequest {
    pub(crate) model: String,
    pub(crate) messages: Vec<OpenRouterChatMessage>,
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<OpenRouterReasoningRequest>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tools: Vec<OpenRouterToolManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parallel_tool_calls: Option<bool>,
}

impl OpenRouterChatRequest {
    pub(crate) fn from_provider_request(request: ProviderRequest) -> Result<Self, ProviderError> {
        let ProviderRequest {
            model,
            messages,
            tools,
            flags,
            ..
        } = request;
        let model = model
            .filter(|model| !model.trim().is_empty())
            .ok_or_else(|| ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: "missing model for chat completion".to_owned(),
            })?;
        let tools = tools
            .into_iter()
            .map(OpenRouterToolManifest::from_tool_manifest)
            .collect::<Vec<_>>();
        let parallel_tool_calls = if tools.is_empty() { None } else { Some(false) };
        let reasoning = flags
            .reasoning_effort
            .filter(|effort| !effort.trim().is_empty())
            .map(|effort| OpenRouterReasoningRequest { effort });

        Ok(Self {
            model,
            messages: messages
                .into_iter()
                .map(OpenRouterChatMessage::from_provider_message)
                .collect(),
            stream: flags.stream,
            reasoning,
            tools,
            parallel_tool_calls,
        })
    }
}

#[derive(Serialize)]
pub(crate) struct OpenRouterReasoningRequest {
    pub(crate) effort: String,
}

#[derive(Serialize)]
pub(crate) struct OpenRouterChatMessage {
    pub(crate) role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tool_calls: Vec<OpenRouterAssistantToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
}

impl OpenRouterChatMessage {
    fn from_provider_message(message: ProviderMessage) -> Self {
        let role = match message.role {
            ProviderMessageRole::System => "system",
            ProviderMessageRole::User => "user",
            ProviderMessageRole::Assistant => "assistant",
            ProviderMessageRole::Tool => "tool",
        };
        let tool_calls = message
            .tool_calls
            .into_iter()
            .map(OpenRouterAssistantToolCall::from_provider_tool_call)
            .collect::<Vec<_>>();
        let content = if role == "assistant" && !tool_calls.is_empty() && message.content.is_empty()
        {
            None
        } else {
            Some(message.content)
        };

        Self {
            role,
            content,
            tool_calls,
            tool_call_id: message.tool_call_id,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct OpenRouterToolManifest {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) function: OpenRouterFunctionManifest,
}

impl OpenRouterToolManifest {
    fn from_tool_manifest(manifest: ToolManifest) -> Self {
        Self {
            kind: "function",
            function: OpenRouterFunctionManifest {
                name: manifest.name,
                description: manifest.description,
                parameters: manifest.parameters,
                strict: true,
            },
        }
    }
}

#[derive(Serialize)]
pub(crate) struct OpenRouterFunctionManifest {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
    pub(crate) strict: bool,
}

#[derive(Serialize)]
pub(crate) struct OpenRouterAssistantToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) function: OpenRouterAssistantToolCallFunction,
}

impl OpenRouterAssistantToolCall {
    fn from_provider_tool_call(tool_call: ProviderToolCall) -> Self {
        Self {
            id: tool_call.id,
            kind: "function",
            function: OpenRouterAssistantToolCallFunction {
                name: tool_call.name,
                arguments: tool_call.arguments,
            },
        }
    }
}

#[derive(Serialize)]
pub(crate) struct OpenRouterAssistantToolCallFunction {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatChunk {
    #[serde(default)]
    pub(crate) choices: Vec<OpenRouterChatChoice>,
    pub(crate) usage: Option<UsageMetadata>,
    pub(crate) error: Option<OpenRouterStreamError>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterStreamError {
    pub(crate) code: Option<serde_json::Value>,
    pub(crate) message: String,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatChoice {
    pub(crate) delta: Option<OpenRouterChatDelta>,
    pub(crate) message: Option<OpenRouterChatChoiceMessage>,
    pub(crate) finish_reason: Option<String>,
    pub(crate) native_finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatDelta {
    pub(crate) content: Option<String>,
    pub(crate) reasoning: Option<String>,
    pub(crate) refusal: Option<String>,
    pub(crate) tool_calls: Option<Vec<OpenRouterChatDeltaToolCall>>,
    pub(crate) finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatChoiceMessage {
    pub(crate) content: Option<String>,
    pub(crate) reasoning: Option<String>,
    pub(crate) refusal: Option<String>,
    pub(crate) tool_calls: Option<Vec<OpenRouterChatMessageToolCall>>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatMessageToolCall {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) function: OpenRouterChatMessageToolCallFunction,
}

impl OpenRouterChatMessageToolCall {
    pub(crate) fn into_provider_tool_call(
        self,
        index: usize,
    ) -> Result<ProviderToolCall, ProviderError> {
        if self.kind != "function" {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("unsupported tool-call type `{}`", self.kind),
            });
        }

        if self.id.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted id"),
            });
        }

        if self.function.name.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function name"),
            });
        }

        Ok(ProviderToolCall::new(
            self.id,
            self.function.name,
            self.function.arguments,
        ))
    }
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatMessageToolCallFunction {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatDeltaToolCall {
    pub(crate) index: usize,
    pub(crate) id: Option<String>,
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    pub(crate) function: Option<OpenRouterChatDeltaToolCallFunction>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterChatDeltaToolCallFunction {
    pub(crate) name: Option<String>,
    pub(crate) arguments: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterModelsResponse {
    pub(crate) data: Vec<OpenRouterModelResponse>,
}

#[derive(Deserialize)]
pub(crate) struct OpenRouterModelResponse {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
}
