use super::*;
use crate::test_support::{remove_workspace, temp_workspace, write_file};
use serde_json::json;

/// Verifies that a real edit updates a file and returns a diff with the first changed line.
#[tokio::test]
async fn real_edit_updates_file_and_returns_diff_with_first_changed_line() {
    let workspace_root = temp_workspace("edit_real_file").await;
    write_file(
        &workspace_root,
        "src/lib.rs",
        "fn main() {\n    old_call();\n}\n",
    )
    .await;
    let tool = EditTool::new(&workspace_root);

    let result = tool
        .execute(
            json!({
                "path": "src/lib.rs",
                "edits": [{
                    "oldText": "    old_call();",
                    "newText": "    new_call();"
                }]
            }),
            Cancellation::default(),
        )
        .await
        .unwrap();
    let output: EditOutput = serde_json::from_str(&result).unwrap();

    assert!(output.success);
    assert_eq!(output.first_changed_line, Some(2));
    assert!(output.diff.contains("2 -    old_call();"));
    assert!(output.diff.contains("2 +    new_call();"));
    assert_eq!(
        tokio::fs::read_to_string(workspace_root.join("src/lib.rs"))
            .await
            .unwrap(),
        "fn main() {\n    new_call();\n}\n"
    );

    remove_workspace(workspace_root).await;
}

/// Verifies that CRLF input keeps CRLF line endings after replacement.
#[tokio::test]
async fn crlf_input_preserves_crlf_after_edit() {
    let workspace_root = temp_workspace("edit_preserves_crlf").await;
    write_file(&workspace_root, "src/lib.rs", "one\r\ntwo\r\nthree\r\n").await;
    let tool = EditTool::new(&workspace_root);

    let output = execute_edit_json(
        &tool,
        json!({
            "path": "src/lib.rs",
            "edits": [{
                "oldText": "two",
                "newText": "TWO"
            }]
        }),
    )
    .await;
    let bytes = tokio::fs::read(workspace_root.join("src/lib.rs"))
        .await
        .unwrap();
    let content = String::from_utf8(bytes).unwrap();

    assert!(output.success);
    assert_eq!(content, "one\r\nTWO\r\nthree\r\n");
    assert!(!content.contains("one\nTWO"));

    remove_workspace(workspace_root).await;
}

/// Verifies that UTF-8 BOM bytes are preserved when editing text.
#[tokio::test]
async fn utf8_bom_is_preserved() {
    let workspace_root = temp_workspace("edit_preserves_bom").await;
    let path = workspace_root.join("bom.txt");
    tokio::fs::write(&path, b"\xEF\xBB\xBFalpha\nbeta\n")
        .await
        .unwrap();
    let tool = EditTool::new(&workspace_root);

    let output = execute_edit_json(
        &tool,
        json!({
            "path": "bom.txt",
            "edits": [{
                "oldText": "beta",
                "newText": "gamma"
            }]
        }),
    )
    .await;
    let bytes = tokio::fs::read(path).await.unwrap();

    assert!(output.success);
    assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
    assert_eq!(String::from_utf8(bytes).unwrap(), "\u{FEFF}alpha\ngamma\n");

    remove_workspace(workspace_root).await;
}

/// Verifies that duplicate old text returns a guidance error asking for more context.
#[tokio::test]
async fn duplicate_old_text_returns_more_context_error() {
    let workspace_root = temp_workspace("edit_duplicate_old_text").await;
    write_file(&workspace_root, "src/lib.rs", "same\nsame\n").await;
    let tool = EditTool::new(&workspace_root);

    let output = execute_edit_json(
        &tool,
        json!({
            "path": "src/lib.rs",
            "edits": [{
                "oldText": "same",
                "newText": "changed"
            }]
        }),
    )
    .await;

    assert!(!output.success);
    assert!(output
        .error
        .unwrap()
        .contains("Please provide more context."));

    remove_workspace(workspace_root).await;
}

/// Verifies that overlapping replacements are rejected before writing the file.
#[tokio::test]
async fn overlapping_edits_return_error() {
    let workspace_root = temp_workspace("edit_overlapping").await;
    write_file(&workspace_root, "src/lib.rs", "abcdef\n").await;
    let tool = EditTool::new(&workspace_root);

    let output = execute_edit_json(
        &tool,
        json!({
            "path": "src/lib.rs",
            "edits": [
                {
                    "oldText": "abc",
                    "newText": "ABC"
                },
                {
                    "oldText": "bcd",
                    "newText": "BCD"
                }
            ]
        }),
    )
    .await;

    assert!(!output.success);
    assert!(output.error.unwrap().contains("overlap"));

    remove_workspace(workspace_root).await;
}

/// Verifies that snake_case edit field aliases deserialize successfully.
#[tokio::test]
async fn old_text_and_new_text_aliases_are_accepted() {
    let workspace_root = temp_workspace("edit_aliases").await;
    write_file(&workspace_root, "src/lib.rs", "alpha\n").await;
    let tool = EditTool::new(&workspace_root);

    let output = execute_edit_json(
        &tool,
        json!({
            "path": "src/lib.rs",
            "edits": [{
                "old_text": "alpha",
                "new_text": "beta"
            }]
        }),
    )
    .await;

    assert!(output.success);
    assert_eq!(
        tokio::fs::read_to_string(workspace_root.join("src/lib.rs"))
            .await
            .unwrap(),
        "beta\n"
    );

    remove_workspace(workspace_root).await;
}

/// Verifies that fuzzy matching tolerates trailing whitespace differences.
#[tokio::test]
async fn trailing_whitespace_tolerance_matches_old_text() {
    let workspace_root = temp_workspace("edit_trailing_whitespace").await;
    write_file(&workspace_root, "src/lib.rs", "alpha   \nbeta\n").await;
    let tool = EditTool::new(&workspace_root);

    let output = execute_edit_json(
        &tool,
        json!({
            "path": "src/lib.rs",
            "edits": [{
                "oldText": "alpha\nbeta",
                "newText": "gamma\nbeta"
            }]
        }),
    )
    .await;

    assert!(output.success);
    assert_eq!(
        tokio::fs::read_to_string(workspace_root.join("src/lib.rs"))
            .await
            .unwrap(),
        "gamma\nbeta"
    );

    remove_workspace(workspace_root).await;
}

/// Executes the edit tool and deserializes the JSON output for assertions.
async fn execute_edit_json(tool: &EditTool, arguments: Value) -> EditOutput {
    let result = tool
        .execute(arguments, Cancellation::default())
        .await
        .unwrap();
    serde_json::from_str(&result).unwrap()
}
