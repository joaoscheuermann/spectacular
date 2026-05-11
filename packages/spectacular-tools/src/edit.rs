#[path = "edit/display.rs"]
mod edit_display;

use crate::diff_preview::diff_preview;
use crate::display::{error_style, paint, tool_line};
use crate::path::resolve_workspace_path;
use edit_display::diff_summary;
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
    /// Creates an edit tool scoped to the provided workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

impl Tool for EditTool {
    /// Returns the stable tool name used for registration and dispatch.
    fn name(&self) -> &str {
        EDIT_TOOL_NAME
    }

    /// Builds the edit tool manifest and JSON parameter schema.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            EDIT_TOOL_NAME,
            EDIT_TOOL_DESCRIPTION,
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to edit. Relative paths resolve against the workspace root; absolute paths and .. traversal are allowed intentionally."
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

    /// Formats edit arguments as concise path and edit-count text.
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
        let noun = if edit_count == 1 { "edit" } else { "edits" };
        format!("{path} ({edit_count} {noun})")
    }

    /// Formats edit arguments as a styled renderer call line.
    fn format_call(&self, arguments: &Value) -> ToolDisplay {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("<missing path>");
        let edit_count = arguments
            .get("edits")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();
        let noun = if edit_count == 1 { "edit" } else { "edits" };
        let metadata = format!("({edit_count} {noun})");
        tool_line("Edited", path, Some(&metadata))
    }

    /// Formats edit output as either an error, diff body, or raw fallback text.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let Some(output) = parsed_output else {
            return raw_output.to_string();
        };

        if let Some(error) = output.get("error").and_then(Value::as_str) {
            let status = paint(error_style(), "failed");
            return format!("{status}: {error}");
        }

        output
            .get("diff")
            .and_then(Value::as_str)
            .filter(|diff| !diff.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| output.to_string())
    }

    /// Adds the edited path and summary counts to multiline diff output when input is available.
    fn format_output_with_input(
        &self,
        raw_output: &str,
        parsed_output: Option<&Value>,
        arguments: Option<&Value>,
    ) -> ToolDisplay {
        let Some(path) = arguments
            .and_then(|input| input.get("path"))
            .and_then(Value::as_str)
        else {
            return self.format_output(raw_output, parsed_output);
        };
        let output = self.format_output(raw_output, parsed_output);
        if output.contains('\n') && !output.starts_with("Edited ") {
            let summary = diff_summary(parsed_output);
            return format!("Edited {path}{summary}\n{output}");
        }

        output
    }

    /// Executes exact replacements against the target file and serializes the output payload.
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

/// Applies all requested edits to one file and returns a structured edit result.
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

    let first_changed_line = first_changed_line(&base_content, &new_content);
    let diff = diff_preview(&base_content, &new_content).lines;
    EditOutput {
        success: true,
        diff,
        first_changed_line,
        error: None,
    }
}

/// Normalizes edit text to LF endings and fuzzy trailing-whitespace matching semantics.
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

/// Validates edit entries before matching and returns the first user-facing error.
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

/// Locates unique, non-overlapping edit ranges in the original content.
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

/// Rejects matched edits whose original ranges overlap.
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

/// Applies matched edits from the end of the file toward the start to preserve byte offsets.
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

/// Builds the user-facing error for an edit whose old text was not found.
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

/// Builds the user-facing error for old text that appears more than once.
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

/// Converts CRLF and CR line endings to LF for matching.
fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Detects whether the original content primarily starts with CRLF or LF endings.
fn detect_line_ending(content: &str) -> &'static str {
    let crlf_position = content.find("\r\n");
    let lf_position = content.find('\n');
    match (crlf_position, lf_position) {
        (Some(crlf), Some(lf)) if crlf < lf => "\r\n",
        _ => "\n",
    }
}

/// Restores normalized LF content to the original line-ending style.
fn restore_line_endings(text: &str, ending: &str) -> String {
    if ending == "\r\n" {
        return text.replace('\n', "\r\n");
    }

    text.to_owned()
}

/// Separates a leading UTF-8 BOM marker from editable content.
fn strip_bom(content: &str) -> (&str, &str) {
    if let Some(stripped) = content.strip_prefix('\u{FEFF}') {
        return ("\u{FEFF}", stripped);
    }

    ("", content)
}

/// Trims trailing whitespace from each line to support tolerant matching.
fn normalize_for_fuzzy(text: &str) -> String {
    text.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the first new-file line number touched by a diff.
fn first_changed_line(old: &str, new: &str) -> Option<usize> {
    let diff = TextDiff::from_lines(old, new);
    let mut new_line = 1;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => new_line += 1,
            ChangeTag::Delete | ChangeTag::Insert => return Some(new_line),
        }
    }

    None
}

/// Builds a failed edit output payload with a user-facing error message.
fn error_output(message: impl Into<String>) -> EditOutput {
    EditOutput {
        success: false,
        diff: String::new(),
        first_changed_line: None,
        error: Some(message.into()),
    }
}

/// Serializes a failed edit output payload for invalid tool input.
fn edit_error(message: impl Into<String>) -> String {
    serialize_output(&error_output(message))
}

/// Serializes an edit output payload to JSON.
fn serialize_output(output: &EditOutput) -> String {
    serde_json::to_string(output).expect("edit output should serialize")
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/edit.rs"));
}
