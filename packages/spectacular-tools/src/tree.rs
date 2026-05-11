use crate::display::tool_arg_line;
use crate::output_preview::preview_text;
use crate::path::resolve_workspace_path;
use glob::Pattern;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Deserialize;
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::path::{Path, PathBuf};

pub const TREE_TOOL_NAME: &str = "tree";

const HIDDEN_EXCEPTIONS: &[&str] = &[".agents"];

const TREE_TOOL_DESCRIPTION: &str = "Display directory structure as an ASCII tree. Directories are listed first, then files, both sorted alphabetically. Respects .gitignore and excludes hidden files except .agents.";

#[derive(Clone, Debug)]
pub struct TreeTool {
    workspace_root: PathBuf,
}

impl TreeTool {
    /// Creates a tree tool scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for TreeTool {
    /// Returns the stable tool name used for registration and dispatch.
    fn name(&self) -> &str {
        TREE_TOOL_NAME
    }

    /// Builds the tree tool manifest and JSON parameter schema.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            TREE_TOOL_NAME,
            TREE_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Root directory to display (default: current directory). Relative paths resolve against the workspace root; absolute paths and .. traversal are allowed intentionally."
                    },
                    "exclude": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Glob patterns to exclude from the tree"
                    }
                },
                "additionalProperties": false
            }),
        )
    }

    /// Formats tree arguments as the requested root path.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        path.to_owned()
    }

    /// Formats tree arguments as a styled renderer call line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        tool_arg_line("List", path)
    }

    /// Formats tree output as a bounded text preview.
    fn format_output(&self, raw_output: &str, _parsed_output: Option<&Value>) -> ToolDisplay {
        preview_text(raw_output)
    }

    /// Executes tree rendering and returns the plain-text tree output.
    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        let workspace_root = self.workspace_root.clone();

        Box::pin(async move {
            let input = match serde_json::from_value::<TreeInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(format!("Error: invalid input JSON: {error}"));
                }
            };

            Ok(execute_tree(&workspace_root, input))
        })
    }
}

#[derive(Debug, Deserialize)]
struct TreeInput {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    exclude: Option<Vec<String>>,
}

/// Resolves tree input, validates the root, and renders the directory tree.
fn execute_tree(workspace_root: &Path, input: TreeInput) -> String {
    let raw_path = input.path.unwrap_or_default();
    let display_path = if raw_path.is_empty() { "." } else { &raw_path };
    let root = resolve_workspace_path(workspace_root, display_path);

    if !root.exists() {
        return format!("Error: path not found: {display_path}");
    }

    if !root.is_dir() {
        return format!("Error: path is not a directory: {display_path}");
    }

    let exclude_patterns = match parse_exclude_patterns(input.exclude.unwrap_or_default()) {
        Ok(patterns) => patterns,
        Err(error) => return error,
    };

    let root_name = root.file_name().unwrap_or_default().to_string_lossy();
    if exclude_patterns
        .iter()
        .any(|pattern| pattern.matches(&root_name))
    {
        return format!("Error: exclude pattern matches the root directory: {root_name}");
    }

    let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
    let root_display = canonical
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .to_string();

    let mut gitignores = Vec::new();
    let mut output = format!("{root_display}\n");
    build_tree(&root, "", &mut gitignores, &exclude_patterns, &mut output);
    output
}

/// Parses exclude glob patterns and returns a user-facing error for invalid patterns.
fn parse_exclude_patterns(patterns: Vec<String>) -> Result<Vec<Pattern>, String> {
    patterns
        .into_iter()
        .map(|pattern| {
            Pattern::new(&pattern)
                .map_err(|error| format!("Error: invalid exclude pattern '{pattern}': {error}"))
        })
        .collect()
}

/// Recursively appends sorted visible entries to the ASCII tree output.
fn build_tree(
    dir: &Path,
    prefix: &str,
    gitignores: &mut Vec<Gitignore>,
    exclude_patterns: &[Pattern],
    output: &mut String,
) {
    let added = maybe_push_gitignore(dir, gitignores);
    let entries = match filtered_sorted_entries(dir, gitignores, exclude_patterns) {
        Ok(entries) => entries,
        Err(_) => {
            if added {
                gitignores.pop();
            }
            return;
        }
    };

    for (index, (path, is_dir)) in entries.iter().enumerate() {
        let is_last = index == entries.len() - 1;
        let connector = if is_last { "`-- " } else { "|-- " };
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        if *is_dir {
            output.push_str(&format!("{prefix}{connector}{name}/"));
            let mark = output.len();
            output.push('\n');

            let child_prefix = if is_last {
                format!("{prefix}    ")
            } else {
                format!("{prefix}|   ")
            };
            build_tree(path, &child_prefix, gitignores, exclude_patterns, output);

            if output.len() == mark + 1 {
                output.truncate(mark);
                output.push_str(" (empty)\n");
            }
            continue;
        }

        output.push_str(&format!("{prefix}{connector}{name}\n"));
    }

    if added {
        gitignores.pop();
    }
}

/// Reads visible directory entries, applies ignore/exclude rules, and sorts directories before files.
fn filtered_sorted_entries(
    dir: &Path,
    gitignores: &[Gitignore],
    exclude_patterns: &[Pattern],
) -> std::io::Result<Vec<(PathBuf, bool)>> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        if name.starts_with('.') && !HIDDEN_EXCEPTIONS.contains(&name.as_ref()) {
            continue;
        }

        if is_symlink(&path) {
            continue;
        }

        let is_dir = path.is_dir();
        if is_gitignored(&path, is_dir, gitignores) || is_excluded(&path, exclude_patterns) {
            continue;
        }

        if is_dir {
            dirs.push(path);
        } else {
            files.push(path);
        }
    }

    dirs.sort_by_key(|path| case_insensitive_name(path));
    files.sort_by_key(|path| case_insensitive_name(path));

    let mut entries = Vec::with_capacity(dirs.len() + files.len());
    entries.extend(dirs.into_iter().map(|path| (path, true)));
    entries.extend(files.into_iter().map(|path| (path, false)));
    Ok(entries)
}

/// Reports whether a path is a symbolic link that should be skipped.
fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// Reports whether a path matches any user-supplied exclude pattern.
fn is_excluded(path: &Path, exclude_patterns: &[Pattern]) -> bool {
    if exclude_patterns.is_empty() {
        return false;
    }

    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let path_str = path.to_string_lossy().replace('\\', "/");
    exclude_patterns
        .iter()
        .any(|pattern| pattern.matches(&name) || pattern.matches(&path_str))
}

/// Reports whether stacked gitignore matchers ignore the given path.
fn is_gitignored(path: &Path, is_dir: bool, gitignores: &[Gitignore]) -> bool {
    for gitignore in gitignores.iter().rev() {
        match gitignore.matched(path, is_dir) {
            ignore::Match::Ignore(_) => return true,
            ignore::Match::Whitelist(_) => return false,
            ignore::Match::None => continue,
        }
    }
    false
}

/// Loads a directory-local .gitignore onto the matcher stack when present.
fn maybe_push_gitignore(dir: &Path, gitignores: &mut Vec<Gitignore>) -> bool {
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.is_file() {
        return false;
    }

    let mut builder = GitignoreBuilder::new(dir);
    builder.add(gitignore_path);
    match builder.build() {
        Ok(gitignore) => {
            gitignores.push(gitignore);
            true
        }
        Err(_) => false,
    }
}

/// Returns a lowercase file name used for stable case-insensitive sorting.
fn case_insensitive_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/tree.rs"));
}
