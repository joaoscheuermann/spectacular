use crate::chat::renderer::{dim_style, paint, user_style, Renderer};
use crate::chat::ChatError;
use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::queue;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType};
use spectacular_commands::{CommandRegistry, CommandSearchMatch};
use std::io::{self, Write};
use std::sync::Arc;

const MAX_SUGGESTIONS: usize = 8;
const PROMPT_WIDTH: u16 = 2;

pub struct PromptEditor<'a, C> {
    renderer: &'a Renderer,
    registry: &'a Arc<CommandRegistry<C>>,
    state: PromptState,
    terminal: PromptTerminal,
    rendered_lines: u16,
}

#[derive(Default)]
struct PromptState {
    buffer: String,
    cursor: usize,
    selected: usize,
}

impl<'a, C> PromptEditor<'a, C> {
    pub fn new(renderer: &'a Renderer, registry: &'a Arc<CommandRegistry<C>>) -> Self {
        Self {
            renderer,
            registry,
            state: PromptState::default(),
            terminal: PromptTerminal,
            rendered_lines: 0,
        }
    }

    pub fn read_line(mut self) -> Result<String, ChatError> {
        let _raw_mode = RawModeGuard::enter()?;
        self.redraw()?;

        loop {
            let event = event::read().map_err(ChatError::Io)?;
            let Event::Key(key) = event else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match self.handle_key(key)? {
                PromptAction::Continue => self.redraw()?,
                PromptAction::Submit => {
                    let line = self.state.buffer.clone();
                    self.clear_rendered_block()?;
                    return Ok(line);
                }
                PromptAction::Exit => {
                    self.clear_rendered_block()?;
                    return Err(ChatError::Exit);
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<PromptAction, ChatError> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            return Ok(PromptAction::Exit);
        }

        match key.code {
            KeyCode::Enter => Ok(PromptAction::Submit),
            KeyCode::Esc => {
                self.state.clear();
                Ok(PromptAction::Continue)
            }
            KeyCode::Left => {
                self.state.move_left();
                Ok(PromptAction::Continue)
            }
            KeyCode::Right => {
                self.state.move_right();
                Ok(PromptAction::Continue)
            }
            KeyCode::Home => {
                self.state.move_home();
                Ok(PromptAction::Continue)
            }
            KeyCode::End => {
                self.state.move_end();
                Ok(PromptAction::Continue)
            }
            KeyCode::Backspace => {
                self.state.backspace();
                Ok(PromptAction::Continue)
            }
            KeyCode::Delete => {
                self.state.delete();
                Ok(PromptAction::Continue)
            }
            KeyCode::Up => {
                if self.suggestions().is_empty() {
                    return Ok(PromptAction::Continue);
                }
                self.state.select_previous();
                Ok(PromptAction::Continue)
            }
            KeyCode::Down => {
                let suggestions = self.suggestions();
                if suggestions.is_empty() {
                    return Ok(PromptAction::Continue);
                }
                self.state.select_next(suggestions.len());
                Ok(PromptAction::Continue)
            }
            KeyCode::Tab => {
                if let Some(command) = self.suggestions().get(self.state.selected).copied() {
                    self.state.complete_command(command.metadata.name);
                }
                Ok(PromptAction::Continue)
            }
            KeyCode::Char(character)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.state.insert_char(character);
                Ok(PromptAction::Continue)
            }
            _ => Ok(PromptAction::Continue),
        }
    }

    fn redraw(&mut self) -> Result<(), ChatError> {
        let suggestions = self.suggestions();
        self.state.clamp_selection(suggestions.len());

        self.clear_rendered_block()?;
        self.terminal
            .render_prompt(self.renderer, &self.state.buffer);

        for (index, suggestion) in suggestions.iter().enumerate() {
            println!();
            print!(
                "{}",
                suggestion_row(*suggestion, index == self.state.selected)
            );
        }

        self.rendered_lines = 1 + suggestions.len() as u16;
        self.move_cursor_to_input()?;
        io::stdout().flush().map_err(ChatError::Io)
    }

    fn clear_rendered_block(&mut self) -> Result<(), ChatError> {
        self.terminal.clear_block(self.rendered_lines)?;
        self.rendered_lines = 0;
        Ok(())
    }

    fn move_cursor_to_input(&self) -> Result<(), ChatError> {
        self.terminal
            .move_cursor_to_input(self.rendered_lines, self.cursor_column())
    }

    fn cursor_column(&self) -> u16 {
        PROMPT_WIDTH.saturating_add(self.state.buffer[..self.state.cursor].chars().count() as u16)
    }

    fn suggestions(&self) -> Vec<CommandSearchMatch> {
        self.state.suggestions(self.registry)
    }
}

#[derive(Clone, Copy)]
struct PromptTerminal;

impl PromptTerminal {
    fn render_prompt(&self, renderer: &Renderer, buffer: &str) {
        renderer.prompt();
        print!("{buffer}");
        print!("\x1b[0K");
    }

