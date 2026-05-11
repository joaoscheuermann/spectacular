use crate::diff_preview::diff_preview;
use crate::display::{error_style, paint, tool_line};
use crate::path::resolve_workspace_path;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::path::{Path, PathBuf};

pub const WRITE_TOOL_NAME: &str = "write";

const WRITE_TOOL_DESCRIPTION: &str = "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.";

#[derive(Clone, Debug)]
pub struct WriteTool {
    workspace_root: PathBuf,
}

impl WriteTool {
    /// Creates a write tool scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for WriteTool {
    /// Returns the stable tool name used for registration and dispatch.
    fn name(&self) -> &str {
        WRITE_TOOL_NAME
    }

    /// Builds the write tool manifest and JSON parameter schema.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            WRITE_TOOL_NAME,
            WRITE_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write. Relative paths resolve against the workspace root; absolute paths and .. traversal are allowed intentionally."
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file."
                    }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        )
    }

    /// Formats write arguments as path and byte-count text.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("<missing path>");
        let byte_count = arguments
            .get("content")
            .and_then(Value::as_str)
            .map(str::len)
            .unwrap_or_default();
        format!("{path} ({byte_count} bytes)")
    }

    /// Formats write arguments as a styled renderer call line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("<missing path>");
        let byte_count = arguments
            .get("content")
            .and_then(Value::as_str)
            .map(str::len)
            .unwrap_or_default();
        let metadata = format!("({byte_count} bytes)");
        tool_line("Write", path, Some(&metadata))
    }

    /// Formats write output as either an error, diff body, or raw fallback text.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        if let Some(error) = output.get("error").and_then(Value::as_str) {
            let status = paint(error_style(), "failed");
            return format!("{status}: {error}");
        }

        output
            .get("diff")
            .and_then(Value::as_str)
            .filter(|diff| !diff.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| output.to_string())
    }

    /// Writes content to disk and returns a serialized write output payload.
    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        let workspace_root = self.workspace_root.clone();

        Box::pin(async move {
            let input = match serde_json::from_value::<WriteInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(write_error(format!("Invalid arguments: {error}")));
                }
            };

            if input.path.is_empty() {
                return Ok(write_error("Path must not be empty"));
            }

            Ok(write_file(&workspace_root, input).await)
        })
    }
}

#[derive(Debug, Deserialize)]
struct WriteInput {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WriteOutput {
    pub success: bool,
    pub bytes_written: usize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Creates parent directories, writes file content, and serializes the write result.
async fn write_file(workspace_root: &Path, input: WriteInput) -> String {
    let path = input.path;
    let content = input.content;
    let bytes_written = content.len();
    let file_path = resolve_workspace_path(workspace_root, &path);
    let Some(parent) = file_path.parent() else {
        return write_error("Invalid path: no parent directory");
    };

    if let Err(error) = tokio::fs::create_dir_all(parent).await {
        return write_error(format!("Failed to create parent directories: {error}"));
    }

    let old_content = tokio::fs::read_to_string(&file_path)
        .await
        .unwrap_or_default();
    if let Err(error) = tokio::fs::write(&file_path, &content).await {
        return write_error(format!("Failed to write file: {error}"));
    }

    write_success(&path, &old_content, &content, bytes_written)
}

/// Builds a successful write payload including byte count and diff preview.
fn write_success(path: &str, old_content: &str, new_content: &str, bytes_written: usize) -> String {
    let diff = diff_preview(old_content, new_content);
    serde_json::to_string(&WriteOutput {
        success: true,
        bytes_written,
        diff: format!(
            "Edited {path} (+{} -{})\n{}",
            diff.added, diff.removed, diff.lines
        ),
        error: None,
    })
    .expect("write output should serialize")
}

/// Builds a failed write payload with a user-facing error message.
fn write_error(message: impl Into<String>) -> String {
    serde_json::to_string(&WriteOutput {
        success: false,
        bytes_written: 0,
        diff: String::new(),
        error: Some(message.into()),
    })
    .expect("write output should serialize")
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/write.rs"));
}
