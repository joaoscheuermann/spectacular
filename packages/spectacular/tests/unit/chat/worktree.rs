use super::current_worktree_metadata;
use std::time::{SystemTime, UNIX_EPOCH};

/// Verifies non-Git workspaces fail softly without footer metadata.
#[test]
fn non_git_directory_returns_none() {
    let workspace = unique_temp_dir("non-git");
    std::fs::create_dir_all(&workspace).unwrap();

    assert_eq!(current_worktree_metadata(&workspace), None);

    std::fs::remove_dir_all(workspace).unwrap();
}

/// Builds a unique temporary directory path for a test case.
fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-worktree-{name}-{suffix}"))
}
