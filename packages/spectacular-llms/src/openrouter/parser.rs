use super::dto::{OpenRouterChatChunk, OpenRouterChatDeltaToolCall, OpenRouterStreamError};
use crate::{
    FinishReason, MessageDelta, ProviderError, ProviderFinished, ProviderStreamEvent,
    ProviderToolCall, ReasoningDelta,
};
use std::collections::BTreeMap;

#[cfg(test)]
pub(crate) fn parse_openrouter_chat_chunk(
    payload: &str,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let mut accumulator = OpenRouterToolCallAccumulator::default();
    parse_openrouter_chat_chunk_with_accumulator(payload, &mut accumulator)
}

pub(crate) fn parse_openrouter_chat_chunk_with_accumulator(
    payload: &str,
    accumulator: &mut OpenRouterToolCallAccumulator,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let chunk: OpenRouterChatChunk =
        serde_json::from_str(payload).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenRouter".to_owned(),
            reason: error.to_string(),
        })?;
    let mut events = Vec::new();
    let usage = chunk.usage;

    if let Some(error) = chunk.error {
        return Err(openrouter_stream_error(error, payload));
    }

    if chunk.choices.is_empty() {
        let Some(usage) = usage else {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!(
                    "stream chunk omitted choices; OpenRouter response chunk JSON: {payload}"
                ),
            });
        };

        events.push(ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason: FinishReason::Stop,
            tool_calls: Vec::new(),
            usage: Some(usage),
            reasoning: None,
        }));
        return Ok(events);
    }

    for choice in chunk.choices {
        let mut finish_reason = choice.finish_reason;
        let native_finish_reason = choice.native_finish_reason;
        let mut complete_tool_calls = Vec::new();
        if let Some(delta) = choice.delta {
            if let Some(tool_calls) = delta.tool_calls {
                accumulator.add_chunks(tool_calls)?;
            }
            append_text_events(&mut events, delta.content, delta.reasoning, delta.refusal);
            if finish_reason.is_none() {
                finish_reason = delta.finish_reason;
            }
        }

        if let Some(message) = choice.message {
            append_text_events(
                &mut events,
                message.content,
                message.reasoning,
                message.refusal,
            );
            if let Some(tool_calls) = message.tool_calls {
                complete_tool_calls = tool_calls
                    .into_iter()
                    .enumerate()
                    .map(|(index, tool_call)| tool_call.into_provider_tool_call(index))
                    .collect::<Result<Vec<_>, _>>()?;
            }
        }

        let Some(finish_reason) = finish_reason else {
            continue;
        };

        let finish_reason = parse_openrouter_finish_reason(&finish_reason);
        let tool_calls = finish_tool_calls(
            accumulator,
            complete_tool_calls,
            finish_reason,
            native_finish_reason.as_deref(),
            payload,
        )?;

        events.push(ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason,
            tool_calls,
            usage,
            reasoning: None,
        }));
    }

    Ok(events)
}

#[derive(Default)]
pub(crate) struct OpenRouterToolCallAccumulator {
    tool_calls: BTreeMap<usize, OpenRouterAccumulatedToolCall>,
}

impl OpenRouterToolCallAccumulator {
    fn add_chunks(
        &mut self,
        tool_calls: Vec<OpenRouterChatDeltaToolCall>,
    ) -> Result<(), ProviderError> {
        for tool_call in tool_calls {
            self.add_chunk(tool_call)?;
        }

        Ok(())
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    fn finish_tool_calls(&mut self) -> Result<Vec<ProviderToolCall>, ProviderError> {
        if self.tool_calls.is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: "tool-call finish did not include tool-call chunks".to_owned(),
            });
        }

        let tool_calls = std::mem::take(&mut self.tool_calls);
        tool_calls
            .into_iter()
            .map(|(index, accumulated)| accumulated.into_provider_tool_call(index))
            .collect()
    }

    fn add_chunk(&mut self, tool_call: OpenRouterChatDeltaToolCall) -> Result<(), ProviderError> {
        if let Some(kind) = tool_call.kind.as_deref() {
            if kind != "function" {
                return Err(ProviderError::MalformedResponse {
                    provider_name: "OpenRouter".to_owned(),
                    reason: format!("unsupported tool-call type `{kind}`"),
                });
            }
        }

        let accumulated = self.tool_calls.entry(tool_call.index).or_default();
        if let Some(id) = tool_call.id {
            append_tool_call_id(accumulated, tool_call.index, id)?;
        }
        if let Some(function) = tool_call.function {
            if let Some(name) = function.name {
                accumulated.name.push_str(&name);
            }
            if let Some(arguments) = function.arguments {
                accumulated.arguments.push_str(&arguments);
                accumulated.saw_arguments = true;
            }
        }

        Ok(())
    }
}

