use super::dto::OpenAiStreamMessage;
use crate::{
    FinishReason, MessageDelta, ProviderError, ProviderFinished, ProviderStreamEvent,
    ReasoningDelta,
};

/// Parses one OpenAI Responses SSE data payload into provider events.
pub(crate) fn parse_openai_response_event(
    payload: &str,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let event: OpenAiStreamMessage =
        serde_json::from_str(payload).map_err(|error| ProviderError::ResponseParsingFailed {
            provider_name: "OpenAI".to_owned(),
            reason: format!("{error}; OpenAI response event JSON: {payload}"),
        })?;

    match event.kind.as_str() {
        "response.output_text.delta" => Ok(event
            .delta
            .filter(|delta| !delta.is_empty())
            .map(|delta| {
                vec![ProviderStreamEvent::MessageDelta(MessageDelta::assistant(
                    delta,
                ))]
            })
            .unwrap_or_default()),
        "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => Ok(event
            .delta
            .filter(|delta| !delta.is_empty())
            .map(|delta| {
                vec![ProviderStreamEvent::ReasoningDelta(ReasoningDelta {
                    content: delta,
                    metadata: None,
                })]
            })
            .unwrap_or_default()),
        "response.output_item.done" => parse_output_item_done(event, payload),
        "response.completed" => Ok(vec![ProviderStreamEvent::Finished(ProviderFinished {
            finish_reason: FinishReason::Stop,
            tool_calls: Vec::new(),
            usage: event.response.and_then(|response| response.usage),
            reasoning: None,
        })]),
        "response.incomplete" => parse_incomplete_response(event),
        "response.failed" => Err(openai_failed_response(event, payload)),
        "error" => Err(openai_stream_error(event, payload)),
        _ => Ok(Vec::new()),
    }
}

/// Parses an incomplete response event as a length-style finish.
fn parse_incomplete_response(
    event: OpenAiStreamMessage,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let response = event.response;
    let _reason = response
        .as_ref()
        .and_then(|response| response.incomplete_details.as_ref())
        .and_then(|details| details.reason.as_deref());

    Ok(vec![ProviderStreamEvent::Finished(ProviderFinished {
        finish_reason: FinishReason::Length,
        tool_calls: Vec::new(),
        usage: response.and_then(|response| response.usage),
        reasoning: None,
    })])
}

/// Parses completed function-call output items into tool-call finish events.
fn parse_output_item_done(
    event: OpenAiStreamMessage,
    payload: &str,
) -> Result<Vec<ProviderStreamEvent>, ProviderError> {
    let Some(item) = event.item else {
        return Ok(Vec::new());
    };
    if item.kind.as_deref() != Some("function_call") {
        return Ok(Vec::new());
    }

    let tool_call =
        item.into_provider_tool_call()
            .map_err(|error| ProviderError::MalformedResponse {
                provider_name: "OpenAI".to_owned(),
                reason: format!("{error}; OpenAI response event JSON: {payload}"),
            })?;
    Ok(vec![ProviderStreamEvent::Finished(ProviderFinished {
        finish_reason: FinishReason::ToolCalls,
        tool_calls: vec![tool_call],
        usage: None,
        reasoning: None,
    })])
}

/// Converts a failed Responses event into a provider error.
fn openai_failed_response(event: OpenAiStreamMessage, payload: &str) -> ProviderError {
    let status = event
        .response
        .and_then(|response| response.status)
        .unwrap_or_else(|| "failed".to_owned());
    ProviderError::StreamError {
        provider_name: "OpenAI".to_owned(),
        code: Some(status),
        message: format!("OpenAI response failed; OpenAI response event JSON: {payload}"),
    }
}

/// Converts an error event into a provider error.
fn openai_stream_error(event: OpenAiStreamMessage, payload: &str) -> ProviderError {
    let Some(error) = event.error else {
        return ProviderError::StreamError {
            provider_name: "OpenAI".to_owned(),
            code: None,
            message: format!("OpenAI stream returned error; OpenAI response event JSON: {payload}"),
        };
    };

    ProviderError::StreamError {
        provider_name: "OpenAI".to_owned(),
        code: error.code,
        message: format!("{}; OpenAI response event JSON: {payload}", error.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessageDelta, ProviderToolCall, UsageMetadata};

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/openai_stream.rs"
    ));
}
