use crate::display::paint;
use crate::path::resolve_workspace_path;
use anstyle::AnsiColor;
use glob::Pattern;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::path::{Path, PathBuf};

pub const FIND_TOOL_NAME: &str = "find";

const DEFAULT_LIMIT: usize = 1000;
const MAX_OUTPUT_BYTES: usize = 50 * 1024;
const FILTERED_ENTRIES: &[&str] = &["node_modules", ".git"];

const FIND_TOOL_DESCRIPTION: &str = "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to 1000 results or 50KB (whichever is hit first).";

#[derive(Clone, Debug)]
pub struct FindTool {
    workspace_root: PathBuf,
}

impl FindTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for FindTool {
    fn name(&self) -> &str {
        FIND_TOOL_NAME
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            FIND_TOOL_NAME,
            FIND_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match files, e.g. '*.ts', '**/*.json', or 'src/**/*.spec.ts'"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (default: current directory)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default: 1000)"
                    }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
        )
    }

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("<missing pattern>");
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        format!("{pattern} in {path}")
    }

    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        if let Some(error) = output.get("error").and_then(Value::as_str) {
            let status = paint(AnsiColor::BrightRed.on_default().bold(), "failed");
            return format!("{status}: {error}");
        }

        let total = output.get("total").and_then(Value::as_u64).unwrap_or(0);
        let truncated = output
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let suffix = if truncated { " (truncated)" } else { "" };
        format!("{total} result(s){suffix}")
    }

    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        let workspace_root = self.workspace_root.clone();

        Box::pin(async move {
            let input = match serde_json::from_value::<FindInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(find_error(format!("Invalid input JSON: {error}")));
                }
            };

            Ok(serialize_output(&execute_find(&workspace_root, input)))
        })
    }
}

#[derive(Debug, Deserialize)]
struct FindInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FindOutput {
    pub results: Vec<String>,
    pub total: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn execute_find(workspace_root: &Path, input: FindInput) -> FindOutput {
    let search_dir = input.path.as_deref().unwrap_or(".");
    let search_path = resolve_workspace_path(workspace_root, search_dir);

    if !search_path.exists() {
        return FindOutput {
            results: vec![],
            total: 0,
            truncated: false,
            error: Some(format!("Path not found: {search_dir}")),
        };
    }

    let glob_pattern = match Pattern::new(&input.pattern) {
        Ok(pattern) => pattern,
        Err(error) => {
            return FindOutput {
                results: vec![],
                total: 0,
                truncated: false,
                error: Some(format!(
                    "Invalid glob pattern '{}': {}",
                    input.pattern, error
                )),
            };
        }
    };

    collect_matches(
        &search_path,
        &glob_pattern,
        input.limit.unwrap_or(DEFAULT_LIMIT),
    )
}

fn collect_matches(
    search_path: &Path,
    glob_pattern: &Pattern,
    effective_limit: usize,
) -> FindOutput {
    let mut results = Vec::new();
    let mut total_bytes = 0;
    let mut truncated = false;
    let mut total_matched = 0;

    let walker = WalkBuilder::new(search_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !FILTERED_ENTRIES.contains(&name.as_ref())
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };

        if entry.file_type().is_none_or(|file_type| file_type.is_dir()) {
            continue;
        }

        let full_path = entry.path();
        let relative = full_path.strip_prefix(search_path).unwrap_or(full_path);
        let relative_path = to_posix(relative);
        let file_name = relative
            .file_name()
            .map(|file_name| file_name.to_string_lossy().to_string())
            .unwrap_or_default();

        if !glob_pattern.matches(&relative_path) && !glob_pattern.matches(&file_name) {
            continue;
        }

        total_matched += 1;

        let line_bytes = relative_path.len() + 1;
        if total_bytes + line_bytes > MAX_OUTPUT_BYTES {
            truncated = true;
            break;
        }

        if results.len() >= effective_limit {
            truncated = true;
            break;
        }

        total_bytes += line_bytes;
        results.push(relative_path);
    }

    FindOutput {
        total: if truncated {
            total_matched
        } else {
            results.len()
        },
        results,
        truncated,
        error: None,
    }
}

fn to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn find_error(message: impl Into<String>) -> String {
    serialize_output(&FindOutput {
        results: vec![],
        total: 0,
        truncated: false,
        error: Some(message.into()),
    })
}

fn serialize_output(output: &FindOutput) -> String {
    serde_json::to_string(output).expect("find output should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{remove_workspace, temp_workspace, write_file};
    use serde_json::json;

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
}
