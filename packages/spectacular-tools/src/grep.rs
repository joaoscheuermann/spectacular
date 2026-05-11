use crate::display::tool_arg_tool_arg_line;
use crate::fs_helpers::{files_to_search, to_posix};
use crate::output_preview::preview_lines;
use crate::path::resolve_workspace_path;
use glob::Pattern;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::fs;
use std::path::{Path, PathBuf};

pub const GREP_TOOL_NAME: &str = "grep";

const DEFAULT_LIMIT: usize = 100;
const MAX_OUTPUT_BYTES: usize = 50 * 1024;
const MAX_LINE_LENGTH: usize = 500;

const GREP_TOOL_DESCRIPTION: &str = "Search file contents for a pattern. Returns matching lines with file paths and line numbers. Respects .gitignore. Output is truncated to 100 matches or 50KB (whichever is hit first). Long lines are truncated to 500 chars.";

#[derive(Clone, Debug)]
pub struct GrepTool {
    workspace_root: PathBuf,
}

impl GrepTool {
    /// Creates a grep tool scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for GrepTool {
    /// Returns the stable tool name used for registration and dispatch.
    fn name(&self) -> &str {
        GREP_TOOL_NAME
    }

    /// Builds the grep tool manifest and JSON parameter schema.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            GREP_TOOL_NAME,
            GREP_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern (regex or literal string)"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory or file to search (default: current directory). Relative paths resolve against the workspace root; absolute paths and .. traversal are allowed intentionally."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Filter files by glob pattern, e.g. '*.ts' or '**/*.spec.ts'"
                    },
                    "ignoreCase": {
                        "type": "boolean",
                        "description": "Case-insensitive search (default: false)"
                    },
                    "literal": {
                        "type": "boolean",
                        "description": "Treat pattern as literal string instead of regex (default: false)"
                    },
                    "context": {
                        "type": "integer",
                        "description": "Number of lines to show before and after each match (default: 0)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of matches to return (default: 100)"
                    }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
        )
    }

    /// Formats grep arguments as a pattern and search path summary.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("<missing pattern>");
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        format!("{pattern} in {path}")
    }

    /// Formats grep arguments as a styled renderer call line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        let pattern = arguments
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("<missing pattern>");
        let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
        tool_arg_tool_arg_line("Search", pattern, "in", path)
    }

    /// Formats grep output as bounded match preview lines or raw fallback text.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return crate::output_preview::preview_text(raw_output);
        };

        preview_lines(grep_output_lines(output))
    }

    /// Executes the content search and serializes the grep output payload.
    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        let workspace_root = self.workspace_root.clone();

        Box::pin(async move {
            let input = match serde_json::from_value::<GrepInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(grep_error(format!("Invalid input JSON: {error}")));
                }
            };

            Ok(serialize_output(&execute_grep(&workspace_root, input)))
        })
    }
}

