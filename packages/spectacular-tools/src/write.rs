use crate::display::paint;
use crate::path::resolve_workspace_path;
use anstyle::AnsiColor;
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
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for WriteTool {
    fn name(&self) -> &str {
        WRITE_TOOL_NAME
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            WRITE_TOOL_NAME,
            WRITE_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write."
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

    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        if output.get("success").and_then(Value::as_bool) == Some(true) {
            let bytes_written = output
                .get("bytes_written")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            let status = paint(AnsiColor::BrightGreen.on_default().bold(), "wrote");
            return format!("{status} {bytes_written} bytes");
        }

        let error = output
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("write failed");
        let status = paint(AnsiColor::BrightRed.on_default().bold(), "failed");
        format!("{status}: {error}")
    }

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

async fn write_file(workspace_root: &Path, input: WriteInput) -> String {
    let content = input.content;
    let bytes_written = content.len();
    let file_path = resolve_workspace_path(workspace_root, input.path);
    let Some(parent) = file_path.parent() else {
        return write_error("Invalid path: no parent directory");
    };

    if let Err(error) = tokio::fs::create_dir_all(parent).await {
        return write_error(format!("Failed to create parent directories: {error}"));
    }

    if let Err(error) = tokio::fs::write(&file_path, content).await {
        return write_error(format!("Failed to write file: {error}"));
    }

    write_success(bytes_written)
}

fn write_success(bytes_written: usize) -> String {
    serde_json::to_string(&WriteOutput {
        success: true,
        bytes_written,
        error: None,
    })
    .expect("write output should serialize")
}

fn write_error(message: impl Into<String>) -> String {
    serde_json::to_string(&WriteOutput {
        success: false,
        bytes_written: 0,
        error: Some(message.into()),
    })
    .expect("write output should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn foo_txt_writes_under_workspace_root() {
        let workspace_root = temp_workspace("foo_txt_writes_under_workspace_root").await;
        let tool = WriteTool::new(&workspace_root);

        let result = tool
            .execute(
                json!({"path": "foo.txt", "content": "hello"}),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: WriteOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(
            output,
            WriteOutput {
                success: true,
                bytes_written: 5,
                error: None
            }
        );
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap(),
            json!({
                "success": true,
                "bytes_written": 5
            })
        );
        assert_eq!(
            tokio::fs::read_to_string(workspace_root.join("foo.txt"))
                .await
                .unwrap(),
            "hello"
        );

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn nested_relative_path_creates_parents() {
        let workspace_root = temp_workspace("nested_relative_path_creates_parents").await;
        let tool = WriteTool::new(&workspace_root);

        let result = tool
            .execute(
                json!({"path": "notes/today/file.txt", "content": "nested"}),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: WriteOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(output.success, true);
        assert_eq!(
            tokio::fs::read_to_string(workspace_root.join("notes/today/file.txt"))
                .await
                .unwrap(),
            "nested"
        );

        remove_workspace(workspace_root).await;
    }

    #[test]
    fn write_manifest_registers_through_tool_storage() {
        let mut storage = spectacular_agent::ToolStorage::default();
        storage
            .register(WriteTool::new(PathBuf::from("workspace")))
            .unwrap();
        let tool = storage.get(WRITE_TOOL_NAME).unwrap();
        let manifest = tool.manifest();

        assert_eq!(manifest.name, WRITE_TOOL_NAME);
        assert_eq!(manifest.description, WRITE_TOOL_DESCRIPTION);
        assert_eq!(manifest.parameters["required"], json!(["path", "content"]));
        assert_eq!(
            storage
                .manifests()
                .into_iter()
                .map(|manifest| manifest.name)
                .collect::<Vec<_>>(),
            vec![WRITE_TOOL_NAME]
        );
    }

    #[test]
    fn write_format_input_and_output_are_non_empty() {
        let tool = WriteTool::new(PathBuf::from("workspace"));
        let input = json!({"path": "foo.txt", "content": "hello"});
        let output = json!({
            "success": true,
            "bytes_written": 5
        });
        let raw_output = output.to_string();

        assert!(!tool.format_input(&input).is_empty());
        assert!(!tool.format_output(&raw_output, Some(&output)).is_empty());
        assert!(!tool.format_output(&raw_output, None).is_empty());
    }

    #[tokio::test]
    async fn empty_path_returns_old_error_payload_shape() {
        let workspace_root = temp_workspace("empty_path_returns_old_error_payload_shape").await;
        let tool = WriteTool::new(&workspace_root);

        let result = tool
            .execute(
                json!({"path": "", "content": "hello"}),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: WriteOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(
            output,
            WriteOutput {
                success: false,
                bytes_written: 0,
                error: Some("Path must not be empty".to_owned())
            }
        );
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap(),
            json!({
                "success": false,
                "bytes_written": 0,
                "error": "Path must not be empty"
            })
        );

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn parent_creation_errors_use_old_prefix() {
        let workspace_root = temp_workspace("parent_creation_errors_use_old_prefix").await;
        let file_workspace_root = workspace_root.join("file_workspace_root");
        tokio::fs::write(&file_workspace_root, "not a directory")
            .await
            .unwrap();
        let tool = WriteTool::new(&file_workspace_root);

        let result = tool
            .execute(
                json!({"path": "nested/file.txt", "content": "hello"}),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: WriteOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(output.success, false);
        assert!(output
            .error
            .unwrap()
            .starts_with("Failed to create parent directories: "));

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn write_errors_use_old_prefix() {
        let workspace_root = temp_workspace("write_errors_use_old_prefix").await;
        tokio::fs::create_dir_all(workspace_root.join("existing-directory"))
            .await
            .unwrap();
        let tool = WriteTool::new(&workspace_root);

        let result = tool
            .execute(
                json!({"path": "existing-directory", "content": "hello"}),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: WriteOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(output.success, false);
        assert!(output.error.unwrap().starts_with("Failed to write file: "));

        remove_workspace(workspace_root).await;
    }

    async fn temp_workspace(test_name: &str) -> PathBuf {
        let unique_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "spectacular_tools_{test_name}_{}_{}",
            std::process::id(),
            unique_id
        ));
        tokio::fs::create_dir_all(&path).await.unwrap();
        path
    }

    async fn remove_workspace(path: PathBuf) {
        let _ = tokio::fs::remove_dir_all(path).await;
    }
}