    fn clear_block(&self, rendered_lines: u16) -> Result<(), ChatError> {
        if rendered_lines == 0 {
            return Ok(());
        }

        let mut stdout = io::stdout();
        queue!(stdout, MoveToColumn(0)).map_err(ChatError::Io)?;
        for index in 0..rendered_lines {
            queue!(stdout, Clear(ClearType::CurrentLine)).map_err(ChatError::Io)?;
            if index + 1 < rendered_lines {
                queue!(stdout, MoveDown(1)).map_err(ChatError::Io)?;
            }
        }
        if rendered_lines > 1 {
            queue!(stdout, MoveUp(rendered_lines - 1)).map_err(ChatError::Io)?;
        }
        queue!(stdout, MoveToColumn(0)).map_err(ChatError::Io)?;
        stdout.flush().map_err(ChatError::Io)
    }

    fn move_cursor_to_input(&self, rendered_lines: u16, column: u16) -> Result<(), ChatError> {
        let mut stdout = io::stdout();
        if rendered_lines > 1 {
            queue!(stdout, MoveUp(rendered_lines - 1)).map_err(ChatError::Io)?;
        }
        queue!(stdout, MoveToColumn(column)).map_err(ChatError::Io)?;
        stdout.flush().map_err(ChatError::Io)
    }
}

impl PromptState {
    fn insert_char(&mut self, character: char) {
        self.buffer.insert(self.cursor, character);
        self.cursor += character.len_utf8();
        self.selected = 0;
    }

    fn backspace(&mut self) {
        let previous = previous_boundary(&self.buffer, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.buffer.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.selected = 0;
    }

    fn delete(&mut self) {
        let next = next_boundary(&self.buffer, self.cursor);
        if next == self.cursor {
            return;
        }

        self.buffer.replace_range(self.cursor..next, "");
        self.selected = 0;
    }

    fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.selected = 0;
    }

    fn move_left(&mut self) {
        self.cursor = previous_boundary(&self.buffer, self.cursor);
    }