#[derive(Debug, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default, rename = "ignoreCase")]
    ignore_case: Option<bool>,
    #[serde(default)]
    literal: Option<bool>,
    #[serde(default)]
    context: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GrepMatch {
    pub file: String,
    pub line: usize,
    pub text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<String>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GrepOutput {
    pub matches: Vec<GrepMatch>,
    pub total: usize,
    pub truncated: bool,
    pub lines_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Validates grep input, compiles filters, and collects content matches.
fn execute_grep(workspace_root: &Path, input: GrepInput) -> GrepOutput {
    let search_dir = input.path.as_deref().unwrap_or(".");
    let search_path = resolve_workspace_path(workspace_root, search_dir);

    if !search_path.exists() {
        return GrepOutput {
            matches: vec![],
            total: 0,
            truncated: false,
            lines_truncated: false,
            error: Some(format!("Path not found: {search_dir}")),
        };
    }

    let regex_pattern = if input.literal.unwrap_or(false) {
        regex::escape(&input.pattern)
    } else {
        input.pattern.clone()
    };
    let regex = match RegexBuilder::new(&regex_pattern)
        .case_insensitive(input.ignore_case.unwrap_or(false))
        .build()
    {
        Ok(regex) => regex,
        Err(error) => {
            return GrepOutput {
                matches: vec![],
                total: 0,
                truncated: false,
                lines_truncated: false,
                error: Some(format!("Invalid regex pattern: {error}")),
            };
        }
    };

    let glob_pattern = match input.glob.as_deref() {
        Some(glob) => match Pattern::new(glob) {
            Ok(pattern) => Some(pattern),
            Err(error) => {
                return GrepOutput {
                    matches: vec![],
                    total: 0,
                    truncated: false,
                    lines_truncated: false,
                    error: Some(format!("Invalid glob pattern '{glob}': {error}")),
                };
            }
        },
        None => None,
    };

    let files = files_to_search(&search_path);
    collect_matches(
        &search_path,
        files,
        input.context.unwrap_or(0),
        input.limit.unwrap_or(DEFAULT_LIMIT).max(1),
        &regex,
        glob_pattern.as_ref(),
    )
}

/// Reads candidate files and collects matching lines within result and byte limits.
fn collect_matches(
    search_path: &Path,
    files: Vec<PathBuf>,
    context_lines: usize,
    effective_limit: usize,
    regex: &regex::Regex,
    glob_pattern: Option<&Pattern>,
) -> GrepOutput {
    let is_file = search_path.is_file();
    let mut results = Vec::new();
    let mut total_bytes = 0;
    let mut truncated = false;
    let mut lines_truncated = false;

    'outer: for file_path in &files {
        let relative = relative_file_path(search_path, file_path, is_file);
        if !matches_optional_glob(file_path, &relative, glob_pattern) {
            continue;
        }

        let content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        let all_lines = content.lines().collect::<Vec<_>>();

        for (line_idx, line) in all_lines.iter().enumerate() {
            if !regex.is_match(line) {
                continue;
            }

            let (text, was_truncated) = truncate_line(line);
            lines_truncated |= was_truncated;
            let (context_before, before_truncated) =
                context_before(&all_lines, line_idx, context_lines);
            let (context_after, after_truncated) =
                context_after(&all_lines, line_idx, context_lines);
            lines_truncated |= before_truncated || after_truncated;

            let entry_estimate = relative.len() + text.len() + 20;
            if total_bytes + entry_estimate > MAX_OUTPUT_BYTES {
                truncated = true;
                break 'outer;
            }
            total_bytes += entry_estimate;

            results.push(GrepMatch {
                file: relative.clone(),
                line: line_idx + 1,
                text,
                context_before,
                context_after,
            });

            if results.len() >= effective_limit {
                truncated = true;
                break 'outer;
            }
        }
    }

    GrepOutput {
        total: results.len(),
        matches: results,
        truncated,
        lines_truncated,
        error: None,
    }
}

/// Reports whether a file passes the optional glob filter by name or relative path.
fn matches_optional_glob(file_path: &Path, relative: &str, glob_pattern: Option<&Pattern>) -> bool {
    let Some(pattern) = glob_pattern else {
        return true;
    };

    let name = file_path
        .file_name()
        .map(|file_name| file_name.to_string_lossy().to_string())
        .unwrap_or_default();
    pattern.matches(&name) || pattern.matches(relative)
}

/// Formats a file path relative to the search root for output.
fn relative_file_path(search_path: &Path, file_path: &Path, is_file: bool) -> String {
    if is_file {
        return file_path
            .file_name()
            .map(|file_name| file_name.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    file_path
        .strip_prefix(search_path)
        .map(to_posix)
        .unwrap_or_else(|_| to_posix(file_path))
}

/// Returns truncated context lines before a matching line.
fn context_before(lines: &[&str], line_idx: usize, context_lines: usize) -> (Vec<String>, bool) {
    if context_lines == 0 {
        return (vec![], false);
    }

    let mut truncated = false;
    let start = line_idx.saturating_sub(context_lines);
    let context = (start..line_idx)
        .map(|index| {
            let (line, was_truncated) = truncate_line(lines[index]);
            truncated |= was_truncated;
            line
        })
        .collect();

    (context, truncated)
}

/// Returns truncated context lines after a matching line.
fn context_after(lines: &[&str], line_idx: usize, context_lines: usize) -> (Vec<String>, bool) {
    if context_lines == 0 {
        return (vec![], false);
    }

    let mut truncated = false;
    let end = (line_idx + 1 + context_lines).min(lines.len());
    let context = ((line_idx + 1)..end)
        .map(|index| {
            let (line, was_truncated) = truncate_line(lines[index]);
            truncated |= was_truncated;
            line
        })
        .collect();

    (context, truncated)
}

/// Truncates long grep lines at a UTF-8 boundary and reports whether truncation occurred.
fn truncate_line(line: &str) -> (String, bool) {
    if line.len() <= MAX_LINE_LENGTH {
        return (line.to_owned(), false);
    }

    let split_at = line
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= MAX_LINE_LENGTH)
        .last()
        .unwrap_or(0);
    (format!("{}... [truncated]", &line[..split_at]), true)
}

/// Serializes a failed grep output payload with a user-facing error message.
fn grep_error(message: impl Into<String>) -> String {
    serialize_output(&GrepOutput {
        matches: vec![],
        total: 0,
        truncated: false,
        lines_truncated: false,
        error: Some(message.into()),
    })
}

/// Serializes a grep output payload to JSON.
fn serialize_output(output: &GrepOutput) -> String {
    serde_json::to_string(output).expect("grep output should serialize")
}

/// Extracts grep match previews or error text from the result payload.
fn grep_output_lines(output: &Value) -> Vec<String> {
    if let Some(error) = output.get("error").and_then(Value::as_str) {
        return vec![error.to_owned()];
    }

    output
        .get("matches")
        .and_then(Value::as_array)
        .map(|matches| matches.iter().map(grep_match_line).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![output.to_string()])
}

/// Formats one grep match as `file:line:text` for display.
fn grep_match_line(matched: &Value) -> String {
    let file = matched.get("file").and_then(Value::as_str).unwrap_or("");
    let line = matched.get("line").and_then(Value::as_u64).unwrap_or(0);
    let text = matched.get("text").and_then(Value::as_str).unwrap_or("");
    format!("{file}:{line}: {text}")
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/grep.rs"));
}
