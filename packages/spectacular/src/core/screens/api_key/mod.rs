use crate::core::keys::is_cancel_key;
use crate::core::terminal::TerminalSession;
use crate::core::widgets::input::Input;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::time::{Duration, Instant};

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

/// Runs the interactive Ratatui API key input screen.
pub fn run_api_key_screen(
    provider_name: &str,
    current_api_key: Option<&str>,
    mut validate_api_key: impl FnMut(&str) -> Result<(), String>,
) -> Result<String, ApiKeyInputError> {
    let mut session = TerminalSession::start().map_err(ApiKeyInputError::TerminalIo)?;
    let mut input = current_api_key.unwrap_or_default().to_owned();
    let mut cursor_position = input.chars().count();
    let mut cursor_visible = true;
    let mut last_cursor_blink = Instant::now();
    let mut status_message: Option<String> = None;

    loop {
        session.terminal.draw(|frame| {
            render_api_key_screen(
                frame,
                provider_name,
                &input,
                cursor_position,
                cursor_visible,
                status_message.as_deref(),
            );
        })?;

        let timeout = CURSOR_BLINK_INTERVAL.saturating_sub(last_cursor_blink.elapsed());
        if !event::poll(timeout)? {
            cursor_visible = !cursor_visible;
            last_cursor_blink = Instant::now();
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        cursor_visible = true;
        last_cursor_blink = Instant::now();

        if is_cancel_key(&key.code) {
            return Err(ApiKeyInputError::Cancelled);
        }

        match key.code {
            KeyCode::Char(character) => {
                insert_character(&mut input, cursor_position, character);
                cursor_position += 1;
                status_message = None;
            }
            KeyCode::Backspace => {
                if cursor_position > 0 {
                    cursor_position -= 1;
                    remove_character(&mut input, cursor_position);
                }
                status_message = None;
            }
            KeyCode::Delete => {
                remove_character(&mut input, cursor_position);
                status_message = None;
            }
            KeyCode::Left => {
                cursor_position = cursor_position.saturating_sub(1);
                status_message = None;
            }
            KeyCode::Right => {
                cursor_position = (cursor_position + 1).min(input.chars().count());
                status_message = None;
            }
            KeyCode::Home => {
                cursor_position = 0;
                status_message = None;
            }
            KeyCode::End => {
                cursor_position = input.chars().count();
                status_message = None;
            }
            KeyCode::Enter => match submit_api_key_input(&input, &mut validate_api_key) {
                Ok(api_key) => return Ok(api_key),
                Err(message) => status_message = Some(message),
            },
            _ => {}
        }
    }
}

/// Renders the API key input screen into the supplied Ratatui frame.
pub fn render_api_key_screen(
    frame: &mut Frame<'_>,
    provider_name: &str,
    input: &str,
    cursor_position: usize,
    cursor_visible: bool,
    status_message: Option<&str>,
) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new(format!("Spectacular {provider_name} API KEY"))
            .style(Style::default().fg(Color::Blue)),
        areas[0],
    );

    let description = Paragraph::new(format!(
        "Enter the API key to enable Spectacular to access {provider_name} provider."
    ))
    .wrap(Wrap { trim: true });

    frame.render_widget(description, areas[1]);

    let input = Input::new(input)
        .mask('*')
        .focused(true)
        .cursor_position(cursor_position)
        .cursor_visible(cursor_visible)
        .block(Block::default().borders(Borders::ALL).title(" API KEY "));
    frame.render_widget(input, areas[2]);

    let status = status_message.unwrap_or("Press Enter to validate, Esc to cancel.");
    frame.render_widget(Paragraph::new(status).wrap(Wrap { trim: true }), areas[4]);
}

/// Validates submitted API key text and returns the trimmed key on success.
pub fn submit_api_key_input(
    input: &str,
    validate_api_key: &mut impl FnMut(&str) -> Result<(), String>,
) -> Result<String, String> {
    let api_key = input.trim();

    if api_key.is_empty() {
        return Err("API key is required.".to_owned());
    }

    validate_api_key(api_key).map_err(|message| format!("Invalid API key: {message}"))?;

    Ok(api_key.to_owned())
}

