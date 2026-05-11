use super::directory::format_directory_with_home;
use super::footer::{format_user_prompt_footer, UserPromptFooterView};
use super::tool::ToolStatus;
use super::*;
use crate::chat::model::ChatPromptFooterModel;
use crate::terminal_style;
use serde_json::{json, Value};
use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
use spectacular_config::ReasoningLevel;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
struct DisplayTool;

impl Tool for DisplayTool {
    fn name(&self) -> &str {
        "display_tool"
    }

    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            self.name(),
            "Formats chat renderer payloads.",
            json!({"type": "object", "additionalProperties": true}),
        )
    }

    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        format!("registered input: {}", arguments["path"].as_str().unwrap())
    }

    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let state = parsed_output
            .and_then(|value| value.get("success"))
            .and_then(Value::as_bool)
            .map(|success| format!("success={success}"))
            .unwrap_or_else(|| "parsed=none".to_owned());
        format!("registered output: {state}; raw={raw_output}")
    }

    fn execute<'a>(&'a self, _arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async { Ok(r#"{"success":true}"#.to_owned()) })
    }
}

#[test]
fn line_output_keeps_working_indicator_active() {
    let renderer = Renderer::default();

    renderer.working_frame(1);
    renderer.dim("status update");

    assert_eq!(renderer.renderable_working_frame(), Some(1));
}

#[test]
fn stream_output_pauses_working_indicator() {
    let renderer = Renderer::default();

    renderer.working_frame(2);
    assert!(renderer.pause_working());

    assert_eq!(renderer.renderable_working_frame(), None);

    renderer.working_frame(3);
    assert_eq!(renderer.renderable_working_frame(), None);

    renderer.resume_working();
    assert_eq!(renderer.renderable_working_frame(), Some(3));

    renderer.clear_working();
    assert_eq!(renderer.renderable_working_frame(), None);
}

