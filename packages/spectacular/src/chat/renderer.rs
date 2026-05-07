use crate::chat::model::HistoryTableModel;
use crate::chat::runner::render_agent_event;
use crate::chat::session::ChatRecord;
use crate::chat::ChatError;
use crate::terminal_style;
use anstyle::Style;
use serde_json::Value;
use spectacular_agent::AgentEvent;
use spectacular_agent::ToolStorage;
use std::io::{self, Write};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

use super::RuntimeSelection;

/// Number of visible characters the terminal typewriter writes per tick while
/// an assistant delta is streaming. Combined with the 50 ms tick this paces
/// revealed text at ~600 chars/sec. Each provider delta is drained before the
/// renderer handles the next event, so no backlog is retained after the run
/// finishes or fails.
pub(crate) const TYPEWRITER_CHARS_PER_TICK: usize = 30;
const TYPEWRITER_TICK_INTERVAL: Duration = Duration::from_millis(50);
const OPENING_BANNER_MIN_WIDTH: usize = 52;

#[derive(Default)]
pub struct Renderer {
    working: Mutex<WorkingLineState>,
}

#[derive(Default)]
struct WorkingLineState {
    active: bool,
    frame: usize,
    pause_depth: usize,
}

pub struct ToolCallView {
    pub name: String,
    pub input: String,
}

pub struct ToolResultView {
    pub output: String,
    pub status: ToolStatus,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ToolStatus {
    Done,
    Failed,
}

impl Renderer {
    pub fn session_created(&self, id: &str, runtime: &RuntimeSelection, directory: &Path) {
        let banner = OpeningBannerView::from_runtime(id, runtime, directory);
        self.with_interrupted_working_line(|| {
            println!("{}", format_opening_banner(&banner));
            println!("");
        });
    }

    pub fn resumed(&self, id: &str) {
        self.notice(format!("resumed session {id}"));
    }

    pub fn clear_screen(&self) {
        self.reset_working();
        print!("\x1b[2J\x1b[3J\x1b[H");
        let _ = io::stdout().flush();
    }

