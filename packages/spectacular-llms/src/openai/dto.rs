use crate::{
    ProviderError, ProviderMessage, ProviderMessageRole, ProviderRequest, ProviderToolCall,
    ToolManifest, UsageMetadata,
};
use serde::{Deserialize, Serialize};

const REASONING_SUMMARY_AUTO: &str = "auto";
const FAST_MODEL_SUFFIX: &str = "-fast";
const FAST_SERVICE_TIER: &str = "priority";

#[derive(Serialize)]
pub(crate) struct OpenAiResponsesRequest {
    pub(crate) model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) service_tier: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) instructions: Option<String>,
    pub(crate) input: Vec<OpenAiInputItem>,
    pub(crate) store: bool,
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) tools: Vec<OpenAiToolManifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_choice: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<OpenAiReasoningRequest>,
}

impl OpenAiResponsesRequest {
    /// Converts the provider-neutral request into an OpenAI Responses request.
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
                provider_name: "OpenAI".to_owned(),
                reason: "missing model for Responses request".to_owned(),
            })?;
        let model_request = OpenAiModelRequest::from_configured_model(model);
        let instructions = instructions_from_messages(&messages);
        let input = input_from_messages(messages);
        let tools = tools
            .into_iter()
            .map(OpenAiToolManifest::from_tool_manifest)
            .collect::<Vec<_>>();
        let tool_choice = if tools.is_empty() { None } else { Some("auto") };
        let parallel_tool_calls = if tools.is_empty() { None } else { Some(false) };
        let reasoning =
            OpenAiReasoningRequest::from_flags(flags.include_reasoning, flags.reasoning_effort);

        Ok(Self {
            model: model_request.model,
            service_tier: model_request.service_tier,
            instructions,
            input,
            store: false,
            stream: flags.stream,
            tools,
            tool_choice,
            parallel_tool_calls,
            reasoning,
        })
    }
}

struct OpenAiModelRequest {
    model: String,
    service_tier: Option<&'static str>,
}

impl OpenAiModelRequest {
    /// Converts Spectacular's model aliases into the OpenAI Responses wire model.
    fn from_configured_model(model: String) -> Self {
        let Some(base_model) = model.strip_suffix(FAST_MODEL_SUFFIX) else {
            return Self {
                model,
                service_tier: None,
            };
        };

        if base_model.trim().is_empty() {
            return Self {
                model,
                service_tier: None,
            };
        }

        Self {
            model: base_model.to_owned(),
            service_tier: Some(FAST_SERVICE_TIER),
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum OpenAiInputItem {
    Message(OpenAiMessageItem),
    FunctionCall(OpenAiFunctionCallItem),
    FunctionCallOutput(OpenAiFunctionCallOutputItem),
}

#[derive(Serialize)]
pub(crate) struct OpenAiMessageItem {
    pub(crate) role: &'static str,
    pub(crate) content: Vec<OpenAiContentItem>,
}

#[derive(Serialize)]
pub(crate) struct OpenAiContentItem {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) text: String,
}

#[derive(Serialize)]
pub(crate) struct OpenAiFunctionCallItem {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) call_id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Serialize)]
pub(crate) struct OpenAiFunctionCallOutputItem {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) call_id: String,
    pub(crate) output: String,
}

#[derive(Serialize)]
pub(crate) struct OpenAiToolManifest {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
    pub(crate) strict: bool,
}

