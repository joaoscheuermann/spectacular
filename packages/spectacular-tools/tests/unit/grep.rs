use super::*;
use crate::test_support::{remove_workspace, temp_workspace, write_file};
use serde_json::json;

/// Verifies that grep finds matching symbols in multiple files with requested context lines.
#[tokio::test]
async fn grep_finds_symbol_in_multiple_files_with_context() {
    let workspace_root = temp_workspace("grep_symbol_context").await;
    write_file(
        &workspace_root,
        "src/one.rs",
        "before one\nlet symbol = TargetSymbol;\nafter one\n",
    )
    .await;
    write_file(
        &workspace_root,
        "src/two.rs",
        "before two\nTargetSymbol::call();\nafter two\n",
    )
    .await;
    write_file(&workspace_root, "src/ignored.txt", "TargetSymbol\n").await;

    let tool = GrepTool::new(&workspace_root);
    let result = tool
        .execute(
            json!({
                "pattern": "TargetSymbol",
                "path": "src",
                "glob": "*.rs",
                "context": 1
            }),
            Cancellation::default(),
        )
        .await
        .unwrap();
    let output: GrepOutput = serde_json::from_str(&result).unwrap();

    assert_eq!(output.total, 2);
    assert!(!output.truncated);
    assert_match(
        &output,
        "one.rs",
        2,
        "let symbol = TargetSymbol;",
        "before one",
        "after one",
    );
    assert_match(
        &output,
        "two.rs",
        2,
        "TargetSymbol::call();",
        "before two",
        "after two",
    );

    remove_workspace(workspace_root).await;
}

/// Verifies that invalid glob filters return a structured error payload.
#[tokio::test]
async fn invalid_glob_returns_clear_error_payload() {
    let workspace_root = temp_workspace("grep_invalid_glob").await;
    write_file(&workspace_root, "src/one.rs", "TargetSymbol\n").await;

    let tool = GrepTool::new(&workspace_root);
    let result = tool
        .execute(
            json!({"pattern": "TargetSymbol", "path": "src", "glob": "["}),
            Cancellation::default(),
        )
        .await
        .unwrap();
    let output: GrepOutput = serde_json::from_str(&result).unwrap();

    assert_eq!(output.matches, vec![]);
    assert_eq!(output.total, 0);
    assert!(output
        .error
        .unwrap()
        .starts_with("Invalid glob pattern '[': "));

    remove_workspace(workspace_root).await;
}

/// Reports whether the expected grep match appears in the output payload.
fn assert_match(
    output: &GrepOutput,
    file_suffix: &str,
    line: usize,
    text: &str,
    before: &str,
    after: &str,
) {
    let found = output.matches.iter().any(|matched| {
        matched.file.ends_with(file_suffix)
            && matched.line == line
            && matched.text == text
            && matched.context_before == vec![before.to_owned()]
            && matched.context_after == vec![after.to_owned()]
    });

    assert!(found, "missing grep match for {file_suffix}");
}