    pub fn user_prompt(&self, prompt: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(user_style(), prompt));
            println!();
        });
    }

    pub fn prompt(&self) {
        self.interrupt_working_line();
        print!("{}", paint(user_style(), "> "));
    }

    pub fn working(&self) {
        self.working_frame(0);
    }

    pub fn working_frame(&self, frame: usize) {
        let should_render = {
            let mut state = self.working_state();
            state.active = true;
            state.frame = frame;
            state.pause_depth == 0
        };

        if should_render {
            Self::write_working_frame(frame);
        }
    }

    pub fn clear_working(&self) {
        self.reset_working();
        Self::clear_working_line();
    }

    pub async fn assistant_delta(&self, content: &str) -> Result<(), ChatError> {
        let paused = self.pause_working();
        let result = async {
            let mut characters = content.chars().peekable();
            while characters.peek().is_some() {
                let chunk = characters
                    .by_ref()
                    .take(TYPEWRITER_CHARS_PER_TICK)
                    .collect::<String>();
                print!("{}", paint(assistant_style(), chunk));
                io::stdout().flush().map_err(ChatError::Io)?;
                if characters.peek().is_some() {
                    tokio::time::sleep(TYPEWRITER_TICK_INTERVAL).await;
                }
            }

            Ok(())
        }
        .await;
        if paused {
            self.resume_working();
        }

        result
    }

    pub fn assistant_text(&self, content: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(assistant_style(), content));
            println!();
        });
    }

    pub fn tool_call(&self, _tool_call_id: &str, name: &str, arguments: &str, tools: &ToolStorage) {
        self.render_tool_call(&ToolCallView::from_parts(name, arguments, tools));
    }

    pub fn render_tool_call(&self, view: &ToolCallView) {
        self.with_interrupted_working_line(|| {
            println!("{} {}", paint(tool_style(), &view.name), view.input);
        });
    }

    pub fn tool_result(&self, name: &str, content: &str, tools: &ToolStorage) {
        self.render_tool_result(&ToolResultView::from_parts(name, content, tools));
    }

    pub fn render_tool_result(&self, view: &ToolResultView) {
        let _ = &view.status;
        self.with_interrupted_working_line(|| {
            println!("└ {}", view.output);
            println!();
        });
    }

    pub fn error(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(error_style(), format!("error: {message}")));
            println!();
        });
    }

    pub fn command_error(&self, error: &impl std::fmt::Display) {
        self.error(&error.to_string());
    }

    pub fn warning(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(warning_style(), format!("warning: {message}")));
        });
    }

    pub fn cancelled(&self, reason: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(warning_style(), reason));
            println!();
        });
    }

    pub fn dim(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), message));
        });
    }

    pub fn success(&self, message: &str) {
        self.with_interrupted_working_line(|| {
            println!("{}", paint(success_style(), message));
        });
    }

    pub async fn render_records(
        &self,
        records: &[ChatRecord],
        tools: &ToolStorage,
    ) -> Result<(), ChatError> {
        let mut assistant_buffer = String::new();
        for record in records {
            if matches!(record, ChatRecord::Corrupt { .. }) {
                self.flush_assistant(&mut assistant_buffer);
                self.warning(&format!(
                    "unreadable session event at line {}",
                    record.line()
                ));
                continue;
            }

            let Some(event) = record.event() else {
                self.flush_assistant(&mut assistant_buffer);
                let event_type = match record {
                    ChatRecord::Unknown { value, .. } => value
                        .get("type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unknown"),
                    ChatRecord::Corrupt { .. } | ChatRecord::Known { .. } => "unknown",
                };
                self.warning(&format!(
                    "unknown session event `{event_type}` at line {}",
                    record.line()
                ));
                continue;
            };

            let Some(event) = event.to_agent_event() else {
                continue;
            };

            if let AgentEvent::MessageDelta(delta) = &event {
                assistant_buffer.push_str(&delta.content);
                continue;
            }

            self.flush_assistant(&mut assistant_buffer);
            render_agent_event(self, tools, &event).await?;
        }

        self.flush_assistant(&mut assistant_buffer);
        Ok(())
    }

    pub fn history_table(&self, table: &HistoryTableModel) {
        self.with_interrupted_working_line(|| {
            println!("sessions");
            println!("hash      updated           title                  messages");
            for session in &table.rows {
                let marker = if session.corrupt { "*" } else { " " };
                println!(
                    "{:<8}  {:<16}  {:<22}  {}{}",
                    session.id, session.updated, session.title, session.messages, marker
                );
            }
        });
        if table.remaining > 0 {
            self.with_interrupted_working_line(|| {
                println!();
            });
            self.dim(&format!("{} more sessions", table.remaining));
        }
    }

    fn flush_assistant(&self, buffer: &mut String) {
        if buffer.is_empty() {
            return;
        }

        self.assistant_text(buffer);
        buffer.clear();
    }

    fn notice(&self, message: impl Into<String>) {
        let content = message.into();
        self.with_interrupted_working_line(|| {
            println!("{}", paint(dim_style(), content));
        });
    }

    fn working_state(&self) -> MutexGuard<'_, WorkingLineState> {
        self.working
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn reset_working(&self) {
        let mut state = self.working_state();
        state.active = false;
        state.pause_depth = 0;
    }

    fn renderable_working_frame(&self) -> Option<usize> {
        let state = self.working_state();
        if state.active && state.pause_depth == 0 {
            return Some(state.frame);
        }

        None
    }

    fn with_interrupted_working_line(&self, write: impl FnOnce()) {
        let frame = self.renderable_working_frame();
        if frame.is_some() {
            Self::clear_working_line();
        }

        write();

        if let Some(frame) = frame {
            if self.renderable_working_frame().is_some() {
                Self::write_working_frame(frame);
            }
        }
    }

    fn interrupt_working_line(&self) {
        if self.renderable_working_frame().is_some() {
            Self::clear_working_line();
        }
    }

    fn pause_working(&self) -> bool {
        let (did_pause, should_clear) = {
            let mut state = self.working_state();
            let should_clear = state.active && state.pause_depth == 0;
            let did_pause = state.active;
            if state.active {
                state.pause_depth += 1;
            }
            (did_pause, should_clear)
        };

        if should_clear {
            Self::clear_working_line();
        }

        did_pause
    }

    fn resume_working(&self) {
        let mut state = self.working_state();
        if state.pause_depth > 0 {
            state.pause_depth -= 1;
        }
    }

    fn write_working_frame(frame: usize) {
        const FRAMES: &[&str] = &["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = FRAMES[frame % FRAMES.len()];
        print!(
            "\r\x1b[2K{}",
            paint(dim_style(), format!("working {frame} (Ctrl+C to stop)"))
        );
        let _ = io::stdout().flush();
    }

    fn clear_working_line() {
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }
}

#[derive(Debug, Eq, PartialEq)]
struct OpeningBannerView {
    version: String,
    model: String,
    reasoning: String,
    directory: String,
    session_id: String,
}

impl OpeningBannerView {
    fn from_runtime(id: &str, runtime: &RuntimeSelection, directory: &Path) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            model: runtime.model.clone(),
            reasoning: runtime.reasoning.to_string(),
            directory: format_directory(directory),
            session_id: id.to_owned(),
        }
    }
}

fn format_opening_banner(view: &OpeningBannerView) -> String {
    let title = format!("Spectacular (v{})", view.version);
    let spacer = String::new();
    let model = format!(
        "model:     {} {}   /model to change",
        view.model, view.reasoning
    );
    let directory = format!("directory: {}", view.directory);
    let session = format!("session:   {}", view.session_id);
    let lines = [&title, &spacer, &model, &directory, &session];
    let content_width = lines
        .iter()
        .map(|line| UnicodeWidthStr::width(line.as_str()))
        .max()
        .unwrap_or(0)
        .max(OPENING_BANNER_MIN_WIDTH);
    let horizontal = "─".repeat(content_width + 2);
    let green = terminal_style::title_style();
    let mut rendered = vec![paint(green, format!("╭{horizontal}╮"))];
    rendered.push(format!(
        "{} {} {}",
        paint(green, "│"),
        paint(green, pad_banner_line(&title, content_width)),
        paint(green, "│")
    ));
    rendered.extend(
        lines
            .iter()
            .skip(1)
            .map(|line| {
                format!(
                    "{} {} {}",
                    paint(green, "│"),
                    pad_banner_line(line, content_width),
                    paint(green, "│")
                )
            }),
    );
    rendered.push(paint(green, format!("╰{horizontal}╯")));
    rendered.join("\n")
}

