use spectacular_agent::{ToolRegistrationError, ToolStorage};
use std::path::PathBuf;

mod display;
pub mod edit;
pub mod find;
pub mod grep;
pub mod path;
pub mod terminal;
#[cfg(test)]
mod test_support;
pub mod tree;
pub mod web;
pub mod write;

pub use edit::{EditTool, EDIT_TOOL_NAME};
pub use find::{FindTool, FIND_TOOL_NAME};
pub use grep::{GrepTool, GREP_TOOL_NAME};
pub use terminal::{TerminalTool, TERMINAL_TOOL_NAME};
pub use tree::{TreeTool, TREE_TOOL_NAME};
pub use web::{WebSearchTool, WEB_SEARCH_TOOL_NAME};
pub use write::{WriteTool, WRITE_TOOL_NAME};

pub fn built_in_tools(
    workspace_root: impl Into<PathBuf>,
) -> Result<ToolStorage, ToolRegistrationError> {
    let workspace_root = workspace_root.into();
    let mut storage = ToolStorage::default();
    storage.register(EditTool::new(workspace_root.clone()))?;
    storage.register(FindTool::new(workspace_root.clone()))?;
    storage.register(GrepTool::new(workspace_root.clone()))?;
    storage.register(TerminalTool::new(workspace_root.clone()))?;
    storage.register(TreeTool::new(workspace_root.clone()))?;
    storage.register(WebSearchTool)?;
    storage.register(WriteTool::new(workspace_root))?;
    Ok(storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::paint;
    use anstyle::AnsiColor;
    use serde_json::{json, Value};

    #[test]
    fn built_in_factory_registers_all_current_tools() {
        let storage = built_in_tools(PathBuf::from("workspace")).unwrap();

        assert_eq!(
            storage
                .manifests()
                .into_iter()
                .map(|manifest| manifest.name)
                .collect::<Vec<_>>(),
            vec![
                EDIT_TOOL_NAME,
                FIND_TOOL_NAME,
                GREP_TOOL_NAME,
                TERMINAL_TOOL_NAME,
                TREE_TOOL_NAME,
                WEB_SEARCH_TOOL_NAME,
                WRITE_TOOL_NAME
            ]
        );
    }

    #[test]
    fn built_in_tool_formatters_omit_redundant_tool_prefixes() {
        let storage = built_in_tools(PathBuf::from("workspace")).unwrap();
        let cases = [
            (
                EDIT_TOOL_NAME,
                json!({
                    "path": "src/lib.rs",
                    "edits": [{"oldText": "old", "newText": "new"}]
                }),
                json!({"success": true, "diff": "", "first_changed_line": 2}),
            ),
            (
                FIND_TOOL_NAME,
                json!({"pattern": "*.rs", "path": "."}),
                json!({"results": [], "total": 3, "truncated": false}),
            ),
            (
                GREP_TOOL_NAME,
                json!({"pattern": "needle", "path": "."}),
                json!({
                    "matches": [],
                    "total": 15,
                    "truncated": false,
                    "lines_truncated": false
                }),
            ),
            (
                TERMINAL_TOOL_NAME,
                json!({"command": "cargo test", "working_directory": "."}),
                json!({"stdout": "", "stderr": "", "exit_code": 0}),
            ),
            (TREE_TOOL_NAME, json!({"path": "."}), Value::Null),
            (
                WEB_SEARCH_TOOL_NAME,
                json!({"action": "search", "query": "rust"}),
                json!({
                    "action": "search",
                    "detail": "rust",
                    "results": [],
                    "matches": [],
                    "total": 0,
                    "truncated": false
                }),
            ),
            (
                WRITE_TOOL_NAME,
                json!({"path": "notes.txt", "content": "hello"}),
                json!({"success": true, "bytes_written": 5}),
            ),
        ];

        for (name, input, output) in cases {
            let tool = storage.get(name).unwrap();
            let redundant_prefix = format!(
                "{} ",
                paint(AnsiColor::BrightCyan.on_default().bold(), name)
            );
            let raw_output = match name {
                TREE_TOOL_NAME => "workspace\n`-- file.txt\n".to_owned(),
                _ => output.to_string(),
            };

            let input_display = tool.format_input(&input);
            let parsed_output = (name != TREE_TOOL_NAME).then_some(&output);
            let output_display = tool.format_output(&raw_output, parsed_output);
            let raw_output_display = tool.format_output("raw output", None);

            assert!(
                !input_display.starts_with(&redundant_prefix),
                "{name} format_input should omit the renderer-owned tool prefix"
            );
            assert!(
                !output_display.starts_with(&redundant_prefix),
                "{name} format_output should omit the renderer-owned tool prefix"
            );
            assert!(
                !raw_output_display.starts_with(&redundant_prefix),
                "{name} raw format_output should omit the renderer-owned tool prefix"
            );
        }
    }
}
