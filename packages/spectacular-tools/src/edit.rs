use crate::display::paint;
use crate::path::resolve_workspace_path;
use anstyle::AnsiColor;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use std::fs;
use std::path::{Path, PathBuf};

pub const EDIT_TOOL_NAME: &str = "edit";

const EDIT_TOOL_DESCRIPTION: &str = "Edit a file using exact text replacement. Each edit's oldText must match a unique, non-overlapping region of the original file. If two changes affect the same block or nearby lines, merge them into one edit.";

#[derive(Clone, Debug)]
pub struct EditTool {
    workspace_root: PathBuf,
}

impl EditTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for EditTool {
    fn name(&self) -> &str {
        EDIT_TOOL_NAME
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            EDIT_TOOL_NAME,
            EDIT_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit (relative or absolute)"
                    },
                    "edits": {
                        "type": "array",
                        "description": "One or more targeted replacements. Each edit is matched against the original file, not incrementally.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "oldText": {
                                    "type": "string",
                                    "description": "Exact text to find. Must be unique in the original file."
                                },
                                "newText": {
                                    "type": "string",
                                    "description": "Replacement text."
                                },
                                "old_text": {
                                    "type": "string",
                                    "description": "Alias for oldText."
                                },
                                "new_text": {
                                    "type": "string",
                                    "description": "Alias for newText."
                                }
                            },
                            "anyOf": [
                                { "required": ["oldText", "newText"] },
                                { "required": ["oldText", "new_text"] },
                                { "required": ["old_text", "newText"] },
                                { "required": ["old_text", "new_text"] }
                            ],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["path", "edits"],
                "additionalProperties": false
            }),
        )
    }

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("<missing path>");
        let edit_count = arguments
            .get("edits")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();
        format!("{path} ({edit_count} edit(s))")
    }

    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        if let Some(error) = output.get("error").and_then(Value::as_str) {
            let status = paint(AnsiColor::BrightRed.on_default().bold(), "failed");
            return format!("{status}: {error}");
        }

        let first_changed_line = output
            .get("first_changed_line")
            .and_then(Value::as_u64)
            .map(|line| format!(" at line {line}"))
            .unwrap_or_default();
        let status = paint(AnsiColor::BrightGreen.on_default().bold(), "edited");
        format!("{status}{first_changed_line}")
    }

    fn execute<'a>(&'a self, arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        let workspace_root = self.workspace_root.clone();

        Box::pin(async move {
            let input = match serde_json::from_value::<EditInput>(arguments) {
                Ok(input) => input,
                Err(error) => {
                    return Ok(edit_error(format!("Invalid input JSON: {error}")));
                }
            };

            Ok(serialize_output(&execute_edit(&workspace_root, input)))
        })
    }
}

#[derive(Debug, Deserialize)]
struct EditEntry {
    #[serde(alias = "oldText")]
    old_text: String,
    #[serde(alias = "newText")]
    new_text: String,
}

