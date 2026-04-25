use serde_json::{json, Value};
use spectacular_llms::{Cancellation, ProviderToolCall};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ToolExecution<'a> = Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + 'a>>;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;

    fn execute<'a>(&'a self, arguments: Value, cancellation: Cancellation) -> ToolExecution<'a>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolError {
    message: String,
}

impl ToolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ToolError {}

#[derive(Clone, Default)]
pub struct ToolStorage {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl std::fmt::Debug for ToolStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolStorage")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ToolStorage {
    pub async fn register<T>(&mut self, tool: T)
    where
        T: Tool + 'static,
    {
        self.tools.insert(tool.name().to_owned(), Arc::new(tool));
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub async fn execute(
        &self,
        tool_call: &ProviderToolCall,
        cancellation: Cancellation,
    ) -> String {
        let Some(tool) = self.tools.get(&tool_call.name) else {
            return tool_error(
                "missing_tool",
                format!("tool `{}` is not registered", tool_call.name),
            );
        };

        let arguments = match serde_json::from_str::<Value>(&tool_call.arguments) {
            Ok(arguments) => arguments,
            Err(error) => return tool_error("malformed_arguments", error.to_string()),
        };

        match tool.execute(arguments, cancellation).await {
            Ok(value) => serde_json::to_string(&value)
                .unwrap_or_else(|error| tool_error("result_formatting", error.to_string())),
            Err(error) => tool_error("execution_failed", error.to_string()),
        }
    }
}

pub fn format_tool_call_request(tool_call: &ProviderToolCall) -> String {
    json!({
        "id": tool_call.id,
        "name": tool_call.name,
        "arguments": tool_call.arguments,
    })
    .to_string()
}

fn tool_error(error_kind: &str, message: impl Into<String>) -> String {
    json!({
        "error_kind": error_kind,
        "message": message.into(),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug)]
    struct CapturingTool {
        calls: Arc<Mutex<Vec<Value>>>,
    }

    impl Tool for CapturingTool {
        fn name(&self) -> &str {
            "capture"
        }

        fn execute<'a>(
            &'a self,
            arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(arguments.clone());
                Ok(json!({"ok": true, "arguments": arguments}))
            })
        }
    }

    #[derive(Clone, Debug)]
    struct FailingTool;

    impl Tool for FailingTool {
        fn name(&self) -> &str {
            "fail"
        }

        fn execute<'a>(
            &'a self,
            _arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
            Box::pin(async { Err(ToolError::new("tool exploded")) })
        }
    }

    #[tokio::test]
    async fn registered_tool_executes_with_parsed_arguments() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut storage = ToolStorage::default();
        storage
            .register(CapturingTool {
                calls: Arc::clone(&calls),
            })
            .await;

        let result = storage
            .execute(
                &ProviderToolCall::new("call-1", "capture", r#"{"path":"Cargo.toml"}"#),
                Cancellation::default(),
            )
            .await;
        let value: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [json!({"path":"Cargo.toml"})]
        );
    }

    #[tokio::test]
    async fn missing_tool_is_returned_as_tool_result_error() {
        let storage = ToolStorage::default();

        let result = storage
            .execute(
                &ProviderToolCall::new("call-1", "missing", "{}"),
                Cancellation::default(),
            )
            .await;
        let value: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(value["error_kind"], "missing_tool");
    }

    #[tokio::test]
    async fn malformed_arguments_are_returned_as_tool_result_error() {
        let mut storage = ToolStorage::default();
        storage
            .register(CapturingTool {
                calls: Arc::new(Mutex::new(Vec::new())),
            })
            .await;

        let result = storage
            .execute(
                &ProviderToolCall::new("call-1", "capture", "{"),
                Cancellation::default(),
            )
            .await;
        let value: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(value["error_kind"], "malformed_arguments");
    }

    #[tokio::test]
    async fn execution_failure_is_returned_as_tool_result_error() {
        let mut storage = ToolStorage::default();
        storage.register(FailingTool).await;

        let result = storage
            .execute(
                &ProviderToolCall::new("call-1", "fail", "{}"),
                Cancellation::default(),
            )
            .await;
        let value: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(value["error_kind"], "execution_failed");
        assert_eq!(value["message"], "tool exploded");
    }

    #[test]
    fn tool_call_request_format_is_provider_visible_json() {
        let formatted =
            format_tool_call_request(&ProviderToolCall::new("call-1", "read", r#"{"a":1}"#));
        let value: Value = serde_json::from_str(&formatted).unwrap();

        assert_eq!(value["id"], "call-1");
        assert_eq!(value["name"], "read");
        assert_eq!(value["arguments"], r#"{"a":1}"#);
    }
}
