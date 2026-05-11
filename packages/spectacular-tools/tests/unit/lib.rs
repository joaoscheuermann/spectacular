use super::*;
use crate::display::{dim_style, paint, text_style, tool_name_style, tool_style};
use serde_json::{json, Value};
use spectacular_agent::Tool;

/// Verifies that the built-in storage factory registers the current built-in tool set in order.
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

/// Verifies that tool-specific formatters do not duplicate the renderer-owned tool prefix.
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
        let redundant_prefix = format!("{} ", paint(tool_name_style(), name));
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

/// Verifies that terminal calls style both the command and working-directory argument pair.
#[test]
fn terminal_format_call_styles_working_directory_as_tool_arg_pair() {
    let tool = TerminalTool::new(PathBuf::from("workspace"));
    let command = r"type .agents\skills\coding-conventions\SKILL.md";
    let working_directory = r"C:\Users\jvito\Documents\git\Personal\spectacular";

    let rendered = tool.format_call(&json!({
        "command": command,
        "working_directory": working_directory
    }));

    assert_eq!(
        strip_ansi_codes(&rendered),
        format!("Run {command} in {working_directory}")
    );
    assert!(rendered.contains(&paint(tool_style(), "Run")));
    assert!(rendered.contains(&paint(text_style(), command)));
    assert!(rendered.contains(&paint(tool_style(), "in")));
    assert!(rendered.contains(&paint(text_style(), working_directory)));
    assert!(!rendered.contains(&paint(dim_style(), format!("in {working_directory}"))));
}

/// Verifies that edit calls render edit counts as dimmed metadata.
#[test]
fn edit_format_call_keeps_edit_count_as_metadata() {
    let tool = EditTool::new(PathBuf::from("workspace"));
    let path = r".agents\skills\coding-conventions\SKILL.md";

    let rendered = tool.format_call(&json!({
        "path": path,
        "edits": [
            {"oldText": "one", "newText": "two"},
            {"oldText": "three", "newText": "four"},
            {"oldText": "five", "newText": "six"}
        ]
    }));

    assert_eq!(
        strip_ansi_codes(&rendered),
        format!("Edited {path} (3 edits)")
    );
    assert!(rendered.contains(&paint(tool_style(), "Edited")));
    assert!(rendered.contains(&paint(text_style(), path)));
    assert!(rendered.contains(&paint(dim_style(), "(3 edits)")));
}

/// Removes ANSI SGR escape sequences from styled display text for stable assertions.
fn strip_ansi_codes(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars().peekable();

    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }

        if chars.peek() == Some(&'[') {
            chars.next();
            for code_character in chars.by_ref() {
                if code_character.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }

    output
}
