use spectacular_tui::WorktreeMetadata;
use std::path::Path;
use std::process::Command;

/// Returns display-safe Git worktree metadata for the current workspace.
pub(crate) fn current_worktree_metadata(workspace_root: &Path) -> Option<WorktreeMetadata> {
    let inside_worktree =
        git_trimmed_output(workspace_root, &["rev-parse", "--is-inside-work-tree"]).as_deref()
            == Some("true");
    if !inside_worktree {
        return None;
    }

    let mut label = git_trimmed_output(workspace_root, &["branch", "--show-current"])
        .or_else(|| git_trimmed_output(workspace_root, &["rev-parse", "--short", "HEAD"]))?;
    let dirty = git_output(workspace_root, &["status", "--porcelain"])
        .map(|output| !output.trim().is_empty())
        .unwrap_or(false);
    if dirty {
        label.push('*');
    }

    Some(WorktreeMetadata::new(label))
}

/// Runs a Git command and drops empty stdout values.
fn git_trimmed_output(workspace_root: &Path, args: &[&str]) -> Option<String> {
    let output = git_output(workspace_root, args)?;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_owned())
}

/// Runs a Git command against the workspace and returns stdout on success.
fn git_output(workspace_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/worktree.rs"
    ));
}
