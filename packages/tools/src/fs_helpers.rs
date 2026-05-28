use ignore::{DirEntry, WalkBuilder};
use std::path::{Path, PathBuf};

const FILTERED_ENTRIES: &[&str] = &["node_modules", ".git"];

/// Returns whether a directory walker entry should be traversed for filesystem search tools.
pub(crate) fn is_allowed_walk_entry(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    !FILTERED_ENTRIES.contains(&name.as_ref())
}

/// Builds a repository-aware walker that includes hidden files and respects gitignore rules.
pub(crate) fn workspace_walker(search_path: &Path) -> ignore::Walk {
    WalkBuilder::new(search_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(is_allowed_walk_entry)
        .build()
}

/// Converts a platform path to a POSIX-style display path.
pub(crate) fn to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Returns files under a search path, or the path itself when it is a file.
pub(crate) fn files_to_search(search_path: &Path) -> Vec<PathBuf> {
    if search_path.is_file() {
        return vec![search_path.to_path_buf()];
    }

    workspace_walker(search_path)
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
        })
        .map(ignore::DirEntry::into_path)
        .collect()
}
