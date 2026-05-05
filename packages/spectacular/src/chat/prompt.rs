use crate::chat::paste_burst::{CharDecision, FlushResult, PasteBurst};
use crate::chat::renderer::{dim_style, paint, user_style, Renderer};
use crate::chat::ChatError;
use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::queue;
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, Clear, ClearType};
use spectacular_commands::{CommandRegistry, CommandSearchMatch};
use std::io::{self, Write};
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

const DEFAULT_TERMINAL_WIDTH: u16 = 80;
const MAX_SUGGESTIONS: usize = 8;
const PROMPT_WIDTH: u16 = 2;

pub struct PromptEditor<'a, C> {
    renderer: &'a Renderer,
    registry: &'a Arc<CommandRegistry<C>>,
    state: PromptState,
    terminal: PromptTerminal,
    rendered_lines: u16,
    rendered_cursor_row: u16,
    paste_burst: PasteBurst,
}

#[derive(Default)]
struct PromptState {
    buffer: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    selected: usize,
    preferred_column: Option<usize>,
    kill_buffer: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VisualRow {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CursorPosition {
    row: usize,
    column: usize,
}

impl<'a, C> PromptEditor<'a, C> {
    pub fn new(renderer: &'a Renderer, registry: &'a Arc<CommandRegistry<C>>) -> Self {
        Self {
            renderer,
            registry,
            state: PromptState::default(),
            terminal: PromptTerminal,
            rendered_lines: 0,
            rendered_cursor_row: 0,
            paste_burst: PasteBurst::default(),
        }
    }

    pub fn read_line(mut self) -> Result<String, ChatError> {
        let _raw_mode = RawModeGuard::enter()?;
        self.redraw()?;

        loop {
            match self.read_next_action()? {
                PromptAction::Noop => {}
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

    fn read_next_action(&mut self) -> Result<PromptAction, ChatError> {
        loop {
            if self.paste_burst.is_active() {
                if event::poll(self.paste_burst.poll_delay()).map_err(ChatError::Io)? {
                    let event = event::read().map_err(ChatError::Io)?;
                    return self.handle_event_with_time(event, Instant::now());
                }

                if self.flush_paste_burst_if_due(Instant::now()) {
                    return Ok(PromptAction::Continue);
                }

                continue;
            }

            let event = event::read().map_err(ChatError::Io)?;
            return self.handle_event_with_time(event, Instant::now());
        }
    }

    #[cfg(test)]
    fn handle_event(&mut self, event: Event) -> Result<PromptAction, ChatError> {
        self.handle_event_with_time(event, Instant::now())
    }

    fn handle_event_with_time(
        &mut self,
        event: Event,
        now: Instant,
    ) -> Result<PromptAction, ChatError> {
        match event {
            Event::Key(key) if is_key_edit_event(key) => self.handle_key_with_time(key, now),
            Event::Paste(text) => {
                self.flush_paste_burst_before_modified_input();
                self.handle_paste(&text);
                Ok(PromptAction::Continue)
            }
            _ => Ok(PromptAction::Noop),
        }
    }

    fn handle_key_with_time(
        &mut self,
        key: KeyEvent,
        now: Instant,
    ) -> Result<PromptAction, ChatError> {
        if is_ctrl_char(key, 'c') {
            if self.state.buffer.is_empty() {
                return Ok(PromptAction::Exit);
            }
            self.state.buffer.clear();
            self.state.cursor = 0;
            self.state.after_edit();
            self.paste_burst.clear_window_after_non_char();
            return Ok(PromptAction::Continue);
        }

        if is_newline_key(key) {
            return Ok(self.handle_newline_with_time(now));
        }

        if is_submit_key(key) || is_unmodified_line_break_char(key) {
            return Ok(self.handle_submit_with_time(now));
        }

        let prompt_changed = self.flush_paste_burst_if_due(now);

        if is_ctrl_char(key, 'a') {
            self.flush_paste_burst_before_modified_input();
            self.state.select_all();
            self.paste_burst.clear_window_after_non_char();
            return Ok(PromptAction::Continue);
        }

        if is_ctrl_char(key, 'u') {
            self.flush_paste_burst_before_modified_input();
            self.state.kill_to_line_start();
            self.paste_burst.clear_window_after_non_char();
            return Ok(PromptAction::Continue);
        }

        if is_ctrl_char(key, 'k') {
            self.flush_paste_burst_before_modified_input();
            self.state.kill_to_line_end();
            self.paste_burst.clear_window_after_non_char();
            return Ok(PromptAction::Continue);
        }

        if is_ctrl_char(key, 'y') {
            self.flush_paste_burst_before_modified_input();
            self.state.yank();
            self.paste_burst.clear_window_after_non_char();
            return Ok(PromptAction::Continue);
        }

        match key.code {
            KeyCode::Esc => {
                self.flush_paste_burst_before_modified_input();
                self.state.escape();
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Left => {
                self.flush_paste_burst_before_modified_input();
                if moves_by_word(key) {
                    self.state.move_word_left(selects_text(key));
                } else {
                    self.state.move_left(selects_text(key));
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Right => {
                self.flush_paste_burst_before_modified_input();
                if moves_by_word(key) {
                    self.state.move_word_right(selects_text(key));
                } else {
                    self.state.move_right(selects_text(key));
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Home => {
                self.flush_paste_burst_before_modified_input();
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.state.move_start(selects_text(key));
                } else {
                    self.state.move_line_start(selects_text(key));
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::End => {
                self.flush_paste_burst_before_modified_input();
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.state.move_end(selects_text(key));
                } else {
                    self.state.move_line_end(selects_text(key));
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Backspace => {
                self.flush_paste_burst_before_modified_input();
                if moves_by_word(key) {
                    self.state.delete_previous_word();
                } else {
                    self.state.backspace();
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Delete => {
                self.flush_paste_burst_before_modified_input();
                if moves_by_word(key) {
                    self.state.delete_next_word();
                } else {
                    self.state.delete();
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Up => {
                self.flush_paste_burst_before_modified_input();
                if self.should_move_suggestion(key) {
                    self.state.select_previous();
                } else {
                    self.state
                        .move_visual_up(selects_text(key), self.content_width());
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Down => {
                self.flush_paste_burst_before_modified_input();
                let suggestions = self.suggestions();
                if self.should_move_suggestion(key) {
                    self.state.select_next(suggestions.len());
                } else {
                    self.state
                        .move_visual_down(selects_text(key), self.content_width());
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Tab => {
                self.flush_paste_burst_before_modified_input();
                if let Some(command) = self.suggestions().get(self.state.selected).copied() {
                    self.state.complete_command(command.metadata.name);
                } else {
                    self.state.insert_char('\t');
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Char(character) if should_insert_char(key) => {
                if is_plain_paste_candidate_key(key) {
                    return Ok(self.handle_plain_char_with_time(character, now, prompt_changed));
                }

                self.flush_paste_burst_before_modified_input();
                self.state.insert_char(character);
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            _ => {
                self.flush_paste_burst_before_modified_input();
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
        }
    }

    fn handle_newline_with_time(&mut self, now: Instant) -> PromptAction {
        if self.paste_burst.append_newline_if_active(now) {
            return PromptAction::Noop;
        }

        self.flush_paste_burst_before_modified_input();
        self.state.insert_str("\n");
        self.paste_burst.clear_window_after_non_char();
        PromptAction::Continue
    }

    fn handle_submit_with_time(&mut self, now: Instant) -> PromptAction {
        if self.is_slash_context() {
            self.flush_paste_burst_before_modified_input();
            return PromptAction::Submit;
        }

        if self.paste_burst.append_newline_if_active(now) {
            return PromptAction::Noop;
        }

        if self
            .paste_burst
            .newline_should_insert_instead_of_submit(now)
        {
            self.state.insert_str("\n");
            self.paste_burst.extend_window(now);
            return PromptAction::Continue;
        }

        PromptAction::Submit
    }

    fn handle_plain_char_with_time(
        &mut self,
        character: char,
        now: Instant,
        prompt_changed: bool,
    ) -> PromptAction {
        match self.paste_burst.on_plain_char(character, now) {
            CharDecision::Buffered | CharDecision::Held => {}
        }

        if prompt_changed {
            return PromptAction::Continue;
        }

        PromptAction::Noop
    }

    fn flush_paste_burst_if_due(&mut self, now: Instant) -> bool {
        match self.paste_burst.flush_if_due(now) {
            FlushResult::Paste(pasted) => {
                self.state.insert_str(&pasted);
                true
            }
            FlushResult::Typed(character) => {
                self.state.insert_char(character);
                true
            }
            FlushResult::None => false,
        }
    }

    fn flush_paste_burst_before_modified_input(&mut self) {
        if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
            self.state.insert_str(&pasted);
        }
    }

    fn handle_paste(&mut self, pasted: &str) {
        self.state.insert_str(&normalize_paste(pasted));
        self.paste_burst.clear_after_explicit_paste();
    }

    fn is_slash_context(&self) -> bool {
        if self
            .state
            .buffer
            .lines()
            .next()
            .unwrap_or_default()
            .starts_with('/')
        {
            return true;
        }

        self.state.buffer.is_empty() && self.paste_burst.starts_with('/')
    }

    fn redraw(&mut self) -> Result<(), ChatError> {
        let suggestions = self.suggestions();
        self.state.clamp_selection(suggestions.len());

        let content_width = self.content_width();
        let rows = self.state.visual_rows(content_width);
        let cursor_position = self.state.cursor_position(content_width, &rows);

        self.clear_rendered_block()?;
        self.terminal
            .render_prompt_rows(self.renderer, &self.state, &rows);

        for (index, suggestion) in suggestions.iter().enumerate() {
            println!();
            print!(
                "{}",
                suggestion_row(*suggestion, index == self.state.selected)
            );
        }

        self.rendered_lines = saturating_u16(rows.len() + suggestions.len());
        self.rendered_cursor_row = saturating_u16(cursor_position.row);
        self.move_cursor_to_input(cursor_position)?;
        io::stdout().flush().map_err(ChatError::Io)
    }

    fn clear_rendered_block(&mut self) -> Result<(), ChatError> {
        self.terminal
            .clear_block(self.rendered_lines, self.rendered_cursor_row)?;
        self.rendered_lines = 0;
        self.rendered_cursor_row = 0;
        Ok(())
    }

    fn move_cursor_to_input(&self, cursor_position: CursorPosition) -> Result<(), ChatError> {
        self.terminal
            .move_cursor_to_input(self.rendered_lines, cursor_position)
    }

    fn should_move_suggestion(&self, key: KeyEvent) -> bool {
        key.modifiers == KeyModifiers::NONE && !self.suggestions().is_empty()
    }

    fn suggestions(&self) -> Vec<CommandSearchMatch> {
        self.state.suggestions(self.registry)
    }

    fn content_width(&self) -> usize {
        let terminal_width = terminal::size()
            .map(|(width, _)| width)
            .unwrap_or(DEFAULT_TERMINAL_WIDTH);
        usize::from(terminal_width.saturating_sub(PROMPT_WIDTH).max(1))
    }
}

#[derive(Clone, Copy)]
struct PromptTerminal;

impl PromptTerminal {
    fn render_prompt_rows(&self, renderer: &Renderer, state: &PromptState, rows: &[VisualRow]) {
        for (index, row) in rows.iter().enumerate() {
            if index > 0 {
                println!();
                print_prompt_indent();
            } else {
                renderer.prompt();
            }

            render_buffer_range(&state.buffer, row.start..row.end, state.selection_range());
            print!("\x1b[0K");
        }
    }

    fn clear_block(&self, rendered_lines: u16, cursor_row: u16) -> Result<(), ChatError> {
        if rendered_lines == 0 {
            return Ok(());
        }

        let mut stdout = io::stdout();
        let cursor_row = cursor_row.min(rendered_lines.saturating_sub(1));
        if cursor_row > 0 {
            queue!(stdout, MoveUp(cursor_row)).map_err(ChatError::Io)?;
        }
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

    fn move_cursor_to_input(
        &self,
        rendered_lines: u16,
        cursor_position: CursorPosition,
    ) -> Result<(), ChatError> {
        let mut stdout = io::stdout();
        let cursor_row = saturating_u16(cursor_position.row);
        let last_row = rendered_lines.saturating_sub(1);
        if last_row > cursor_row {
            queue!(stdout, MoveUp(last_row - cursor_row)).map_err(ChatError::Io)?;
        }

        let column = PROMPT_WIDTH.saturating_add(saturating_u16(cursor_position.column));
        queue!(stdout, MoveToColumn(column)).map_err(ChatError::Io)?;
        stdout.flush().map_err(ChatError::Io)
    }
}

impl PromptState {
    fn insert_char(&mut self, character: char) {
        self.insert_str(&character.to_string());
    }

    fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.delete_selection();
        self.buffer.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.after_edit();
    }

    fn backspace(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let previous = previous_boundary(&self.buffer, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.buffer.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.after_edit();
    }

    fn delete(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let next = next_boundary(&self.buffer, self.cursor);
        if next == self.cursor {
            return;
        }

        self.buffer.replace_range(self.cursor..next, "");
        self.after_edit();
    }

    fn delete_previous_word(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let previous = previous_word_boundary(&self.buffer, self.cursor);
        if previous == self.cursor {
            return;
        }

        self.buffer.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        self.after_edit();
    }

    fn delete_next_word(&mut self) {
        if self.delete_selection() {
            self.after_edit();
            return;
        }

        let next = next_word_boundary(&self.buffer, self.cursor);
        if next == self.cursor {
            return;
        }

        self.buffer.replace_range(self.cursor..next, "");
        self.after_edit();
    }

    fn kill_to_line_start(&mut self) {
        if self.delete_selection_to_kill_buffer() {
            self.after_edit();
            return;
        }

        let line_start = line_start(&self.buffer, self.cursor);
        if line_start == self.cursor {
            return;
        }

        self.kill_buffer = self.buffer[line_start..self.cursor].to_owned();
        self.buffer.replace_range(line_start..self.cursor, "");
        self.cursor = line_start;
        self.after_edit();
    }

    fn kill_to_line_end(&mut self) {
        if self.delete_selection_to_kill_buffer() {
            self.after_edit();
            return;
        }

        let line_end = line_end(&self.buffer, self.cursor);
        if line_end == self.cursor {
            return;
        }

        self.kill_buffer = self.buffer[self.cursor..line_end].to_owned();
        self.buffer.replace_range(self.cursor..line_end, "");
        self.after_edit();
    }

    fn yank(&mut self) {
        if self.kill_buffer.is_empty() {
            return;
        }

        let value = self.kill_buffer.clone();
        self.insert_str(&value);
    }

    fn escape(&mut self) {
        if self.selection_anchor.is_some() {
            self.clear_selection();
            return;
        }

        self.buffer.clear();
        self.cursor = 0;
        self.after_edit();
    }

    fn move_left(&mut self, selecting: bool) {
        let cursor = previous_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    fn move_right(&mut self, selecting: bool) {
        let cursor = next_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    fn move_word_left(&mut self, selecting: bool) {
        let cursor = previous_word_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    fn move_word_right(&mut self, selecting: bool) {
        let cursor = next_word_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    fn move_line_start(&mut self, selecting: bool) {
        self.move_to(line_start(&self.buffer, self.cursor), selecting);
    }

    fn move_line_end(&mut self, selecting: bool) {
        self.move_to(line_end(&self.buffer, self.cursor), selecting);
    }

    fn move_start(&mut self, selecting: bool) {
        self.move_to(0, selecting);
    }

    fn move_end(&mut self, selecting: bool) {
        self.move_to(self.buffer.len(), selecting);
    }

    fn move_visual_up(&mut self, selecting: bool, content_width: usize) {
        self.move_visual_line(-1, selecting, content_width);
    }

    fn move_visual_down(&mut self, selecting: bool, content_width: usize) {
        self.move_visual_line(1, selecting, content_width);
    }

    fn select_all(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        self.selection_anchor = Some(0);
        self.cursor = self.buffer.len();
        self.selected = 0;
        self.preferred_column = None;
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
        self.clear_selection();
        self.selected = 0;
        self.preferred_column = None;
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

    fn selection_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }

        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    fn visual_rows(&self, content_width: usize) -> Vec<VisualRow> {
        visual_rows(&self.buffer, content_width)
    }

    fn cursor_position(&self, content_width: usize, rows: &[VisualRow]) -> CursorPosition {
        let row = row_for_cursor(rows, self.cursor);
        let start = rows.get(row).map(|row| row.start).unwrap_or(0);
        let end = rows.get(row).map(|row| row.end).unwrap_or(start);
        let cursor = self.cursor.min(end);
        CursorPosition {
            row,
            column: display_width(&self.buffer[start..cursor]).min(content_width),
        }
    }

    fn move_visual_line(&mut self, delta: isize, selecting: bool, content_width: usize) {
        let rows = self.visual_rows(content_width);
        let current_row = row_for_cursor(&rows, self.cursor);
        let current_position = self.cursor_position(content_width, &rows);
        let column = self.preferred_column.unwrap_or(current_position.column);
        let target_row = if delta < 0 {
            current_row.saturating_sub(delta.unsigned_abs())
        } else {
            (current_row + delta as usize).min(rows.len().saturating_sub(1))
        };

        self.preferred_column = Some(column);
        let cursor = cursor_at_column(&self.buffer, &rows[target_row], column);
        self.move_to_preserving_preferred_column(cursor, selecting);
    }

    fn move_to(&mut self, cursor: usize, selecting: bool) {
        self.preferred_column = None;
        self.move_to_preserving_preferred_column(cursor, selecting);
    }

    fn move_to_preserving_preferred_column(&mut self, cursor: usize, selecting: bool) {
        let previous_cursor = self.cursor;
        self.cursor = clamp_to_boundary(&self.buffer, cursor);
        if selecting {
            self.selection_anchor.get_or_insert(previous_cursor);
            if self.selection_anchor == Some(self.cursor) {
                self.clear_selection();
            }
        } else {
            self.clear_selection();
        }
        self.selected = 0;
    }

    fn delete_selection(&mut self) -> bool {
        let Some(selection) = self.selection_range() else {
            return false;
        };

        self.buffer.replace_range(selection.clone(), "");
        self.cursor = selection.start;
        self.clear_selection();
        true
    }

    fn delete_selection_to_kill_buffer(&mut self) -> bool {
        let Some(selection) = self.selection_range() else {
            return false;
        };

        self.kill_buffer = self.buffer[selection.clone()].to_owned();
        self.buffer.replace_range(selection.clone(), "");
        self.cursor = selection.start;
        self.clear_selection();
        true
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    fn after_edit(&mut self) {
        self.clear_selection();
        self.selected = 0;
        self.preferred_column = None;
    }
}

enum PromptAction {
    Noop,
    Continue,
    Submit,
    Exit,
}

struct RawModeGuard;

impl RawModeGuard {
    fn enter() -> Result<Self, ChatError> {
        enable_raw_mode().map_err(ChatError::Io)?;
        let mut stdout = io::stdout();
        if let Err(error) = queue!(stdout, EnableBracketedPaste).and_then(|_| stdout.flush()) {
            let _ = disable_raw_mode();
            return Err(ChatError::Io(error));
        }
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = queue!(stdout, DisableBracketedPaste);
        let _ = stdout.flush();
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

fn render_buffer_range(buffer: &str, range: Range<usize>, selection: Option<Range<usize>>) {
    if range.is_empty() {
        if selection
            .is_some_and(|selection| selection.start <= range.start && selection.end > range.start)
        {
            print!("\x1b[7m \x1b[27m");
        }
        return;
    }

    let Some(selection) = selection else {
        print!("{}", display_text(&buffer[range]));
        return;
    };

    let selected_start = selection.start.max(range.start);
    let selected_end = selection.end.min(range.end);
    if selected_start >= selected_end {
        print!("{}", display_text(&buffer[range]));
        return;
    }

    print!("{}", display_text(&buffer[range.start..selected_start]));
    print!(
        "\x1b[7m{}\x1b[27m",
        display_text(&buffer[selected_start..selected_end])
    );
    print!("{}", display_text(&buffer[selected_end..range.end]));
}

fn print_prompt_indent() {
    print!("{:width$}", "", width = usize::from(PROMPT_WIDTH));
}

fn display_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '\t' => "    ".chars().collect::<Vec<_>>(),
            character if character.is_control() => Vec::new(),
            character => vec![character],
        })
        .collect()
}

fn visual_rows(buffer: &str, content_width: usize) -> Vec<VisualRow> {
    let content_width = content_width.max(1);
    if buffer.is_empty() {
        return vec![VisualRow { start: 0, end: 0 }];
    }

    let mut rows = Vec::new();
    let mut line_start = 0;
    loop {
        let line_end = buffer[line_start..]
            .find('\n')
            .map(|index| line_start + index)
            .unwrap_or(buffer.len());
        push_wrapped_line(buffer, line_start, line_end, content_width, &mut rows);

        if line_end == buffer.len() {
            break;
        }

        line_start = line_end + 1;
        if line_start == buffer.len() {
            rows.push(VisualRow {
                start: buffer.len(),
                end: buffer.len(),
            });
            break;
        }
    }

    rows
}

fn push_wrapped_line(
    buffer: &str,
    line_start: usize,
    line_end: usize,
    content_width: usize,
    rows: &mut Vec<VisualRow>,
) {
    if line_start == line_end {
        rows.push(VisualRow {
            start: line_start,
            end: line_end,
        });
        return;
    }

    let mut row_start = line_start;
    let mut row_width = 0usize;
    for (offset, character) in buffer[line_start..line_end].char_indices() {
        let index = line_start + offset;
        let width = char_width(character);
        if row_width > 0 && row_width + width > content_width {
            rows.push(VisualRow {
                start: row_start,
                end: index,
            });
            row_start = index;
            row_width = 0;
        }
        row_width += width;
    }

    rows.push(VisualRow {
        start: row_start,
        end: line_end,
    });
}

fn row_for_cursor(rows: &[VisualRow], cursor: usize) -> usize {
    rows.iter()
        .enumerate()
        .take_while(|(_, row)| row.start <= cursor)
        .map(|(index, _)| index)
        .last()
        .unwrap_or(0)
}

fn cursor_at_column(buffer: &str, row: &VisualRow, target_column: usize) -> usize {
    let mut column = 0usize;
    for (offset, character) in buffer[row.start..row.end].char_indices() {
        let width = char_width(character);
        if column + width > target_column {
            return row.start + offset;
        }
        column += width;
    }

    row.end
}

fn display_width(value: &str) -> usize {
    value.chars().map(char_width).sum()
}

fn char_width(character: char) -> usize {
    if character == '\t' {
        return 4;
    }

    UnicodeWidthChar::width(character).unwrap_or(0)
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

fn line_start(buffer: &str, cursor: usize) -> usize {
    buffer[..cursor]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn line_end(buffer: &str, cursor: usize) -> usize {
    buffer[cursor..]
        .find('\n')
        .map(|index| cursor + index)
        .unwrap_or(buffer.len())
}

fn previous_word_boundary(buffer: &str, cursor: usize) -> usize {
    let mut cursor = cursor;
    cursor = move_while_previous(buffer, cursor, |character| character.is_whitespace());
    let Some(category) = previous_character(buffer, cursor).map(word_category) else {
        return cursor;
    };

    move_while_previous(buffer, cursor, |character| {
        word_category(character) == category
    })
}

fn next_word_boundary(buffer: &str, cursor: usize) -> usize {
    let mut cursor = cursor;
    cursor = move_while_next(buffer, cursor, |character| character.is_whitespace());
    let Some(category) = next_character(buffer, cursor).map(word_category) else {
        return cursor;
    };

    move_while_next(buffer, cursor, |character| {
        word_category(character) == category
    })
}

fn move_while_previous(buffer: &str, mut cursor: usize, predicate: impl Fn(char) -> bool) -> usize {
    while let Some(character) = previous_character(buffer, cursor) {
        if !predicate(character) {
            break;
        }
        cursor = previous_boundary(buffer, cursor);
    }
    cursor
}

fn move_while_next(buffer: &str, mut cursor: usize, predicate: impl Fn(char) -> bool) -> usize {
    while let Some(character) = next_character(buffer, cursor) {
        if !predicate(character) {
            break;
        }
        cursor = next_boundary(buffer, cursor);
    }
    cursor
}

fn previous_character(buffer: &str, cursor: usize) -> Option<char> {
    if cursor == 0 {
        return None;
    }

    buffer[..cursor].chars().next_back()
}

fn next_character(buffer: &str, cursor: usize) -> Option<char> {
    if cursor >= buffer.len() {
        return None;
    }

    buffer[cursor..].chars().next()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum WordCategory {
    Word,
    Separator,
}

fn word_category(character: char) -> WordCategory {
    if character.is_alphanumeric() || character == '_' {
        return WordCategory::Word;
    }

    WordCategory::Separator
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

fn clamp_to_boundary(buffer: &str, cursor: usize) -> usize {
    let mut cursor = cursor.min(buffer.len());
    while !buffer.is_char_boundary(cursor) {
        cursor = cursor.saturating_sub(1);
    }
    cursor
}

fn normalize_paste(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

fn is_key_edit_event(key: KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn is_submit_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Enter && key.modifiers == KeyModifiers::NONE
}

fn is_unmodified_line_break_char(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('\n' | '\r')) && key.modifiers == KeyModifiers::NONE
}

fn is_newline_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::ALT)
            || key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SHIFT))
        || is_ctrl_char(key, 'j')
}

fn moves_by_word(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::ALT) || key.modifiers.contains(KeyModifiers::CONTROL)
}

fn selects_text(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT)
}

fn is_ctrl_char(key: KeyEvent, target: char) -> bool {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&target))
}

fn should_insert_char(key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }

    if key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    matches!(key.code, KeyCode::Char(character) if !character.is_control())
}

fn is_plain_paste_candidate_key(key: KeyEvent) -> bool {
    key.modifiers == KeyModifiers::NONE
}

fn saturating_u16(value: usize) -> u16 {
    value.min(usize::from(u16::MAX)) as u16
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
    fn suggestions_hide_after_newline() {
        assert_eq!(suggestion_query("/history\n2", 9), None);
    }

    #[test]
    fn suggestions_can_show_when_cursor_is_still_in_command_name() {
        assert_eq!(suggestion_query("/history 2", 4), Some("his"));
    }

    #[test]
    fn cursor_boundaries_handle_utf8() {
        let text = "a\u{00e9}/";
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

    #[test]
    fn prompt_state_inserts_multiline_text() {
        let mut state = state_with("hello");

        state.insert_str("\nworld");

        assert_eq!(state.buffer, "hello\nworld");
        assert_eq!(state.cursor, state.buffer.len());
    }

    #[test]
    fn prompt_state_replaces_selection_on_insert() {
        let mut state = state_with("hello");
        state.cursor = 1;
        state.move_right(true);
        state.move_right(true);

        state.insert_char('X');

        assert_eq!(state.buffer, "hXlo");
        assert_eq!(state.cursor, 2);
        assert_eq!(state.selection_range(), None);
    }

    #[test]
    fn prompt_state_deletes_selection_on_backspace() {
        let mut state = state_with("hello");
        state.cursor = 1;
        state.move_right(true);
        state.move_right(true);

        state.backspace();

        assert_eq!(state.buffer, "hlo");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn prompt_state_select_all_replaces_buffer() {
        let mut state = state_with("hello");

        state.select_all();
        state.insert_str("bye");

        assert_eq!(state.buffer, "bye");
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn prompt_state_home_and_end_use_current_line() {
        let mut state = state_with("one\ntwo\nthree");
        state.cursor = "one\ntwo".len();

        state.move_line_start(false);
        assert_eq!(state.cursor, "one\n".len());

        state.move_line_end(false);
        assert_eq!(state.cursor, "one\ntwo".len());
    }

    #[test]
    fn prompt_state_ctrl_home_and_end_use_whole_buffer() {
        let mut state = state_with("one\ntwo\nthree");
        state.cursor = "one\ntwo".len();

        state.move_start(false);
        assert_eq!(state.cursor, 0);

        state.move_end(false);
        assert_eq!(state.cursor, state.buffer.len());
    }

    #[test]
    fn prompt_state_vertical_movement_uses_visual_rows() {
        let mut state = state_with("abcd");

        state.move_visual_up(false, 2);
        assert_eq!(state.cursor, 2);

        state.move_visual_down(false, 2);
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn prompt_state_vertical_selection_extends_anchor() {
        let mut state = state_with("abcd");

        state.move_visual_up(true, 2);

        assert_eq!(state.selection_range(), Some(2..4));
    }

    #[test]
    fn prompt_state_deletes_previous_word() {
        let mut state = state_with("hello world");

        state.delete_previous_word();

        assert_eq!(state.buffer, "hello ");
        assert_eq!(state.cursor, "hello ".len());
    }

    #[test]
    fn prompt_state_kill_and_yank_round_trip_current_line() {
        let mut state = state_with("one\ntwo");

        state.kill_to_line_start();
        assert_eq!(state.buffer, "one\n");
        assert_eq!(state.kill_buffer, "two");

        state.yank();
        assert_eq!(state.buffer, "one\ntwo");
    }

    #[test]
    fn visual_rows_wrap_and_preserve_empty_lines() {
        let state = state_with("ab\n\ncde\n");

        assert_eq!(
            state.visual_rows(2),
            vec![
                VisualRow { start: 0, end: 2 },
                VisualRow { start: 3, end: 3 },
                VisualRow { start: 4, end: 6 },
                VisualRow { start: 6, end: 7 },
                VisualRow { start: 8, end: 8 },
            ]
        );
    }

    #[test]
    fn paste_normalization_uses_lf() {
        assert_eq!(normalize_paste("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn bracketed_paste_normalizes_crlf_without_submitting() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let mut editor = PromptEditor::new(&renderer, &registry);

        let action = editor
            .handle_event(Event::Paste("a\r\nb".to_owned()))
            .unwrap();

        assert!(matches!(action, PromptAction::Continue));
        assert_eq!(editor.state.buffer, "a\nb");
    }

    #[test]
    fn unbracketed_crlf_paste_keeps_line_breaks_without_submit() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let mut editor = PromptEditor::new(&renderer, &registry);
        let pasted =
            "error: variants `PowerShell` and `Cmd` are never constructed\r\n\r\nError: build failed";
        let mut now = Instant::now();

        for key in unbracketed_paste_keys(pasted) {
            let action = editor.handle_key_with_time(key, now).unwrap();
            assert!(matches!(
                action,
                PromptAction::Noop | PromptAction::Continue
            ));
            now += std::time::Duration::from_millis(1);
        }

        let flush_at = now
            + PasteBurst::recommended_active_flush_delay()
            + std::time::Duration::from_millis(1);
        assert!(editor.flush_paste_burst_if_due(flush_at));
        assert_eq!(editor.state.buffer, normalize_paste(pasted));
    }

    #[test]
    fn plain_enter_submits_after_pending_character_flushes() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let mut editor = PromptEditor::new(&renderer, &registry);
        let now = Instant::now();

        let action = editor
            .handle_key_with_time(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), now)
            .unwrap();
        assert!(matches!(action, PromptAction::Noop));
        assert_eq!(editor.state.buffer, "");

        let flush_at = now + PasteBurst::recommended_flush_delay();
        assert!(editor.flush_paste_burst_if_due(flush_at));
        assert_eq!(editor.state.buffer, "h");

        let action = editor
            .handle_key_with_time(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                flush_at + std::time::Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(action, PromptAction::Submit));
    }

    #[test]
    fn unbracketed_paste_does_not_redraw_until_flush() {
        let renderer = Renderer::default();
        let registry = Arc::new(test_registry());
        let mut editor = PromptEditor::new(&renderer, &registry);
        let now = Instant::now();

        let first = editor
            .handle_key_with_time(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), now)
            .unwrap();
        let second = editor
            .handle_key_with_time(
                KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
                now + std::time::Duration::from_millis(1),
            )
            .unwrap();
        let newline = editor
            .handle_key_with_time(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                now + std::time::Duration::from_millis(2),
            )
            .unwrap();

        assert!(matches!(first, PromptAction::Noop));
        assert!(matches!(second, PromptAction::Noop));
        assert!(matches!(newline, PromptAction::Noop));
        assert_eq!(editor.state.buffer, "");

        let flush_at = now
            + std::time::Duration::from_millis(2)
            + PasteBurst::recommended_active_flush_delay()
            + std::time::Duration::from_millis(1);
        assert!(editor.flush_paste_burst_if_due(flush_at));
        assert_eq!(editor.state.buffer, "ab\n");
    }

    fn state_with(value: &str) -> PromptState {
        PromptState {
            buffer: value.to_owned(),
            cursor: value.len(),
            selection_anchor: None,
            selected: 0,
            preferred_column: None,
            kill_buffer: String::new(),
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

    fn unbracketed_paste_keys(value: &str) -> Vec<KeyEvent> {
        let mut keys = Vec::new();
        let mut chars = value.chars().peekable();
        while let Some(character) = chars.next() {
            match character {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    keys.push(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
                '\n' => keys.push(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                character => keys.push(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)),
            }
        }
        keys
    }
}
