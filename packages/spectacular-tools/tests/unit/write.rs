use super::*;
use crate::test_support::{remove_workspace, temp_workspace};
use serde_json::json;

/// Verifies that writing foo.txt creates the file under the workspace root and returns the legacy payload.
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
            diff: "Edited foo.txt (+1 -0)\n1 +hello".to_owned(),
            error: None
        }
    );
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap(),
        json!({
            "success": true,
            "bytes_written": 5,
            "diff": "Edited foo.txt (+1 -0)\n1 +hello"
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

/// Verifies that writing nested paths automatically creates parent directories.
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

    assert!(output.success);
    assert_eq!(
        tokio::fs::read_to_string(workspace_root.join("notes/today/file.txt"))
            .await
            .unwrap(),
        "nested"
    );

    remove_workspace(workspace_root).await;
}

/// Verifies that the write tool registers through ToolStorage with its expected manifest.
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

/// Verifies that write input and output formatters always return displayable text.
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

/// Verifies that empty paths return the legacy error payload shape.
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
            diff: String::new(),
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

/// Verifies that parent directory creation errors keep the expected error prefix.
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

    assert!(!output.success);
    assert!(output
        .error
        .unwrap()
        .starts_with("Failed to create parent directories: "));

    remove_workspace(workspace_root).await;
}

/// Verifies that file write errors keep the expected error prefix.
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

    assert!(!output.success);
    assert!(output.error.unwrap().starts_with("Failed to write file: "));

    remove_workspace(workspace_root).await;
}