    fn move_right(&mut self) {
        self.cursor = next_boundary(&self.buffer, self.cursor);
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn select_next(&mut self, suggestion_count: usize) {
        self.selected = (self.selected + 1).min(suggestion_count.saturating_sub(1));
    }

    fn clamp_selection(&mut self, suggestion_count: usize) {
        if self.selected >= suggestion_count {
            self.selected = 0;
        }
    }

    fn complete_command(&mut self, command_name: &str) {
        complete_command(&mut self.buffer, &mut self.cursor, command_name);
        self.selected = 0;
    }

    fn suggestions<C>(&self, registry: &CommandRegistry<C>) -> Vec<CommandSearchMatch> {
        let Some(query) = self.suggestion_query() else {
            return Vec::new();
        };

        registry.search(query, MAX_SUGGESTIONS)
    }

    fn suggestion_query(&self) -> Option<&str> {
        suggestion_query(&self.buffer, self.cursor)
    }
}

enum PromptAction {
    Continue,
    Submit,
    Exit,
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self, ChatError> {
        enable_raw_mode().map_err(ChatError::Io)?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn suggestion_row(suggestion: CommandSearchMatch, selected: bool) -> String {
    let text = format!(
        "  /{:<12} {}",
        suggestion.metadata.name, suggestion.metadata.summary
    );
    if selected {
        return paint(user_style(), text);
    }

    paint(dim_style(), text)
}

fn suggestion_query(buffer: &str, cursor: usize) -> Option<&str> {
    if !buffer.starts_with('/') || cursor == 0 || cursor > buffer.len() {
        return None;
    }

    let command_prefix = &buffer[..cursor];
    if command_prefix.chars().any(char::is_whitespace) {
        return None;
    }

    Some(&buffer[1..cursor])
}

fn complete_command(buffer: &mut String, cursor: &mut usize, command_name: &str) {
    if !buffer.starts_with('/') {
        return;
    }

    let command_end = buffer[1..]
        .find(char::is_whitespace)
        .map(|index| index + 1)
        .unwrap_or(buffer.len());
    buffer.replace_range(1..command_end, command_name);

    let insert_at = 1 + command_name.len();
    if buffer[insert_at..]
        .chars()
        .next()
        .is_some_and(char::is_whitespace)
    {
        *cursor = next_boundary(buffer, insert_at);
        return;
    }

    buffer.insert(insert_at, ' ');
    *cursor = insert_at + 1;
}

fn previous_boundary(buffer: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }

    buffer[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_boundary(buffer: &str, cursor: usize) -> usize {
    if cursor >= buffer.len() {
        return buffer.len();
    }

    buffer[cursor..]
        .char_indices()
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(buffer.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_commands::{Command, CommandControl, CommandFuture};

    #[test]
    fn slash_query_shows_all_commands() {
        assert_eq!(suggestion_query("/", 1), Some(""));
    }

    #[test]
    fn command_prefix_query_is_extracted() {
        assert_eq!(suggestion_query("/his", 4), Some("his"));
    }

    #[test]
    fn suggestions_hide_after_arguments_start() {
        assert_eq!(suggestion_query("/history 2", 10), None);
    }

    #[test]
    fn suggestions_can_show_when_cursor_is_still_in_command_name() {
        assert_eq!(suggestion_query("/history 2", 4), Some("his"));
    }

    #[test]
    fn cursor_boundaries_handle_utf8() {
        let text = "aé/";
        assert_eq!(next_boundary(text, 0), 1);
        assert_eq!(next_boundary(text, 1), 3);
        assert_eq!(previous_boundary(text, 3), 1);
    }

    #[test]
    fn completion_adds_trailing_space() {
        let mut buffer = "/his".to_owned();
        let mut cursor = buffer.len();

        complete_command(&mut buffer, &mut cursor, "history");

        assert_eq!(buffer, "/history ");
        assert_eq!(cursor, buffer.len());
    }

    #[test]
    fn completion_preserves_arguments() {
        let mut buffer = "/his 2".to_owned();
        let mut cursor = 4;

        complete_command(&mut buffer, &mut cursor, "history");

        assert_eq!(buffer, "/history 2");
        assert_eq!(cursor, "/history ".len());
    }

    #[test]
    fn prompt_state_slash_suggests_all_commands() {
        let registry = test_registry();
        let mut state = PromptState::default();
        state.insert_char('/');

        let suggestions = state.suggestions(&registry);

        assert_eq!(suggestions.len(), 3);
    }

    #[test]
    fn prompt_state_filters_command_suggestions() {
        let registry = test_registry();
        let state = state_with("/his");

        let suggestions = state.suggestions(&registry);

        assert_eq!(suggestions[0].metadata.name, "history");
    }

    #[test]
    fn prompt_state_hides_suggestions_after_whitespace() {
        let registry = test_registry();
        let state = state_with("/history 2");

        assert!(state.suggestions(&registry).is_empty());
    }

    #[test]
    fn prompt_state_selection_clamps() {
        let mut state = state_with("/");

        state.select_next(2);
        state.select_next(2);
        state.select_next(2);
        assert_eq!(state.selected, 1);
        state.select_previous();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn prompt_state_completion_preserves_arguments() {
        let mut state = state_with("/his 2");
        state.cursor = 4;

        state.complete_command("history");

        assert_eq!(state.buffer, "/history 2");
        assert_eq!(state.cursor, "/history ".len());
    }

    fn state_with(value: &str) -> PromptState {
        PromptState {
            buffer: value.to_owned(),
            cursor: value.len(),
            selected: 0,
        }
    }

    fn test_registry() -> CommandRegistry<()> {
        fn execute<'a>(_context: &'a mut (), _args: Vec<String>) -> CommandFuture<'a> {
            Box::pin(async { Ok(CommandControl::Continue) })
        }

        let mut registry = CommandRegistry::new();
        for name in ["clear", "history", "resume"] {
            registry
                .register(Command {
                    name,
                    usage: name,
                    summary: name,
                    execute,
                })
                .unwrap();
        }
        registry
    }
}