#[derive(Debug, Deserialize)]
struct EditInput {
    path: String,
    edits: Vec<EditEntry>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EditOutput {
    pub success: bool,
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_changed_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct NormalizedEdit {
    old_text: String,
    new_text: String,
}

struct MatchedEdit {
    edit_index: usize,
    match_index: usize,
    match_length: usize,
    new_text: String,
}

fn execute_edit(workspace_root: &Path, input: EditInput) -> EditOutput {
    let file_path = resolve_workspace_path(workspace_root, &input.path);

    if !file_path.exists() {
        return error_output(format!("File not found: {}", input.path));
    }

    if input.edits.is_empty() {
        return error_output("edits must contain at least one replacement.");
    }

    let raw_content = match fs::read_to_string(&file_path) {
        Ok(content) => content,
        Err(error) => return error_output(format!("Failed to read file: {error}")),
    };

    let (bom, content) = strip_bom(&raw_content);
    let original_ending = detect_line_ending(content);
    let normalized = normalize_to_lf(content);
    let edits = normalize_edits(&input.edits);

    if let Some(error) = validate_edits(&input.path, &edits) {
        return error_output(error);
    }

    let any_fuzzy = edits
        .iter()
        .any(|edit| !normalized.contains(edit.old_text.as_str()));
    let base_content = if any_fuzzy {
        normalize_for_fuzzy(&normalized)
    } else {
        normalized.clone()
    };

    let matched_edits = match locate_edits(&input.path, &edits, &base_content) {
        Ok(matched_edits) => matched_edits,
        Err(error) => return error_output(error),
    };

    let new_content = apply_edits(&base_content, &matched_edits);
    if base_content == new_content {
        return error_output(format!(
            "No changes made to {}. The replacement produced identical content.",
            input.path
        ));
    }

    let final_content = format!(
        "{}{}",
        bom,
        restore_line_endings(&new_content, original_ending)
    );

    if let Err(error) = fs::write(&file_path, final_content) {
        return error_output(format!("Failed to write file: {error}"));
    }

    let (diff, first_changed_line) = generate_diff(&base_content, &new_content);
    EditOutput {
        success: true,
        diff,
        first_changed_line,
        error: None,
    }
}

fn normalize_edits(edits: &[EditEntry]) -> Vec<NormalizedEdit> {
    edits
        .iter()
        .map(|edit| {
            let old_lf = normalize_to_lf(&edit.old_text);
            let new_lf = normalize_to_lf(&edit.new_text);
            NormalizedEdit {
                old_text: normalize_for_fuzzy(&old_lf),
                new_text: normalize_for_fuzzy(&new_lf),
            }
        })
        .collect()
}

fn validate_edits(path: &str, edits: &[NormalizedEdit]) -> Option<String> {
    for (index, edit) in edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Some(if edits.len() == 1 {
                format!("oldText must not be empty in {path}.")
            } else {
                format!("edits[{index}].oldText must not be empty in {path}.")
            });
        }

        if edit.old_text == edit.new_text {
            return Some(if edits.len() == 1 {
                format!("No changes made to {path}. The replacement produced identical content.")
            } else {
                format!(
                    "No changes made by edits[{index}] in {path}. The replacement produced identical content."
                )
            });
        }
    }

    None
}

fn locate_edits(
    path: &str,
    edits: &[NormalizedEdit],
    base_content: &str,
) -> Result<Vec<MatchedEdit>, String> {
    let mut matched_edits = Vec::new();

    for (index, edit) in edits.iter().enumerate() {
        let match_index = match base_content.find(edit.old_text.as_str()) {
            Some(match_index) => match_index,
            None => return Err(missing_match_error(path, index, edits.len())),
        };

        let occurrences = base_content.matches(edit.old_text.as_str()).count();
        if occurrences > 1 {
            return Err(duplicate_match_error(path, index, edits.len(), occurrences));
        }

        matched_edits.push(MatchedEdit {
            edit_index: index,
            match_index,
            match_length: edit.old_text.len(),
            new_text: edit.new_text.clone(),
        });
    }

    matched_edits.sort_by_key(|edit| edit.match_index);
    reject_overlaps(path, &matched_edits)?;
    Ok(matched_edits)
}

fn reject_overlaps(path: &str, matched_edits: &[MatchedEdit]) -> Result<(), String> {
    for index in 1..matched_edits.len() {
        let previous = &matched_edits[index - 1];
        let current = &matched_edits[index];
        if previous.match_index + previous.match_length > current.match_index {
            return Err(format!(
                "edits[{}] and edits[{}] overlap in {}. Merge them into one edit.",
                previous.edit_index, current.edit_index, path
            ));
        }
    }

    Ok(())
}

fn apply_edits(base_content: &str, matched_edits: &[MatchedEdit]) -> String {
    let mut new_content = base_content.to_owned();
    for edit in matched_edits.iter().rev() {
        let end = edit.match_index + edit.match_length;
        new_content = format!(
            "{}{}{}",
            &new_content[..edit.match_index],
            edit.new_text,
            &new_content[end..]
        );
    }
    new_content
}

fn missing_match_error(path: &str, edit_index: usize, edit_count: usize) -> String {
    if edit_count == 1 {
        return format!(
            "Could not find the exact text in {path}. The old text must match exactly including all whitespace and newlines."
        );
    }

    format!(
        "Could not find edits[{edit_index}] in {path}. The oldText must match exactly including all whitespace and newlines."
    )
}

