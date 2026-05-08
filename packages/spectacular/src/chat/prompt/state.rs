impl PromptState {
    /// Performs the insert char operation for the editable prompt buffer.
    fn insert_char(&mut self, character: char) {
        self.insert_str(&character.to_string());
    }

    /// Performs the insert str operation for the editable prompt buffer.
    fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.delete_selection();
        self.buffer.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.after_edit();
    }

    /// Performs the backspace operation for the editable prompt buffer.
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

    /// Performs the delete operation for the editable prompt buffer.
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

    /// Performs the delete previous word operation for the editable prompt buffer.
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

    /// Performs the delete next word operation for the editable prompt buffer.
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

    /// Performs the kill to line start operation for the editable prompt buffer.
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

    /// Performs the kill to line end operation for the editable prompt buffer.
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

    /// Performs the yank operation for the editable prompt buffer.
    fn yank(&mut self) {
        if self.kill_buffer.is_empty() {
            return;
        }

        let value = self.kill_buffer.clone();
        self.insert_str(&value);
    }

    /// Performs the escape operation for the editable prompt buffer.
    fn escape(&mut self) {
        if self.selection_anchor.is_some() {
            self.clear_selection();
            return;
        }

        self.buffer.clear();
        self.cursor = 0;
        self.after_edit();
    }

    /// Performs the move left operation for the editable prompt buffer.
    fn move_left(&mut self, selecting: bool) {
        let cursor = previous_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Performs the move right operation for the editable prompt buffer.
    fn move_right(&mut self, selecting: bool) {
        let cursor = next_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Performs the move word left operation for the editable prompt buffer.
    fn move_word_left(&mut self, selecting: bool) {
        let cursor = previous_word_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Performs the move word right operation for the editable prompt buffer.
    fn move_word_right(&mut self, selecting: bool) {
        let cursor = next_word_boundary(&self.buffer, self.cursor);
        self.move_to(cursor, selecting);
    }

    /// Performs the move line start operation for the editable prompt buffer.
    fn move_line_start(&mut self, selecting: bool) {
        self.move_to(line_start(&self.buffer, self.cursor), selecting);
    }

    /// Performs the move line end operation for the editable prompt buffer.
    fn move_line_end(&mut self, selecting: bool) {
        self.move_to(line_end(&self.buffer, self.cursor), selecting);
    }

    /// Performs the move start operation for the editable prompt buffer.
    fn move_start(&mut self, selecting: bool) {
        self.move_to(0, selecting);
    }

    /// Performs the move end operation for the editable prompt buffer.
    fn move_end(&mut self, selecting: bool) {
        self.move_to(self.buffer.len(), selecting);
    }

    /// Performs the move visual up operation for the editable prompt buffer.
    fn move_visual_up(&mut self, selecting: bool, content_width: usize) {
        self.move_visual_line(-1, selecting, content_width);
    }

    /// Performs the move visual down operation for the editable prompt buffer.
    fn move_visual_down(&mut self, selecting: bool, content_width: usize) {
        self.move_visual_line(1, selecting, content_width);
    }

    /// Performs the select all operation for the editable prompt buffer.
    fn select_all(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        self.selection_anchor = Some(0);
        self.cursor = self.buffer.len();
        self.selected = 0;
        self.preferred_column = None;
    }

    /// Performs the select previous operation for the editable prompt buffer.
    fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Performs the select next operation for the editable prompt buffer.
    fn select_next(&mut self, suggestion_count: usize) {
        self.selected = (self.selected + 1).min(suggestion_count.saturating_sub(1));
    }

    /// Hides the active completion picker until the command context changes.
    fn dismiss_suggestions<C>(
        &mut self,
        registry: &CommandRegistry<C>,
        completions: &PromptCompletionCatalog<'_>,
    ) -> bool {
        let key = completion_context_key(&self.buffer, self.cursor);
        if key == self.dismissed_completion {
            return false;
        }

        if key.is_none()
            || prompt_suggestions(&self.buffer, self.cursor, registry, completions).is_empty()
        {
            return false;
        }

        self.dismissed_completion = key;
        self.selected = 0;
        true
    }

    /// Performs the clamp selection operation for the editable prompt buffer.
    fn clamp_selection(&mut self, suggestion_count: usize) {
        if self.selected >= suggestion_count {
            self.selected = 0;
        }
    }

    /// Performs the complete suggestion operation for the editable prompt buffer.
    fn complete_suggestion(&mut self, suggestion: &PromptSuggestion) {
        complete_suggestion(&mut self.buffer, &mut self.cursor, suggestion);
        self.clear_selection();
        self.selected = 0;
        self.preferred_column = None;
        self.dismissed_completion = None;
    }

    /// Advances the command composer to the next field that needs user input.
    fn guide_command_field(&mut self, completions: &PromptCompletionCatalog<'_>) -> bool {
        let Some(field) = command_field_needing_attention(&self.buffer, completions) else {
            return false;
        };

        focus_command_field(&mut self.buffer, &mut self.cursor, field.name);
        self.clear_selection();
        self.selected = 0;
        self.preferred_column = None;
        self.dismissed_completion = None;
        true
    }

    /// Performs the suggestions operation for the editable prompt buffer.
    fn suggestions<C>(
        &self,
        registry: &CommandRegistry<C>,
        completions: &PromptCompletionCatalog<'_>,
    ) -> Vec<PromptSuggestion> {
        if completion_context_key(&self.buffer, self.cursor) == self.dismissed_completion {
            return Vec::new();
        }

        prompt_suggestions(&self.buffer, self.cursor, registry, completions)
    }

    /// Performs the selection range operation for the editable prompt buffer.
    fn selection_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        if anchor == self.cursor {
            return None;
        }

        Some(anchor.min(self.cursor)..anchor.max(self.cursor))
    }

    /// Performs the visual rows operation for the editable prompt buffer.
    fn visual_rows(&self, content_width: usize) -> Vec<VisualRow> {
        visual_rows(&self.buffer, content_width)
    }

    /// Performs the cursor position operation for the editable prompt buffer.
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

    /// Performs the move visual line operation for the editable prompt buffer.
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

    /// Performs the move to operation for the editable prompt buffer.
    fn move_to(&mut self, cursor: usize, selecting: bool) {
        self.preferred_column = None;
        self.move_to_preserving_preferred_column(cursor, selecting);
    }

    /// Performs the move to preserving preferred column operation for the editable prompt buffer.
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

    /// Performs the delete selection operation for the editable prompt buffer.
    fn delete_selection(&mut self) -> bool {
        let Some(selection) = self.selection_range() else {
            return false;
        };

        self.buffer.replace_range(selection.clone(), "");
        self.cursor = selection.start;
        self.clear_selection();
        true
    }

    /// Performs the delete selection to kill buffer operation for the editable prompt buffer.
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

    /// Performs the clear selection operation for the editable prompt buffer.
    fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Performs the after edit operation for the editable prompt buffer.
    fn after_edit(&mut self) {
        self.clear_selection();
        self.selected = 0;
        self.preferred_column = None;
        self.dismissed_completion = None;
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
    /// Performs the enter operation for the editable prompt buffer.
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
    /// Performs the drop operation for the editable prompt buffer.
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = queue!(stdout, DisableBracketedPaste);
        let _ = stdout.flush();
        let _ = disable_raw_mode();
    }
}
