use crate::display::tool_arg_tool_arg_line;
use crate::fs_helpers::{to_posix, workspace_walker};
use crate::output_preview::preview_lines;
use crate::path::resolve_workspace_path;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::path::{Path, PathBuf};

pub const FIND_TOOL_NAME: &str = "find";

const DEFAULT_LIMIT: usize = 1000;
const MAX_OUTPUT_BYTES: usize = 50 * 1024;

const FIND_TOOL_DESCRIPTION: &str = "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to 1000 results or 50KB (whichever is hit first).";

#[derive(Clone, Debug)]
pub struct FindTool {
    workspace_root: PathBuf,
}

impl FindTool {
    /// Creates a find tool scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for FindTool {
    /// Returns the stable tool name used for registration and dispatch.
    fn name(&self) -> &str {
        FIND_TOOL_NAME
    }

    /// Builds the find tool manifest and JSON parameter schema.
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
                        "description": "Directory to search in (default: current directory). Relative paths resolve against the workspace root; absolute paths and .. traversal are allowed intentionally."
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

    /// Formats find arguments as a pattern and search path summary.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("<missing pattern>");
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        format!("{pattern} in {path}")
    }

    /// Formats find arguments as a styled renderer call line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("<missing pattern>");
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        tool_arg_tool_arg_line("Search files", pattern, "in", path)
    }

    /// Formats find output as a bounded list of paths or an error preview.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return crate::output_preview::preview_text(raw_output);
        };

        preview_lines(find_output_lines(output))
    }

    /// Executes the glob search and serializes the output payload.
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

/// Validates find input, resolves the search path, and collects matching files.
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

/// Walks the search path and collects glob matches within result and byte limits.
fn collect_matches(
    search_path: &Path,
    glob_pattern: &Pattern,
    effective_limit: usize,
) -> FindOutput {
    let mut results = Vec::new();
    let mut total_bytes = 0;
    let mut truncated = false;
    let mut total_matched = 0;

    for entry in workspace_walker(search_path) {
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

/// Serializes a failed find output payload with a user-facing error message.
fn find_error(message: impl Into<String>) -> String {
    serialize_output(&FindOutput {
        results: vec![],
        total: 0,
        truncated: false,
        error: Some(message.into()),
    })
}

/// Serializes a find output payload to JSON.
fn serialize_output(output: &FindOutput) -> String {
    serde_json::to_string(output).expect("find output should serialize")
}

/// Extracts returned paths or error text from the find result payload.
fn find_output_lines(output: &Value) -> Vec<String> {
    if let Some(error) = output.get("error").and_then(Value::as_str) {
        return vec![error.to_owned()];
    }

    output
        .get("results")
        .and_then(Value::as_array)
        .map(|results| {
            results
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![output.to_string()])
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/find.rs"));
}