fn duplicate_match_error(
    path: &str,
    edit_index: usize,
    edit_count: usize,
    occurrences: usize,
) -> String {
    if edit_count == 1 {
        return format!(
            "Found {occurrences} occurrences of the text in {path}. The text must be unique. Please provide more context."
        );
    }

    format!(
        "Found {occurrences} occurrences of edits[{edit_index}] in {path}. Each oldText must be unique. Please provide more context."
    )
}

fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn detect_line_ending(content: &str) -> &'static str {
    let crlf_position = content.find("\r\n");
    let lf_position = content.find('\n');
    match (crlf_position, lf_position) {
        (Some(crlf), Some(lf)) if crlf < lf => "\r\n",
        _ => "\n",
    }
}

fn restore_line_endings(text: &str, ending: &str) -> String {
    if ending == "\r\n" {
        return text.replace('\n', "\r\n");
    }

    text.to_owned()
}

fn strip_bom(content: &str) -> (&str, &str) {
    if let Some(stripped) = content.strip_prefix('\u{FEFF}') {
        return ("\u{FEFF}", stripped);
    }

    ("", content)
}

fn normalize_for_fuzzy(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn generate_diff(old: &str, new: &str) -> (String, Option<usize>) {
    let diff = TextDiff::from_lines(old, new);
    let mut output = Vec::new();
    let mut first_changed_line = None;
    let max_line = old.lines().count().max(new.lines().count());
    let width = format!("{max_line}").len();

    let mut old_line = 1;
    let mut new_line = 1;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                let text = change.value().trim_end_matches('\n');
                output.push(format!(" {old_line:>width$} {text}"));
                old_line += 1;
                new_line += 1;
            }
            ChangeTag::Delete => {
                if first_changed_line.is_none() {
                    first_changed_line = Some(new_line);
                }
                let text = change.value().trim_end_matches('\n');
                output.push(format!("-{old_line:>width$} {text}"));
                old_line += 1;
            }
            ChangeTag::Insert => {
                if first_changed_line.is_none() {
                    first_changed_line = Some(new_line);
                }
                let text = change.value().trim_end_matches('\n');
                output.push(format!("+{new_line:>width$} {text}"));
                new_line += 1;
            }
        }
    }

    (output.join("\n"), first_changed_line)
}

fn error_output(message: impl Into<String>) -> EditOutput {
    EditOutput {
        success: false,
        diff: String::new(),
        first_changed_line: None,
        error: Some(message.into()),
    }
}

fn edit_error(message: impl Into<String>) -> String {
    serialize_output(&error_output(message))
}

