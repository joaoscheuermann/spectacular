use super::directory::format_directory_with_home;
use super::footer::{format_user_prompt_footer, UserPromptFooterView};
use super::tool::ToolStatus;
use super::*;
use crate::chat::model::ChatPromptFooterModel;
use crate::terminal_style;
use serde_json::{json, Value};
use spectacular_agent::{
    Cancellation, ContextTokenUsage, Tool, ToolDisplay, ToolExecution, ToolManifest,
};
use spectacular_config::ReasoningLevel;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
struct DisplayTool;

impl Tool for DisplayTool {
    /// Returns the stable tool name used in manifests and calls.
    fn name(&self) -> &str {
        "display_tool"
    }

    /// Builds the tool manifest exposed to provider requests.
    fn manifest(&self) -> ToolManifest {
        ToolManifest::new(
            self.name(),
            "Formats chat renderer payloads.",
            json!({"type": "object", "additionalProperties": true}),
        )
    }

    /// Formats tool input arguments for terminal display.
    fn format_input(&self, arguments: &Value) -> ToolDisplay {
        format!("registered input: {}", arguments["path"].as_str().unwrap())
    }

    /// Formats tool output for terminal display.
    fn format_output(&self, raw_output: &str, parsed_output: Option<&Value>) -> ToolDisplay {
        let state = parsed_output
            .and_then(|value| value.get("success"))
            .and_then(Value::as_bool)
            .map(|success| format!("success={success}"))
            .unwrap_or_else(|| "parsed=none".to_owned());
        format!("registered output: {state}; raw={raw_output}")
    }