fn pad_banner_line(line: &str, width: usize) -> String {
    let padding = width.saturating_sub(UnicodeWidthStr::width(line));
    format!("{line}{}", " ".repeat(padding))
}

fn format_directory(directory: &Path) -> String {
    format_directory_with_home(directory, home_dir().as_deref())
}

fn format_directory_with_home(directory: &Path, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return directory.display().to_string();
    };

    if directory == home {
        return "~".to_owned();
    }

    let Ok(relative) = directory.strip_prefix(home) else {
        return directory.display().to_string();
    };

    if relative.as_os_str().is_empty() {
        return "~".to_owned();
    }

    format!("~{}{}", MAIN_SEPARATOR, relative.display())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut home = drive;
            home.push(path);
            Some(PathBuf::from(home)).filter(|path| !path.as_os_str().is_empty())
        })
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| !path.as_os_str().is_empty())
        })
}

impl ToolCallView {
    pub fn from_parts(name: &str, arguments: &str, tools: &ToolStorage) -> Self {
        let name = if name.trim().is_empty() { "tool" } else { name };
        let parsed_arguments = serde_json::from_str::<Value>(arguments).ok();
        let input = match (tools.get(name), parsed_arguments.as_ref()) {
            (Some(tool), Some(arguments)) => tool.format_input(arguments),
            _ => format_json_preview(arguments),
        };

        Self {
            name: name.to_owned(),
            input,
        }
    }
}

impl ToolResultView {
    pub fn from_parts(name: &str, content: &str, tools: &ToolStorage) -> Self {
        let parsed = serde_json::from_str::<Value>(content).ok();
        let output = match tools.get(name) {
            Some(tool) => tool.format_output(content, parsed.as_ref()),
            None => format_json_preview(content),
        };

        Self {
            output,
            status: if result_failed(content, parsed.as_ref()) {
                ToolStatus::Failed
            } else {
                ToolStatus::Done
            },
        }
    }
}

fn result_failed(content: &str, parsed: Option<&Value>) -> bool {
    if content.trim_start().starts_with("Error:") {
        return true;
    }

    let Some(value) = parsed else {
        return false;
    };

    if value.get("error").is_some() || value.get("error_kind").is_some() {
        return true;
    }

    value
        .get("exit_code")
        .and_then(Value::as_i64)
        .is_some_and(|exit_code| exit_code != 0)
}

pub fn paint(style: Style, value: impl AsRef<str>) -> String {
    terminal_style::paint(style, value)
}

fn format_json_preview(value: &str) -> String {
    let parsed = serde_json::from_str::<Value>(value);
    let value = match parsed {
        Ok(Value::Object(map)) => map
            .into_iter()
            .map(|(key, value)| format!("{key}: {}", compact_value(&value)))
            .collect::<Vec<_>>()
            .join(", "),
        Ok(value) => compact_value(&value),
        Err(_) => value.to_owned(),
    };

    const LIMIT: usize = 180;
    if value.chars().count() <= LIMIT {
        return value;
    }

    value.chars().take(LIMIT).collect::<String>() + "..."
}

fn compact_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

pub fn user_style() -> Style {
    terminal_style::user_style()
}

fn assistant_style() -> Style {
    terminal_style::assistant_style()
}

fn tool_style() -> Style {
    terminal_style::tool_style()
}

fn success_style() -> Style {
    terminal_style::success_style()
}

fn warning_style() -> Style {
    terminal_style::warning_style()
}

fn error_style() -> Style {
    terminal_style::error_style()
}

pub fn dim_style() -> Style {
    terminal_style::dim_style()
}

pub fn selection_style() -> Style {
    terminal_style::selection_style()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use spectacular_agent::{Cancellation, Tool, ToolDisplay, ToolExecution, ToolManifest};
    use spectacular_config::ReasoningLevel;

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

        fn execute<'a>(
            &'a self,
            _arguments: Value,
            _cancellation: Cancellation,
        ) -> ToolExecution<'a> {
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
        assert!(banner.contains("model:     gpt-5.5 high   /model to change"));
        assert!(banner.contains(&format!(
            "directory: ~{}Documents{}git{}Personal{}spectacular",
            MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR
        )));
        assert!(banner.contains("session:   a83f19c2"));

        let widths = banner
            .lines()
            .map(|line| strip_ansi_codes(line))
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
            provider: "openrouter".to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
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
}