fn insert_character(input: &mut String, cursor_position: usize, character: char) {
    let byte_index = byte_index_for_char_position(input, cursor_position);
    input.insert(byte_index, character);
}

fn remove_character(input: &mut String, cursor_position: usize) {
    let byte_index = byte_index_for_char_position(input, cursor_position);

    if byte_index >= input.len() {
        return;
    }

    input.remove(byte_index);
}

fn byte_index_for_char_position(input: &str, cursor_position: usize) -> usize {
    input
        .char_indices()
        .nth(cursor_position)
        .map(|(index, _)| index)
        .unwrap_or(input.len())
}

/// Errors returned by API key input UI.
#[derive(Debug)]
pub enum ApiKeyInputError {
    Cancelled,
    TerminalIo(io::Error),
}

impl From<io::Error> for ApiKeyInputError {
    fn from(source: io::Error) -> Self {
        Self::TerminalIo(source)
    }
}

impl Display for ApiKeyInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiKeyInputError::Cancelled => formatter.write_str("API key input was cancelled."),
            ApiKeyInputError::TerminalIo(source) => {
                write!(formatter, "API key input failed: {source}")
            }
        }
    }
}

impl Error for ApiKeyInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ApiKeyInputError::TerminalIo(source) => Some(source),
            ApiKeyInputError::Cancelled => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn api_key_screen_renders_provider_title_and_masks_input() {
        let backend = TestBackend::new(80, 13);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_api_key_screen(frame, "OpenRouter", "sk-or-v1-secret", 15, true, None);
            })
            .unwrap();

        let rendered = rendered_text(&terminal);

        assert!(rendered.contains("Spectacular OpenRouter API KEY"));
        assert!(rendered
            .contains("Enter the API key to enable Spectacular to access OpenRouter provider."));
        assert!(rendered.contains("***************"));
        assert!(!rendered.contains("sk-or-v1-secret"));
        assert_has_no_section_chrome(&rendered, &["API key", "API KEY"]);
    }

    #[test]
    fn api_key_screen_renders_autofilled_input() {
        let backend = TestBackend::new(80, 13);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_api_key_screen(frame, "OpenRouter", "existing-secret", 15, true, None);
            })
            .unwrap();

        let rendered = rendered_text(&terminal);

        assert!(rendered.contains("***************"));
        assert!(!rendered.contains("existing-secret"));
    }

    #[test]
    fn input_editing_inserts_and_removes_at_cursor() {
        let mut input = "abcd".to_owned();

        insert_character(&mut input, 2, 'X');
        remove_character(&mut input, 3);

        assert_eq!(input, "abXd");
    }

    #[test]
    fn api_key_submit_rejects_blank_input_before_validation() {
        let result = submit_api_key_input("   ", &mut |_| panic!("validation must not run"));

        assert_eq!(result, Err("API key is required.".to_owned()));
    }

    #[test]
    fn api_key_submit_returns_trimmed_key_after_validation() {
        let result = submit_api_key_input("  sk-or-v1-valid  ", &mut |api_key| {
            assert_eq!(api_key, "sk-or-v1-valid");
            Ok(())
        })
        .unwrap();

        assert_eq!(result, "sk-or-v1-valid");
    }

    #[test]
    fn api_key_submit_returns_clear_validation_error() {
        let result = submit_api_key_input("sk-or-v1-invalid", &mut |_| {
            Err("invalid API key".to_owned())
        });

        assert_eq!(result, Err("Invalid API key: invalid API key".to_owned()));
    }

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .fold(String::new(), |mut output, cell| {
                output.push_str(cell.symbol());
                output
            })
    }

    fn assert_has_no_section_chrome(rendered: &str, allowed_titles: &[&str]) {
        for title in ["Configuration", "Assignments", "API key", "API KEY"] {
            if allowed_titles.contains(&title) {
                continue;
            }

            assert!(!rendered.contains(title));
        }
    }
}
