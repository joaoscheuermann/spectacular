use super::ChatEvent;
use serde_json::{Map, Value};
use spectacular_llms::{FinishReason, ProviderMessageRole};

impl ChatEvent {
    /// Deserializes a chat event value while preserving unknown input on failure.
    pub fn from_value(value: Value) -> Result<Self, Value> {
        let original = value.clone();
        let value = normalize_legacy_tool_call(value);
        serde_json::from_value(value).map_err(|_| original)
    }
}

/// Converts a provider message role into its serialized chat role.
pub(super) fn role(role: ProviderMessageRole) -> &'static str {
    match role {
        ProviderMessageRole::System => "system",
        ProviderMessageRole::User => "user",
        ProviderMessageRole::Assistant => "assistant",
        ProviderMessageRole::Tool => "tool",
    }
}

/// Converts a finish reason into its persisted string form.
pub(super) fn finish_reason_to_str(reason: FinishReason) -> &'static str {
    match reason {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "toolcalls",
        FinishReason::Cancelled => "cancelled",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::Error => "error",
    }
}

/// Converts a persisted finish reason string into a provider finish reason.
pub(super) fn finish_reason_from_str(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "toolcalls" | "tool_calls" => FinishReason::ToolCalls,
        "cancelled" => FinishReason::Cancelled,
        "content_filter" => FinishReason::ContentFilter,
        "error" => FinishReason::Error,
        _ => FinishReason::Stop,
    }
}

/// Returns the default title used for sessions without a stored title.
pub(super) fn untitled() -> String {
    "Untitled session".to_owned()
}

/// Returns the default assistant role used for legacy assistant deltas.
pub(super) fn assistant_role() -> String {
    "assistant".to_owned()
}

/// Returns the replay ID used for legacy assistant delta records.
pub(super) fn session_replay_message_id() -> String {
    "session-replay-message".to_owned()
}

/// Returns the replay ID used for legacy reasoning delta records.
pub(super) fn session_replay_reasoning_id() -> String {
    "session-replay-reasoning".to_owned()
}

/// Normalizes legacy tool-call JSON into the structured session schema.
pub(super) fn normalize_legacy_tool_call(mut value: Value) -> Value {
    let Some(object) = legacy_tool_call_object(&mut value) else {
        return value;
    };
    let Some(content) = legacy_tool_call_content(object) else {
        return value;
    };

    apply_legacy_tool_call_content(object, &content);
    value
}

fn legacy_tool_call_object(value: &mut Value) -> Option<&mut Map<String, Value>> {
    let object = value.as_object_mut()?;
    if object.get("type").and_then(Value::as_str) != Some("tool_call") {
        return None;
    }
    if has_structured_tool_call_fields(object) {
        return None;
    }

    Some(object)
}

fn has_structured_tool_call_fields(object: &Map<String, Value>) -> bool {
    object
        .get("tool_call_id")
        .or_else(|| object.get("name"))
        .or_else(|| object.get("arguments"))
        .is_some()
}

fn legacy_tool_call_content(object: &Map<String, Value>) -> Option<Value> {
    let content = object.get("content").and_then(Value::as_str)?;
    serde_json::from_str(content).ok()
}

fn apply_legacy_tool_call_content(object: &mut Map<String, Value>, content: &Value) {
    if let Some(tool_call_id) = content.get("id").and_then(Value::as_str) {
        object.insert(
            "tool_call_id".to_owned(),
            Value::String(tool_call_id.to_owned()),
        );
    }
    if let Some(name) = content.get("name").and_then(Value::as_str) {
        object.insert("name".to_owned(), Value::String(name.to_owned()));
    }
    if let Some(arguments) = content.get("arguments").and_then(Value::as_str) {
        object.insert("arguments".to_owned(), Value::String(arguments.to_owned()));
    }
}
