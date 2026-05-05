use serde_json::{json, Value};
use spectacular_llms::{Cancellation, ProviderToolCall, ToolManifest};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type ToolDisplay = String;
pub type ToolExecution<'a> = Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + 'a>>;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;

    fn manifest(&self) -> ToolManifest;

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        serde_json::to_string(arguments).unwrap_or_else(|_| arguments.to_string())
    }

    fn format_output(&self, raw_output: &str, _parsed_output: Option<&Value>) -> ToolDisplay {
        raw_output.to_owned()
    }

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

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ToolRegistrationError {
    EmptyName,
    UnsafeName {
        name: String,
    },
    ManifestNameMismatch {
        tool_name: String,
        manifest_name: String,
    },
    EmptyDescription {
        name: String,
    },
    InvalidParameterSchema {
        name: String,
        reason: String,
    },
    DuplicateName {
        name: String,
    },
}

impl Display for ToolRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => formatter.write_str("tool name cannot be empty"),
            Self::UnsafeName { name } => write!(
                formatter,
                "tool name `{name}` must match ^[A-Za-z_][A-Za-z0-9_]*$"
            ),
            Self::ManifestNameMismatch {
                tool_name,
                manifest_name,
            } => write!(
                formatter,
                "tool name `{tool_name}` does not match manifest name `{manifest_name}`"
            ),
            Self::EmptyDescription { name } => {
                write!(
                    formatter,
                    "tool `{name}` manifest description cannot be empty"
                )
            }
            Self::InvalidParameterSchema { name, reason } => {
                write!(
                    formatter,
                    "tool `{name}` parameter schema is invalid: {reason}"
                )
            }
            Self::DuplicateName { name } => {
                write!(formatter, "tool `{name}` is already registered")
            }
        }
    }
}

impl Error for ToolRegistrationError {}

#[derive(Clone, Default)]
pub struct ToolStorage {
    tools: BTreeMap<String, Arc<dyn Tool>>,
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
    pub fn try_with_tool<T>(tool: T) -> Result<Self, ToolRegistrationError>
    where
        T: Tool + 'static,
    {
        let mut storage = Self::default();
        storage.register(tool)?;
        Ok(storage)
    }

    pub fn register<T>(&mut self, tool: T) -> Result<(), ToolRegistrationError>
    where
        T: Tool + 'static,
    {
        let manifest = tool.manifest();
        validate_tool_registration(tool.name(), &manifest)?;
        if self.tools.contains_key(&manifest.name) {
            return Err(ToolRegistrationError::DuplicateName {
                name: manifest.name,
            });
        }

        self.tools.insert(manifest.name, Arc::new(tool));
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn manifests(&self) -> Vec<ToolManifest> {
        self.tools.values().map(|tool| tool.manifest()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub async fn execute(
        &self,
        tool_call: &ProviderToolCall,
        cancellation: Cancellation,
    ) -> String {
        let Some(tool) = self.get(&tool_call.name) else {
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
            Ok(output) => output,
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

fn validate_tool_registration(
    tool_name: &str,
    manifest: &ToolManifest,
) -> Result<(), ToolRegistrationError> {
    validate_tool_name(tool_name)?;
    validate_tool_name(&manifest.name)?;
    if tool_name != manifest.name {
        return Err(ToolRegistrationError::ManifestNameMismatch {
            tool_name: tool_name.to_owned(),
            manifest_name: manifest.name.clone(),
        });
    }

    if manifest.description.trim().is_empty() {
        return Err(ToolRegistrationError::EmptyDescription {
            name: manifest.name.clone(),
        });
    }

    validate_parameter_schema(&manifest.name, &manifest.parameters)
}

fn validate_tool_name(name: &str) -> Result<(), ToolRegistrationError> {
    if name.trim().is_empty() {
        return Err(ToolRegistrationError::EmptyName);
    }

    if is_function_call_safe_name(name) {
        return Ok(());
    }

    Err(ToolRegistrationError::UnsafeName {
        name: name.to_owned(),
    })
}

fn is_function_call_safe_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn validate_parameter_schema(name: &str, schema: &Value) -> Result<(), ToolRegistrationError> {
    if !schema.is_object() {
        return Err(ToolRegistrationError::InvalidParameterSchema {
            name: name.to_owned(),
            reason: "schema must be a JSON object".to_owned(),
        });
    }

    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err(ToolRegistrationError::InvalidParameterSchema {
            name: name.to_owned(),
            reason: "schema type must be `object`".to_owned(),
        });
    }

    jsonschema::validator_for(schema).map_err(|error| {
        ToolRegistrationError::InvalidParameterSchema {
            name: name.to_owned(),
            reason: error.to_string(),
        }
    })?;
    Ok(())
}