fn serialize_output(output: &EditOutput) -> String {
    serde_json::to_string(output).expect("edit output should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{remove_workspace, temp_workspace, write_file};
    use serde_json::json;

    #[tokio::test]
    async fn real_edit_updates_file_and_returns_diff_with_first_changed_line() {
        let workspace_root = temp_workspace("edit_real_file").await;
        write_file(
            &workspace_root,
            "src/lib.rs",
            "fn main() {\n    old_call();\n}\n",
        )
        .await;
        let tool = EditTool::new(&workspace_root);

        let result = tool
            .execute(
                json!({
                    "path": "src/lib.rs",
                    "edits": [{
                        "oldText": "    old_call();",
                        "newText": "    new_call();"
                    }]
                }),
                Cancellation::default(),
            )
            .await
            .unwrap();
        let output: EditOutput = serde_json::from_str(&result).unwrap();

        assert!(output.success);
        assert_eq!(output.first_changed_line, Some(2));
        assert!(output.diff.contains("-2     old_call();"));
        assert!(output.diff.contains("+2     new_call();"));
        assert_eq!(
            tokio::fs::read_to_string(workspace_root.join("src/lib.rs"))
                .await
                .unwrap(),
            "fn main() {\n    new_call();\n}\n"
        );

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn crlf_input_preserves_crlf_after_edit() {
        let workspace_root = temp_workspace("edit_preserves_crlf").await;
        write_file(&workspace_root, "src/lib.rs", "one\r\ntwo\r\nthree\r\n").await;
        let tool = EditTool::new(&workspace_root);

        let output = execute_edit_json(
            &tool,
            json!({
                "path": "src/lib.rs",
                "edits": [{
                    "oldText": "two",
                    "newText": "TWO"
                }]
            }),
        )
        .await;
        let bytes = tokio::fs::read(workspace_root.join("src/lib.rs"))
            .await
            .unwrap();
        let content = String::from_utf8(bytes).unwrap();

        assert!(output.success);
        assert_eq!(content, "one\r\nTWO\r\nthree\r\n");
        assert!(!content.contains("one\nTWO"));

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn utf8_bom_is_preserved() {
        let workspace_root = temp_workspace("edit_preserves_bom").await;
        let path = workspace_root.join("bom.txt");
        tokio::fs::write(&path, b"\xEF\xBB\xBFalpha\nbeta\n")
            .await
            .unwrap();
        let tool = EditTool::new(&workspace_root);

        let output = execute_edit_json(
            &tool,
            json!({
                "path": "bom.txt",
                "edits": [{
                    "oldText": "beta",
                    "newText": "gamma"
                }]
            }),
        )
        .await;
        let bytes = tokio::fs::read(path).await.unwrap();

        assert!(output.success);
        assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
        assert_eq!(String::from_utf8(bytes).unwrap(), "\u{FEFF}alpha\ngamma\n");

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn duplicate_old_text_returns_more_context_error() {
        let workspace_root = temp_workspace("edit_duplicate_old_text").await;
        write_file(&workspace_root, "src/lib.rs", "same\nsame\n").await;
        let tool = EditTool::new(&workspace_root);

        let output = execute_edit_json(
            &tool,
            json!({
                "path": "src/lib.rs",
                "edits": [{
                    "oldText": "same",
                    "newText": "changed"
                }]
            }),
        )
        .await;

        assert!(!output.success);
        assert!(output
            .error
            .unwrap()
            .contains("Please provide more context."));

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn overlapping_edits_return_error() {
        let workspace_root = temp_workspace("edit_overlapping").await;
        write_file(&workspace_root, "src/lib.rs", "abcdef\n").await;
        let tool = EditTool::new(&workspace_root);

        let output = execute_edit_json(
            &tool,
            json!({
                "path": "src/lib.rs",
                "edits": [
                    {
                        "oldText": "abc",
                        "newText": "ABC"
                    },
                    {
                        "oldText": "bcd",
                        "newText": "BCD"
                    }
                ]
            }),
        )
        .await;

        assert!(!output.success);
        assert!(output.error.unwrap().contains("overlap"));

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn old_text_and_new_text_aliases_are_accepted() {
        let workspace_root = temp_workspace("edit_aliases").await;
        write_file(&workspace_root, "src/lib.rs", "alpha\n").await;
        let tool = EditTool::new(&workspace_root);

        let output = execute_edit_json(
            &tool,
            json!({
                "path": "src/lib.rs",
                "edits": [{
                    "old_text": "alpha",
                    "new_text": "beta"
                }]
            }),
        )
        .await;

        assert!(output.success);
        assert_eq!(
            tokio::fs::read_to_string(workspace_root.join("src/lib.rs"))
                .await
                .unwrap(),
            "beta\n"
        );

        remove_workspace(workspace_root).await;
    }

    #[tokio::test]
    async fn trailing_whitespace_tolerance_matches_old_text() {
        let workspace_root = temp_workspace("edit_trailing_whitespace").await;
        write_file(&workspace_root, "src/lib.rs", "alpha   \nbeta\n").await;
        let tool = EditTool::new(&workspace_root);

        let output = execute_edit_json(
            &tool,
            json!({
                "path": "src/lib.rs",
                "edits": [{
                    "oldText": "alpha\nbeta",
                    "newText": "gamma\nbeta"
                }]
            }),
        )
        .await;

        assert!(output.success);
        assert_eq!(
            tokio::fs::read_to_string(workspace_root.join("src/lib.rs"))
                .await
                .unwrap(),
            "gamma\nbeta"
        );

        remove_workspace(workspace_root).await;
    }

    async fn execute_edit_json(tool: &EditTool, arguments: Value) -> EditOutput {
        let result = tool
            .execute(arguments, Cancellation::default())
            .await
            .unwrap();
        serde_json::from_str(&result).unwrap()
    }
}
