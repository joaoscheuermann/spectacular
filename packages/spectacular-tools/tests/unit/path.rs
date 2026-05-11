use super::*;
use std::path::PathBuf;

/// Verifies that relative paths resolve under the workspace root.
#[test]
fn relative_path_resolves_under_workspace_root() {
    assert_eq!(
        resolve_workspace_path(PathBuf::from("/workspace"), "src/lib.rs"),
        PathBuf::from("/workspace/src/lib.rs")
    );
}

/// Verifies that lexical dot and parent components are normalized.
#[test]
fn dot_and_parent_components_are_normalized_lexically() {
    assert_eq!(
        resolve_workspace_path(PathBuf::from("/workspace/project"), "src/../README.md"),
        PathBuf::from("/workspace/project/README.md")
    );
}

/// Verifies that absolute paths remain host-absolute after lexical normalization.
#[test]
fn absolute_paths_remain_absolute() {
    let absolute = if cfg!(windows) {
        PathBuf::from(r"C:\workspace\..\outside.txt")
    } else {
        PathBuf::from("/workspace/../outside.txt")
    };
    let expected = if cfg!(windows) {
        PathBuf::from(r"C:\outside.txt")
    } else {
        PathBuf::from("/outside.txt")
    };

    assert_eq!(resolve_workspace_path(PathBuf::from("/workspace"), absolute), expected);
}
