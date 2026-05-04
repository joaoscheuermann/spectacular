use crate::display::paint;
use crate::path::resolve_workspace_path;
use anstyle::AnsiColor;
use glob::Pattern;
use ignore::WalkBuilder;
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
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for GrepTool {
    fn name(&self) -> &str {
        GREP_TOOL_NAME
    }

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
                        "description": "Directory or file to search (default: current directory)"
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
        format!("{total} match(es){suffix}")
    }

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

fn files_to_search(search_path: &Path) -> Vec<PathBuf> {
    if search_path.is_file() {
        return vec![search_path.to_path_buf()];
    }

    WalkBuilder::new(search_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            name != "node_modules" && name != ".git"
        })
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
        })
        .map(ignore::DirEntry::into_path)
        .collect()
}

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

fn to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn grep_error(message: impl Into<String>) -> String {
    serialize_output(&GrepOutput {
        matches: vec![],
        total: 0,
        truncated: false,
        lines_truncated: false,
        error: Some(message.into()),
    })
}

fn serialize_output(output: &GrepOutput) -> String {
    serde_json::to_string(output).expect("grep output should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{remove_workspace, temp_workspace, write_file};
    use serde_json::json;

    #[tokio::test]
    async fn grep_finds_symbol_in_multiple_files_with_context() {
        let workspace_root = temp_workspace("grep_symbol_context").await;
        write_file(
            &workspace_root,
            "src/one.rs",
            "before one\nlet symbol = TargetSymbol;\nafter one\n",
        )
        .await;
        write_file(
            &workspace_root,
            "src/two.rs",
            "before two\nTargetSymbol::call();\nafter two\n",
        )
        .await;
        write_file(&workspace_root, "src/ignored.txt", "TargetSymbol\n").await;

        let tool = GrepTool::new(&workspace_root);
        let result = tool
            .execute(
                json!({
                    "pattern": "TargetSymbol",
                    "path": "src",
                    "glob": "*.rs",
                    "context": 1
                }),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: GrepOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(output.total, 2);
        assert!(!output.truncated);
        assert_match(
            &output,
            "one.rs",
            2,
            "let symbol = TargetSymbol;",
            "before one",
            "after one",
        );
        assert_match(
            &output,
            "two.rs",
            2,
            "TargetSymbol::call();",
            "before two",
            "after two",
        );

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn invalid_glob_returns_clear_error_payload() {
        let workspace_root = temp_workspace("grep_invalid_glob").await;
        write_file(&workspace_root, "src/one.rs", "TargetSymbol\n").await;

        let tool = GrepTool::new(&workspace_root);
        let result = tool
            .execute(
                json!({"pattern": "TargetSymbol", "path": "src", "glob": "["}),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: GrepOutput = serde_json::from_str(&result).unwrap();

        assert_eq!(output.matches, vec![]);
        assert_eq!(output.total, 0);
        assert!(output
            .error
            .unwrap()
            .starts_with("Invalid glob pattern '[': "));

        remove_workspace(workspace_root).await;
    }

    fn assert_match(
        output: &GrepOutput,
        file_suffix: &str,
        line: usize,
        text: &str,
        before: &str,
        after: &str,
    ) {
        let found = output.matches.iter().any(|matched| {
            matched.file.ends_with(file_suffix)
                && matched.line == line
                && matched.text == text
                && matched.context_before == vec![before.to_owned()]
                && matched.context_after == vec![after.to_owned()]
        });

        assert!(found, "missing grep match for {file_suffix}");
    }
}
