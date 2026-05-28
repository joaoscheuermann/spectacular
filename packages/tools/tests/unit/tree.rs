use super::*;
use crate::test_support::{remove_workspace, temp_workspace, write_file};
use serde_json::json;

/// Verifies ASCII tree rendering, .agents visibility, and default hidden-directory filtering.
#[tokio::test]
async fn tree_uses_ascii_connectors_and_includes_agents_but_excludes_other_hidden() {
    let workspace_root = temp_workspace("tree_hidden_ascii").await;
    write_file(&workspace_root, ".agents/skill.md", "skill").await;
    write_file(&workspace_root, ".hidden/secret.txt", "secret").await;
    write_file(&workspace_root, "src/lib.rs", "lib").await;
    write_file(&workspace_root, "visible.txt", "visible").await;

    let tool = TreeTool::new(&workspace_root);
    let output = tool
        .execute(json!({"path": "."}), Cancellation::default())
        .await
        .unwrap();

    assert!(output.contains("|-- .agents/") || output.contains("`-- .agents/"));
    assert!(output.contains("|   `-- skill.md") || output.contains("    `-- skill.md"));
    assert!(output.contains("`-- visible.txt") || output.contains("|-- visible.txt"));
    assert!(output.contains("|--"));
    assert!(output.contains("`-- "));
    assert!(output.contains("|   "));
    assert!(!output.contains(".hidden"));
    assert!(!output.contains('\u{251C}'));
    assert!(!output.contains('\u{2514}'));
    assert!(!output.contains('\u{2502}'));
    assert!(!output.contains('\u{2500}'));

    remove_workspace(workspace_root).await;
}