impl OpenAiToolManifest {
    /// Converts a provider-neutral tool manifest into Responses function-tool shape.
    fn from_tool_manifest(manifest: ToolManifest) -> Self {
        Self {
            kind: "function",
            name: manifest.name,
            description: manifest.description,
            parameters: manifest.parameters,
            strict: false,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct OpenAiReasoningRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) effort: Option<String>,
    pub(crate) summary: &'static str,
}

impl OpenAiReasoningRequest {
    fn from_flags(include_reasoning: bool, effort: Option<String>) -> Option<Self> {
        let effort = effort.filter(|effort| !effort.trim().is_empty());
        if !include_reasoning && effort.is_none() {
            return None;
        }

        Some(Self {
            effort,
            summary: REASONING_SUMMARY_AUTO,
        })
    }
}

#[derive(Deserialize)]
pub(crate) struct OpenAiStreamMessage {
    #[serde(rename = "type")]
    pub(crate) kind: String,
    pub(crate) delta: Option<String>,
    pub(crate) item: Option<OpenAiOutputItem>,
    pub(crate) response: Option<OpenAiResponse>,
    pub(crate) error: Option<OpenAiError>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiOutputItem {
    #[serde(rename = "type")]
    pub(crate) kind: Option<String>,
    pub(crate) id: Option<String>,
    pub(crate) call_id: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) arguments: Option<String>,
}

impl OpenAiOutputItem {
    /// Converts a completed function-call item into a provider tool call.
    pub(crate) fn into_provider_tool_call(self) -> Result<ProviderToolCall, ProviderError> {
        if self.kind.as_deref() != Some("function_call") {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenAI".to_owned(),
                reason: "output item was not a function call".to_owned(),
            });
        }
        let call_id = self
            .call_id
            .or(self.id)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ProviderError::MalformedResponse {
                provider_name: "OpenAI".to_owned(),
                reason: "function call omitted call_id".to_owned(),
            })?;
        let name = self
            .name
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ProviderError::MalformedResponse {
                provider_name: "OpenAI".to_owned(),
                reason: "function call omitted name".to_owned(),
            })?;
        let arguments = self.arguments.unwrap_or_default();

        Ok(ProviderToolCall::new(call_id, name, arguments))
    }
}

#[derive(Deserialize)]
pub(crate) struct OpenAiResponse {
    pub(crate) status: Option<String>,
    pub(crate) incomplete_details: Option<OpenAiIncompleteDetails>,
    pub(crate) usage: Option<UsageMetadata>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiIncompleteDetails {
    pub(crate) reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OpenAiError {
    pub(crate) code: Option<String>,
    pub(crate) message: String,
}

/// Extracts system messages as Responses instructions.
fn instructions_from_messages(messages: &[ProviderMessage]) -> Option<String> {
    let instructions = messages
        .iter()
        .filter(|message| message.role == ProviderMessageRole::System)
        .map(|message| message.content.as_str())
        .filter(|content| !content.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    (!instructions.trim().is_empty()).then_some(instructions)
}

/// Converts non-system provider messages into Responses input items.
fn input_from_messages(messages: Vec<ProviderMessage>) -> Vec<OpenAiInputItem> {
    messages
        .into_iter()
        .filter(|message| message.role != ProviderMessageRole::System)
        .flat_map(input_from_message)
        .collect()
}

/// Converts one provider message into one or more Responses input items.
fn input_from_message(message: ProviderMessage) -> Vec<OpenAiInputItem> {
    if message.role == ProviderMessageRole::Tool {
        return vec![OpenAiInputItem::FunctionCallOutput(
            OpenAiFunctionCallOutputItem {
                kind: "function_call_output",
                call_id: message.tool_call_id.unwrap_or_default(),
                output: message.content,
            },
        )];
    }

    if message.role == ProviderMessageRole::Assistant && !message.tool_calls.is_empty() {
        return message
            .tool_calls
            .into_iter()
            .map(|tool_call| {
                OpenAiInputItem::FunctionCall(OpenAiFunctionCallItem {
                    kind: "function_call",
                    call_id: tool_call.id,
                    name: tool_call.name,
                    arguments: tool_call.arguments,
                })
            })
            .collect();
    }

    let role = match message.role {
        ProviderMessageRole::User => "user",
        ProviderMessageRole::Assistant => "assistant",
        ProviderMessageRole::System | ProviderMessageRole::Tool => "user",
    };
    let content_type = if role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };

    vec![OpenAiInputItem::Message(OpenAiMessageItem {
        role,
        content: vec![OpenAiContentItem {
            kind: content_type,
            text: message.content,
        }],
    })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderMessage, ProviderRequest, ProviderToolCall, ToolManifest};
    use serde_json::json;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openai_requests.rs"
    ));
}
