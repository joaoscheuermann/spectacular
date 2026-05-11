use std::path::{Component, Path, PathBuf};

/// Resolves a tool path against the workspace root with lexical `.` and `..` normalization.
pub fn resolve_workspace_path(workspace_root: impl AsRef<Path>, path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.as_ref().join(path)
    };

    normalize_lexically(&joined)
}

/// Normalizes path components without touching the filesystem or resolving symlinks.
fn normalize_lexically(path: &Path) -> PathBuf {
    path.components()
        .fold(PathBuf::new(), |mut normalized, component| {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
                Component::RootDir => normalized.push(component.as_os_str()),
                Component::Normal(part) => normalized.push(part),
            }
            normalized
        })
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/path.rs"));
}
