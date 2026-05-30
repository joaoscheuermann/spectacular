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

/// Verifies that absolute write paths are preserved instead of being joined to the workspace root.
#[tokio::test]
async fn absolute_path_writes_to_absolute_path() {
    let temp_root = temp_workspace("absolute_path_writes_to_absolute_path").await;
    let workspace_root = temp_root.join("workspace");
    let absolute_path = temp_root.join("absolute-write.txt");
    tokio::fs::create_dir_all(&workspace_root).await.unwrap();
    let tool = WriteTool::new(&workspace_root);

    let result = tool
        .execute(
            json!({"path": absolute_path.to_string_lossy(), "content": "absolute"}),
            Cancellation::default(),
        )
        .await
        .unwrap();
    let output: WriteOutput = serde_json::from_str(&result).unwrap();

    assert!(output.success);
    assert_eq!(
        tokio::fs::read_to_string(&absolute_path).await.unwrap(),
        "absolute"
    );
    assert!(!workspace_root.join("absolute-write.txt").exists());

    remove_workspace(temp_root).await;
}

/// Verifies that parent traversal writes using the current lexical path semantics.
#[tokio::test]
async fn parent_traversal_writes_outside_workspace_root() {
    let temp_root = temp_workspace("parent_traversal_writes_outside_workspace_root").await;
    let workspace_root = temp_root.join("workspace");
    tokio::fs::create_dir_all(&workspace_root).await.unwrap();
    let tool = WriteTool::new(&workspace_root);

    let result = tool
        .execute(
            json!({"path": "../outside-write.txt", "content": "outside"}),
            Cancellation::default(),
        )
        .await
        .unwrap();
    let output: WriteOutput = serde_json::from_str(&result).unwrap();

    assert!(output.success);
    assert_eq!(
        tokio::fs::read_to_string(temp_root.join("outside-write.txt"))
            .await
            .unwrap(),
        "outside"
    );
    assert!(!workspace_root.join("outside-write.txt").exists());

    remove_workspace(temp_root).await;
}

/// Verifies that the write tool registers through ToolStorage with its expected manifest.
#[test]
fn write_manifest_registers_through_tool_storage() {
    let mut storage = agent::ToolStorage::default();
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
