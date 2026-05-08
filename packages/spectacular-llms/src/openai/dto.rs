use crate::{
    ProviderError, ProviderMessage, ProviderMessageRole, ProviderRequest, ProviderToolCall,
    ToolManifest, UsageMetadata,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct OpenAiResponsesRequest {
    pub(crate) model: String,
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
        let instructions = instructions_from_messages(&messages);
        let input = input_from_messages(messages);
        let tools = tools
            .into_iter()
            .map(OpenAiToolManifest::from_tool_manifest)
            .collect::<Vec<_>>();
        let tool_choice = if tools.is_empty() { None } else { Some("auto") };
        let parallel_tool_calls = if tools.is_empty() { None } else { Some(false) };
        let reasoning = flags
            .reasoning_effort
            .filter(|effort| !effort.trim().is_empty())
            .map(|effort| OpenAiReasoningRequest { effort });

        Ok(Self {
            model,
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
    pub(crate) effort: String,
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