#[derive(Default)]
struct OpenRouterAccumulatedToolCall {
    id: Option<String>,
    name: String,
    arguments: String,
    saw_arguments: bool,
}

impl OpenRouterAccumulatedToolCall {
    fn into_provider_tool_call(self, index: usize) -> Result<ProviderToolCall, ProviderError> {
        let Some(id) = self.id.filter(|id| !id.trim().is_empty()) else {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted id"),
            });
        };

        if self.name.trim().is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function name"),
            });
        }

        if !self.saw_arguments {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: format!("tool-call index {index} omitted function arguments"),
            });
        }

        Ok(ProviderToolCall::new(id, self.name, self.arguments))
    }
}

fn append_text_events(
    events: &mut Vec<ProviderStreamEvent>,
    content: Option<String>,
    reasoning: Option<String>,
    refusal: Option<String>,
) {
    if let Some(content) = content.filter(|content| !content.is_empty()) {
        events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
            content,
        )));
    }
    if let Some(reasoning) = reasoning.filter(|reasoning| !reasoning.is_empty()) {
        events.push(ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
            content: reasoning,
            metadata: None,
        }));
    }
    if let Some(refusal) = refusal.filter(|refusal| !refusal.is_empty()) {
        events.push(ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
            refusal,
        )));
    }
}

fn append_tool_call_id(
    accumulated: &mut OpenRouterAccumulatedToolCall,
    index: usize,
    id: String,
) -> Result<(), ProviderError> {
    let Some(existing_id) = accumulated.id.as_deref() else {
        accumulated.id = Some(id);
        return Ok(());
    };

    if existing_id == id {
        return Ok(());
    }

    Err(ProviderError::MalformedResponse {
        provider_name: "OpenRouter".to_owned(),
        reason: format!("tool-call index {index} changed id from `{existing_id}` to `{id}`"),
    })
}

fn finish_tool_calls(
    accumulator: &mut OpenRouterToolCallAccumulator,
    complete_tool_calls: Vec<ProviderToolCall>,
    finish_reason: FinishReason,
    native_finish_reason: Option<&str>,
    payload: &str,
) -> Result<Vec<ProviderToolCall>, ProviderError> {
    if finish_reason == FinishReason::ToolCalls {
        let mut accumulated = if accumulator.has_pending() {
            accumulator.finish_tool_calls()?
        } else {
            Vec::new()
        };
        accumulated.extend(complete_tool_calls);
        if accumulated.is_empty() {
            return Err(ProviderError::MalformedResponse {
                provider_name: "OpenRouter".to_owned(),
                reason: openrouter_empty_tool_call_finish_reason(native_finish_reason, payload),
            });
        }
        return Ok(accumulated);
    }

    if accumulator.has_pending() || !complete_tool_calls.is_empty() {
        return Err(ProviderError::MalformedResponse {
            provider_name: "OpenRouter".to_owned(),
            reason: format!(
                "tool-call chunks ended without tool-call finish; OpenRouter response chunk JSON: {payload}"
            ),
        });
    }

    Ok(Vec::new())
}

fn parse_openrouter_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "tool_calls" => FinishReason::ToolCalls,
        "cancelled" => FinishReason::Cancelled,
        "content_filter" => FinishReason::ContentFilter,
        "error" => FinishReason::Error,
        _ => FinishReason::Stop,
    }
}

fn openrouter_stream_error(error: OpenRouterStreamError, payload: &str) -> ProviderError {
    ProviderError::StreamError {
        provider_name: "OpenRouter".to_owned(),
        code: error
            .code
            .as_ref()
            .and_then(openrouter_error_code_to_string),
        message: format!(
            "{}; OpenRouter response chunk JSON: {payload}",
            error.message
        ),
    }
}

fn openrouter_error_code_to_string(code: &serde_json::Value) -> Option<String> {
    match code {
        serde_json::Value::Null => None,
        serde_json::Value::String(code) if code.trim().is_empty() => None,
        serde_json::Value::String(code) => Some(code.clone()),
        serde_json::Value::Number(code) => Some(code.to_string()),
        _ => Some(code.to_string()),
    }
}

fn openrouter_empty_tool_call_finish_reason(
    native_finish_reason: Option<&str>,
    payload: &str,
) -> String {
    let native_finish_reason = native_finish_reason
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("unknown");

    format!(
        "OpenRouter reported finish_reason=tool_calls without any tool call data \
         (no delta.tool_calls and no message.tool_calls). \
         native_finish_reason={native_finish_reason}. \
         This usually means the selected model/provider route stopped without emitting a native function call, \
         even though tools were present. Try a different tool-capable model/provider route or disable tools for this model. \
         OpenRouter response chunk JSON: {payload}"
    )
}
