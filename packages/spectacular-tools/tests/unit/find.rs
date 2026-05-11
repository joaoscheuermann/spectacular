use super::*;
use crate::test_support::{remove_workspace, temp_workspace, write_file};
use serde_json::json;

/// Verifies that find returns matching files and marks output as truncated at the default limit.
#[tokio::test]
async fn find_locates_matching_files_and_respects_default_limit_truncation() {
    let workspace_root = temp_workspace("find_default_limit").await;
    for index in 0..=DEFAULT_LIMIT {
        write_file(
            &workspace_root,
            &format!("matches/file_{index:04}.target"),
            "content",
        )
        .await;
    }
    write_file(&workspace_root, "matches/other.txt", "content").await;

    let tool = FindTool::new(&workspace_root);
    let result = tool
        .execute(
            json!({"pattern": "*.target", "path": "matches"}),
            Cancellation::default(),
        )
        .await
        .unwrap();
    let output: FindOutput = serde_json::from_str(&result).unwrap();

    assert_eq!(output.results.len(), DEFAULT_LIMIT);
    assert_eq!(output.total, DEFAULT_LIMIT + 1);
    assert!(output.truncated);
    assert!(output.results.iter().all(|path| path.ends_with(".target")));

    remove_workspace(workspace_root).await;
}
