use crate::display::paint;
use crate::path::resolve_workspace_path;
use anstyle::AnsiColor;
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
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for TreeTool {
    fn name(&self) -> &str {
        TREE_TOOL_NAME
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            TREE_TOOL_NAME,
            TREE_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Root directory to display (default: current directory)"
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

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        path.to_owned()
    }

    fn format_output(&self, raw_output: &str, _parsed_output: Option<&Value>) -> ToolDisplay {
        if raw_output.starts_with("Error:") {
            let status = paint(AnsiColor::BrightRed.on_default().bold(), "failed");
            return format!("{status}: {raw_output}");
        }

        let lines = raw_output.lines().count();
        format!("{lines} line(s)")
    }

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

fn parse_exclude_patterns(patterns: Vec<String>) -> Result<Vec<Pattern>, String> {
    patterns
        .into_iter()
        .map(|pattern| {
            Pattern::new(&pattern)
                .map_err(|error| format!("Error: invalid exclude pattern '{pattern}': {error}"))
        })
        .collect()
}

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

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

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

fn case_insensitive_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{remove_workspace, temp_workspace, write_file};
    use serde_json::json;

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

        assert!(output.contains("|-- .agents/"));
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
}