#[test]
fn terminal_glyphs_are_not_mojibake() {
    assert_eq!(TOOL_RESULT_PREFIX, "└");
    assert_eq!(
        WORKING_FRAMES,
        &["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    );
    assert!(!TOOL_RESULT_PREFIX.contains('\u{00e2}'));
    assert!(WORKING_FRAMES
        .iter()
        .all(|frame| !frame.contains('\u{00e2}')));
}

#[test]
fn registered_tool_display_is_used_for_tool_call_and_result() {
    let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

    let call = ToolCallView::from_parts("display_tool", r#"{"path":"foo.txt"}"#, &tools);
    let result = ToolResultView::from_parts("display_tool", r#"{"success":true}"#, &tools);

    assert_eq!(call.name, "display_tool");
    assert_eq!(call.input, "registered input: foo.txt");
    assert_eq!(
        result.output,
        r#"registered output: success=true; raw={"success":true}"#
    );
    assert_eq!(result.status, ToolStatus::Done);
}

#[test]
fn malformed_registered_tool_arguments_use_generic_fallback() {
    let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

    let call = ToolCallView::from_parts("display_tool", "{", &tools);

    assert_eq!(call.input, "{");
}

#[test]
fn registered_tool_receives_none_for_non_json_output() {
    let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

    let result = ToolResultView::from_parts("display_tool", "not json", &tools);

    assert_eq!(
        result.output,
        "registered output: parsed=none; raw=not json"
    );
    assert_eq!(result.status, ToolStatus::Done);
}

#[test]
fn assistant_visibility_requires_nonblank_trimmed_text() {
    assert!(!has_visible_assistant_text(""));
    assert!(!has_visible_assistant_text(" \n\t"));
    assert!(has_visible_assistant_text(" answer "));
}

#[test]
fn reasoning_text_formats_dim_content_without_header() {
    let output =
        reasoning::format_reasoning_text("thinking").expect("visible reasoning should render");

    assert!(output.contains("thinking"));
    assert!(!output.contains("reasoning"));
    assert!(output.contains(&terminal_style::dim_style().to_string()));
}

#[test]
fn blank_reasoning_text_is_hidden() {
    assert!(reasoning::format_reasoning_text(" \n\t").is_none());
}

#[test]
fn result_status_marks_common_failures() {
    let tools = ToolStorage::default();

    assert_eq!(
        ToolResultView::from_parts("missing", r#"{"exit_code":1}"#, &tools).status,
        ToolStatus::Failed
    );
    assert_eq!(
        ToolResultView::from_parts("missing", r#"{"error_kind":"timeout"}"#, &tools).status,
        ToolStatus::Failed
    );
    assert_eq!(
        ToolResultView::from_parts("missing", "Error: failed", &tools).status,
        ToolStatus::Failed
    );
    assert_eq!(
        ToolResultView::from_parts("missing", r#"{"exit_code":0}"#, &tools).status,
        ToolStatus::Done
    );
}

#[test]
fn missing_tool_uses_generic_preview_for_session_replay() {
    let tools = ToolStorage::default();

    let call = ToolCallView::from_parts("missing_tool", r#"{"path":"foo.txt"}"#, &tools);
    let result = ToolResultView::from_parts("missing_tool", r#"{"success":true}"#, &tools);

    assert_eq!(call.name, "missing_tool");
    assert_eq!(call.input, "path: foo.txt");
    assert_eq!(result.output, "success: true");
    assert_eq!(result.status, ToolStatus::Done);
}

#[test]
fn opening_banner_renders_codex_style_session_summary() {
    let view = OpeningBannerView {
        version: "0.1.0".to_owned(),
        model: "gpt-5.5".to_owned(),
        reasoning: "high".to_owned(),
        directory: format!(
            "~{}Documents{}git{}Personal{}spectacular",
            MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR
        ),
        session_id: "a83f19c2".to_owned(),
    };

    let banner = format_opening_banner(&view);

    assert!(banner.contains("Spectacular (v0.1.0)"));
    assert!(banner.contains(&format!(
        "{}Spectacular (v0.1.0)",
        terminal_style::title_style()
    )));
    assert!(banner.contains("model:     gpt-5.5 high"));
    assert!(banner.contains(&format!(
        "directory: ~{}Documents{}git{}Personal{}spectacular",
        MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR
    )));
    assert!(banner.contains("session:   a83f19c2"));

    let widths = banner
        .lines()
        .map(strip_ansi_codes)
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .collect::<Vec<_>>();
    assert!(widths.windows(2).all(|pair| pair[0] == pair[1]));
}

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

#[test]
fn opening_banner_view_uses_runtime_selection() {
    let runtime = RuntimeSelection {
        provider_type: "openrouter".to_owned(),
        provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
        provider: "openrouter".to_owned(),
        api_key: "sk-or-v1-test".to_owned(),
        model_key: "openai/gpt-5.5".to_owned(),
        model: "openai/gpt-5.5".to_owned(),
        reasoning: ReasoningLevel::High,
    };

    let view = OpeningBannerView::from_runtime("b7d4201f", &runtime, Path::new("workspace"));

    assert_eq!(view.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(view.model, "openai/gpt-5.5");
    assert_eq!(view.reasoning, "high");
    assert_eq!(view.session_id, "b7d4201f");
}

#[test]
fn directory_label_uses_home_shorthand() {
    let home = PathBuf::from("home");
    let directory = home.join("repo").join("spectacular");

    assert_eq!(
        format_directory_with_home(&directory, Some(&home)),
        format!("~{}repo{}spectacular", MAIN_SEPARATOR, MAIN_SEPARATOR)
    );
    assert_eq!(format_directory_with_home(&home, Some(&home)), "~");
}

#[test]
fn directory_label_leaves_paths_outside_home_unchanged() {
    let home = PathBuf::from("home");
    let directory = PathBuf::from("workspace");

    assert_eq!(
        format_directory_with_home(&directory, Some(&home)),
        directory.display().to_string()
    );
}

#[test]
fn user_prompt_footer_formats_context_in_expected_order() {
    let footer = ChatPromptFooterModel {
        directory: PathBuf::from("workspace"),
        model: "openai/gpt-5.5".to_owned(),
        reasoning: ReasoningLevel::High,
    };
    let view = UserPromptFooterView::from_model(&footer);

    assert_eq!(
        view.directory,
        PathBuf::from("workspace").display().to_string()
    );
    assert_eq!(view.model, "openai/gpt-5.5");
    assert_eq!(view.reasoning, "high");
    assert_eq!(
        format_user_prompt_footer(&view),
        format!(
            "{} · openai/gpt-5.5 (high)",
            PathBuf::from("workspace").display()
        )
    );
}