    /// Executes the tool with the provided arguments and cancellation handle.
    fn execute<'a>(&'a self, _arguments: Value, _cancellation: Cancellation) -> ToolExecution<'a> {
        Box::pin(async { Ok(r#"{"success":true}"#.to_owned()) })
    }
}

/// Verifies that line output keeps working indicator active.
#[test]
fn line_output_keeps_working_indicator_active() {
    let renderer = Renderer::default();

    renderer.working_frame(1, Some(42));
    renderer.dim("status update");

    assert_eq!(renderer.renderable_working_frame(), Some((1, Some(42))));
}

/// Verifies working text includes current assistant-turn tokens when present.
#[test]
fn working_line_includes_turn_tokens_when_present() {
    assert_eq!(
        working_line::format_working_line("⠋", Some(100)),
        "⠋ Working (CTRL + C to stop · 100 tokens)"
    );
}

/// Verifies working text omits token segment when no count is available.
#[test]
fn working_line_omits_turn_tokens_when_absent() {
    assert_eq!(
        working_line::format_working_line("⠋", None),
        "⠋ Working (CTRL + C to stop)"
    );
}

/// Verifies worked text includes elapsed time and total turn tokens.
#[test]
fn worked_line_includes_elapsed_time_and_total_tokens() {
    assert_eq!(
        working_line::format_worked_line(Duration::from_secs(62), Some(100)),
        "Worked for 1m 2s · total 100 tokens"
    );
}

/// Verifies that paused stream output stores working frames without rendering them.
#[test]
fn paused_stream_output_stores_working_frame() {
    let renderer = Renderer::default();

    renderer.working_frame(2, None);
    renderer.pause_working_line();

    assert_eq!(renderer.renderable_working_frame(), None);

    renderer.working_frame(3, Some(7));
    assert_eq!(renderer.renderable_working_frame(), None);

    renderer.resume_working_line();
    assert_eq!(renderer.renderable_working_frame(), Some((3, Some(7))));

    renderer.clear_working();
    assert_eq!(renderer.renderable_working_frame(), None);
}

/// Verifies that resume without a matching pause leaves visible working state unchanged.
#[test]
fn unpaired_resume_preserves_visible_working_frame() {
    let renderer = Renderer::default();

    renderer.working_frame(4, Some(11));
    renderer.resume_working_line();

    assert_eq!(renderer.renderable_working_frame(), Some((4, Some(11))));
}

/// Verifies that response spacing does not deactivate a paused working indicator.
#[test]
fn response_spacer_preserves_paused_working_indicator() {
    let renderer = Renderer::default();

    renderer.working_frame(2, Some(9));
    renderer.pause_working_line();

    renderer.response_spacer();

    assert_eq!(renderer.renderable_working_frame(), None);
    renderer.resume_working_line();
    assert_eq!(renderer.renderable_working_frame(), Some((2, Some(9))));
}

/// Verifies that terminal glyphs are not mojibake.
#[test]
fn terminal_glyphs_are_not_mojibake() {
    assert_eq!(
        WORKING_FRAMES,
        &["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    );
    assert!(WORKING_FRAMES
        .iter()
        .all(|frame| !frame.contains('\u{00e2}')));
}

/// Verifies that command output style uses the existing dim gray command color.
#[test]
fn command_output_style_uses_dim_gray_command_color() {
    assert!(paint(terminal_style::command_output_style(), "output")
        .contains("\x1b[38;2;107;114;128m"));
}

/// Verifies that command invocation style is bold blue.
#[test]
fn command_style_is_bold_blue() {
    assert!(paint(terminal_style::command_style(), "/git commit")
        .contains("\x1b[1m\x1b[38;2;96;165;250m"));
}

/// Verifies that tool call view uses tool owned formatted line.
#[test]
fn tool_call_view_uses_tool_owned_formatted_line() {
    let view = ToolCallView {
        line: tool::format_tool_call_parts(
            "Edited",
            "packages/spectacular/tests/unit/chat/renderer.rs",
            Some("(1 edit)"),
        ),
    };

    let rendered = format_tool_call_view(&view);

    assert!(rendered.contains("Edited"));
    assert!(rendered.contains("packages/spectacular/tests/unit/chat/renderer.rs"));
    assert!(rendered.contains("(1 edit)"));
}

/// Verifies that registered tool display is used for tool call and result.
#[test]
fn registered_tool_display_is_used_for_tool_call_and_result() {
    let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

    let call = ToolCallView::from_parts("display_tool", r#"{"path":"foo.txt"}"#, &tools);
    let result = ToolResultView::from_parts_with_arguments(
        "display_tool",
        r#"{"success":true}"#,
        &tools,
        None,
    );

    assert!(call.line.contains("display_tool"));
    assert!(call.line.contains("registered input: foo.txt"));
    assert_eq!(
        result.output,
        r#"registered output: success=true; raw={"success":true}"#
    );
    assert_eq!(result.status, ToolStatus::Done);
}

/// Verifies that malformed registered tool arguments use generic fallback.
#[test]
fn malformed_registered_tool_arguments_use_generic_fallback() {
    let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

    let call = ToolCallView::from_parts("display_tool", "{", &tools);

    assert!(call.line.contains("display_tool"));
    assert!(call.line.contains("{"));
}

/// Verifies that registered tool receives none for non JSON output.
#[test]
fn registered_tool_receives_none_for_non_json_output() {
    let tools = ToolStorage::try_with_tool(DisplayTool).unwrap();

    let result =
        ToolResultView::from_parts_with_arguments("display_tool", "not json", &tools, None);

    assert_eq!(
        result.output,
        "registered output: parsed=none; raw=not json"
    );
    assert_eq!(result.status, ToolStatus::Done);
}

/// Verifies that assistant visibility requires nonblank trimmed text.
#[test]
fn assistant_visibility_requires_nonblank_trimmed_text() {
    assert!(!has_visible_assistant_text(""));
    assert!(!has_visible_assistant_text(" \n\t"));
    assert!(has_visible_assistant_text(" answer "));
}

/// Verifies that reasoning text formats dim content without header.
#[test]
fn reasoning_text_formats_dim_content_without_header() {
    let output =
        reasoning::format_reasoning_text("thinking").expect("visible reasoning should render");

    assert!(output.contains("thinking"));
    assert!(!output.contains("reasoning"));
    assert!(output.contains(&terminal_style::dim_style().to_string()));
}

/// Verifies that blank reasoning text is hidden.
#[test]
fn blank_reasoning_text_is_hidden() {
    assert!(reasoning::format_reasoning_text(" \n\t").is_none());
}

/// Verifies that result status marks common failures.
#[test]
fn result_status_marks_common_failures() {
    let tools = ToolStorage::default();

    assert_eq!(
        ToolResultView::from_parts_with_arguments("missing", r#"{"exit_code":1}"#, &tools, None)
            .status,
        ToolStatus::Failed
    );
    assert_eq!(
        ToolResultView::from_parts_with_arguments(
            "missing",
            r#"{"error_kind":"timeout"}"#,
            &tools,
            None
        )
        .status,
        ToolStatus::Failed
    );
    assert_eq!(
        ToolResultView::from_parts_with_arguments("missing", "Error: failed", &tools, None).status,
        ToolStatus::Failed
    );
    assert_eq!(
        ToolResultView::from_parts_with_arguments("missing", r#"{"exit_code":0}"#, &tools, None)
            .status,
        ToolStatus::Done
    );
}

/// Verifies that missing tool uses generic preview for session replay.
#[test]
fn missing_tool_uses_generic_preview_for_session_replay() {
    let tools = ToolStorage::default();

    let call = ToolCallView::from_parts("missing_tool", r#"{"path":"foo.txt"}"#, &tools);
    let result = ToolResultView::from_parts_with_arguments(
        "missing_tool",
        r#"{"success":true}"#,
        &tools,
        None,
    );

    assert!(call.line.contains("missing_tool"));
    assert!(call.line.contains("path: foo.txt"));
    assert_eq!(result.output, "success: true");
    assert_eq!(result.status, ToolStatus::Done);
}

/// Verifies that opening banner renders codex style session summary.
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

/// Removes ANSI escape sequences from rendered terminal text for assertions.
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

/// Verifies that opening banner view uses runtime selection.
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
        context_window_tokens: None,
    };

    let view = OpeningBannerView::from_runtime("b7d4201f", &runtime, Path::new("workspace"));

    assert_eq!(view.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(view.model, "openai/gpt-5.5");
    assert_eq!(view.reasoning, "high");
    assert_eq!(view.session_id, "b7d4201f");
}

/// Verifies that directory label uses home shorthand.
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

/// Verifies that directory label leaves paths outside home unchanged.
#[test]
fn directory_label_leaves_paths_outside_home_unchanged() {
    let home = PathBuf::from("home");
    let directory = PathBuf::from("workspace");

    assert_eq!(
        format_directory_with_home(&directory, Some(&home)),
        directory.display().to_string()
    );
}

/// Verifies that user prompt footer formats context in expected order.
#[test]
fn user_prompt_footer_formats_context_in_expected_order() {
    let footer = ChatPromptFooterModel {
        directory: PathBuf::from("workspace"),
        model: "openai/gpt-5.5".to_owned(),
        reasoning: ReasoningLevel::High,
        token_usage: None,
    };
    let view = UserPromptFooterView::from_model(&footer);

    assert_eq!(
        view.directory,
        PathBuf::from("workspace").display().to_string()
    );
    assert_eq!(view.model, "openai/gpt-5.5");
    assert_eq!(view.reasoning, "high");
    assert_eq!(view.token_usage, None);
    let formatted = format_user_prompt_footer(&view);
    assert!(formatted.contains(&format!(
        "{} · openai/gpt-5.5 (high)",
        PathBuf::from("workspace").display()
    )));
}

/// Verifies that user prompt footer appends compact context token usage.
#[test]
fn user_prompt_footer_formats_token_usage() {
    let footer = ChatPromptFooterModel {
        directory: PathBuf::from("workspace"),
        model: "gpt-5.5".to_owned(),
        reasoning: ReasoningLevel::High,
        token_usage: Some(ContextTokenUsage {
            input_tokens: 100,
            context_window_tokens: Some(240_000),
        }),
    };
    let formatted = format_user_prompt_footer(&UserPromptFooterView::from_model(&footer));

    assert!(formatted.contains("gpt-5.5 (high)"));
    assert!(formatted.contains("100/240k tks"));
}

/// Verifies compact token formatting for footer context usage.
#[test]
fn formats_context_usage_compactly() {
    let formatted = token_usage::format_context_token_usage(ContextTokenUsage {
        input_tokens: 100,
        context_window_tokens: Some(240_000),
    });

    assert!(formatted.contains("100/240k tks"));
}

/// Verifies context pressure styling thresholds.
#[test]
fn classifies_context_pressure_thresholds() {
    assert_eq!(
        token_usage::context_pressure_style(ContextTokenUsage {
            input_tokens: 79,
            context_window_tokens: Some(100),
        }),
        dim_style()
    );
    assert_eq!(
        token_usage::context_pressure_style(ContextTokenUsage {
            input_tokens: 80,
            context_window_tokens: Some(100),
        }),
        warning_style()
    );
    assert_eq!(
        token_usage::context_pressure_style(ContextTokenUsage {
            input_tokens: 90,
            context_window_tokens: Some(100),
        }),
        error_style()
    );
}
