/// Request data for rendering a command-side option selection prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPromptRequest {
    pub title: String,
    pub description: String,
    pub options: Vec<String>,
    pub allow_custom: bool,
    pub allow_comment: bool,
}

impl SelectionPromptRequest {
    /// Creates a selection prompt request with static options and no optional text fields.
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            options,
            allow_custom: false,
            allow_comment: false,
        }
    }

    /// Configures optional custom text input and comment entry.
    pub fn with_inputs(mut self, allow_custom: bool, allow_comment: bool) -> Self {
        self.allow_custom = allow_custom;
        self.allow_comment = allow_comment;
        self
    }
}

/// User-provided answer returned by the command-side selection prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPromptAnswer {
    pub choice: SelectionPromptChoice,
    pub comment: Option<String>,
}

/// Selected predefined option or custom free-text value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionPromptChoice {
    Option { index: usize, label: String },
    Custom(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectionInputMode {
    Options,
    Comment,
}

enum SelectionAction {
    Continue,
    Submit,
    Exit,
}

/// Interactive terminal UI for selecting one option and optional comment text.
pub struct SelectionPrompt {
    request: SelectionPromptRequest,
    footer: Option<ChatPromptFooterModel>,
    selected: usize,
    custom_input: String,
    custom_cursor: usize,
    comment: String,
    comment_cursor: usize,
    input_mode: SelectionInputMode,
    rendered_lines: u16,
    rendered_cursor_row: u16,
}

impl SelectionPrompt {
    /// Creates a selection prompt while preserving the shared renderer seam for commands.
    pub fn new(_renderer: &Renderer, request: SelectionPromptRequest) -> Self {
        Self {
            request,
            footer: None,
            selected: 0,
            custom_input: String::new(),
            custom_cursor: 0,
            comment: String::new(),
            comment_cursor: 0,
            input_mode: SelectionInputMode::Options,
            rendered_lines: 0,
            rendered_cursor_row: 0,
        }
    }

    /// Adds contextual footer data rendered below the selection prompt.
    pub fn with_footer(mut self, footer: ChatPromptFooterModel) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Reads the selected option, optional custom text, and optional comment from the terminal.
    pub fn read_selection(mut self) -> Result<SelectionPromptAnswer, ChatError> {
        if self.choice_count() == 0 {
            return Err(ChatError::Session(
                "selection prompt requires an option or custom input".to_owned(),
            ));
        }

        let _raw_mode = RawModeGuard::enter()?;
        self.redraw()?;

        loop {
            let action = self.read_next_action()?;
            if let Some(answer) = self.apply_action(action)? {
                return answer;
            }
        }
    }

    /// Applies a prompt action after key handling.
    fn apply_action(
        &mut self,
        action: SelectionAction,
    ) -> Result<Option<Result<SelectionPromptAnswer, ChatError>>, ChatError> {
        match action {
            SelectionAction::Continue => self.redraw()?,
            SelectionAction::Submit => {
                let answer = self.answer();
                self.clear_rendered_block()?;
                return Ok(Some(answer));
            }
            SelectionAction::Exit => {
                self.clear_rendered_block()?;
                return Ok(Some(Err(ChatError::Exit)));
            }
        }
        Ok(None)
    }

    /// Reads the next terminal event and converts it into a selection prompt action.
    fn read_next_action(&mut self) -> Result<SelectionAction, ChatError> {
        loop {
            let event = event::read().map_err(ChatError::Io)?;
            if let Event::Key(key) = event {
                if is_key_edit_event(key) {
                    return self.handle_key(key);
                }
            }
        }
    }

    /// Applies one key event to the option/comment state machine.
    fn handle_key(&mut self, key: KeyEvent) -> Result<SelectionAction, ChatError> {
        if is_ctrl_char(key, 'c') {
            return Ok(SelectionAction::Exit);
        }

        if is_submit_key(key) || is_unmodified_line_break_char(key) {
            return Ok(SelectionAction::Submit);
        }

        if key.code == KeyCode::Esc {
            return Ok(self.handle_escape());
        }

        if self.request.allow_comment && key.code == KeyCode::Tab {
            self.toggle_input_mode();
            return Ok(SelectionAction::Continue);
        }

        match self.input_mode {
            SelectionInputMode::Options => self.handle_option_key(key),
            SelectionInputMode::Comment => self.handle_comment_key(key),
        }
    }

    /// Handles Escape by leaving comment mode first and exiting otherwise.
    fn handle_escape(&mut self) -> SelectionAction {
        if self.input_mode == SelectionInputMode::Comment {
            self.input_mode = SelectionInputMode::Options;
            return SelectionAction::Continue;
        }

        SelectionAction::Exit
    }

    /// Handles navigation and custom text edits while the option list is active.
    fn handle_option_key(&mut self, key: KeyEvent) -> Result<SelectionAction, ChatError> {
        match key.code {
            KeyCode::Up => self.select_previous(),
            KeyCode::Down => self.select_next(),
            KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => self.select_previous(),
            KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => self.select_next(),
            KeyCode::Left if self.is_custom_selected() => {
                self.custom_cursor = previous_boundary(&self.custom_input, self.custom_cursor);
            }
            KeyCode::Right if self.is_custom_selected() => {
                self.custom_cursor = next_boundary(&self.custom_input, self.custom_cursor);
            }
            KeyCode::Home if self.is_custom_selected() => self.custom_cursor = 0,
            KeyCode::End if self.is_custom_selected() => self.custom_cursor = self.custom_input.len(),
            KeyCode::Backspace if self.is_custom_selected() => {
                delete_previous_character(&mut self.custom_input, &mut self.custom_cursor);
            }
            KeyCode::Delete if self.is_custom_selected() => {
                delete_next_character(&mut self.custom_input, self.custom_cursor);
            }
            KeyCode::Char(character) if should_insert_char(key) && self.request.allow_custom => {
                self.select_custom();
                insert_character(&mut self.custom_input, &mut self.custom_cursor, character);
            }
            _ => {}
        }

        Ok(SelectionAction::Continue)
    }

    /// Handles editable comment text while comment mode is active.
    fn handle_comment_key(&mut self, key: KeyEvent) -> Result<SelectionAction, ChatError> {
        match key.code {
            KeyCode::Left => self.comment_cursor = previous_boundary(&self.comment, self.comment_cursor),
            KeyCode::Right => self.comment_cursor = next_boundary(&self.comment, self.comment_cursor),
            KeyCode::Home => self.comment_cursor = 0,
            KeyCode::End => self.comment_cursor = self.comment.len(),
            KeyCode::Backspace => delete_previous_character(&mut self.comment, &mut self.comment_cursor),
            KeyCode::Delete => delete_next_character(&mut self.comment, self.comment_cursor),
            KeyCode::Char(character) if should_insert_char(key) => {
                insert_character(&mut self.comment, &mut self.comment_cursor, character);
            }
            _ => {}
        }

        Ok(SelectionAction::Continue)
    }

    /// Renders the full selection prompt and moves the cursor to editable text only.
    fn redraw(&mut self) -> Result<(), ChatError> {
        self.clear_rendered_block()?;

        let lines = self.render_lines();
        print!("{}", lines.join("\n"));

        let cursor_position = self.editable_cursor_position(&lines);
        self.rendered_lines = saturating_u16(lines.len());
        self.rendered_cursor_row = cursor_position
            .map(|position| position.row)
            .unwrap_or_else(|| self.rendered_lines.saturating_sub(1));
        self.set_cursor_for_position(cursor_position)?;
        io::stdout().flush().map_err(ChatError::Io)
    }

    /// Builds the display lines for the current prompt state.
    fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(paint(title_style(), &self.request.title));
        if !self.request.description.trim().is_empty() {
            lines.extend(self.request.description.lines().map(str::to_owned));
        }
        lines.push(String::new());

        for (index, option) in self.request.options.iter().enumerate() {
            lines.push(self.render_option_line(index, option));
        }

        if self.request.allow_custom {
            lines.push(self.render_custom_line());
        }

        if self.request.allow_comment {
            lines.push(String::new());
            lines.push(self.render_comment_line());
        }

        if let Some(footer) = &self.footer {
            lines.push(String::new());
            lines.push(crate::chat::renderer::format_prompt_footer(footer));
        }

        lines
    }

    /// Renders one predefined option row with selection highlighting.
    fn render_option_line(&self, index: usize, option: &str) -> String {
        let marker = if self.selected == index { ">" } else { " " };
        let label = format!("{}. {option}", option_letter(index));
        if self.selected == index {
            return format!("{marker} {}", paint(title_style(), label));
        }

        format!("{marker} {label}")
    }

    /// Renders the custom free-text option row and current custom input.
    fn render_custom_line(&self) -> String {
        let index = self.request.options.len();
        let marker = if self.selected == index { ">" } else { " " };
        let label = format!("{}. ", option_letter(index));
        let value = if self.custom_input.is_empty() {
            paint(dim_style(), "Type your option here")
        } else {
            self.custom_input.clone()
        };

        if self.selected == index {
            return format!("{marker} {}{value}", paint(title_style(), label));
        }

        format!("{marker} {}", paint(dim_style(), format!("{label}{value}")))
    }

    /// Renders the comment row or the Tab hint when comment mode is inactive.
    fn render_comment_line(&self) -> String {
        if self.input_mode == SelectionInputMode::Comment {
            let value = if self.comment.is_empty() {
                paint(dim_style(), "Add a comment")
            } else {
                self.comment.clone()
            };
            return format!("  comment: {value}");
        }

        let selected = self.selected_label();
        paint(
            dim_style(),
            format!("  Press Tab to add a comment on {selected}."),
        )
    }

    /// Computes the terminal cursor row and column for editable text after a redraw.
    fn editable_cursor_position(&self, lines: &[String]) -> Option<SelectionCursorPosition> {
        if self.input_mode == SelectionInputMode::Comment {
            return Some(SelectionCursorPosition {
                row: saturating_u16(self.comment_row(lines)),
                column: saturating_u16(11 + display_width(&self.comment[..self.comment_cursor])),
            });
        }

        if self.is_custom_selected() {
            return Some(SelectionCursorPosition {
                row: saturating_u16(self.option_start_row() + self.request.options.len()),
                column: saturating_u16(5 + display_width(&self.custom_input[..self.custom_cursor])),
            });
        }

        None
    }

    /// Returns the rendered row index where selectable options begin.
    fn option_start_row(&self) -> usize {
        let description_rows = if self.request.description.trim().is_empty() {
            0
        } else {
            self.request.description.lines().count()
        };

        2 + description_rows
    }

    /// Returns the rendered row index for the editable comment line.
    fn comment_row(&self, lines: &[String]) -> usize {
        if self.footer.is_some() {
            return lines.len().saturating_sub(4);
        }

        lines.len().saturating_sub(1)
    }

    /// Clears all rows rendered by the selection prompt.
    fn clear_rendered_block(&mut self) -> Result<(), ChatError> {
        clear_block(self.rendered_lines, self.rendered_cursor_row)?;
        self.rendered_lines = 0;
        self.rendered_cursor_row = 0;
        Ok(())
    }

    /// Hides the cursor for static options or moves it to editable text when available.
    fn set_cursor_for_position(
        &self,
        position: Option<SelectionCursorPosition>,
    ) -> Result<(), ChatError> {
        let Some(position) = position else {
            let mut stdout = io::stdout();
            queue!(stdout, Hide).map_err(ChatError::Io)?;
            return stdout.flush().map_err(ChatError::Io);
        };

        let mut stdout = io::stdout();
        queue!(stdout, Show).map_err(ChatError::Io)?;
        let last_row = self.rendered_lines.saturating_sub(1);
        if last_row > position.row {
            queue!(stdout, MoveUp(last_row - position.row)).map_err(ChatError::Io)?;
        }
        queue!(stdout, MoveToColumn(position.column)).map_err(ChatError::Io)?;
        stdout.flush().map_err(ChatError::Io)
    }

    /// Returns the current answer, validating custom text when selected.
    fn answer(&self) -> Result<SelectionPromptAnswer, ChatError> {
        let choice = if self.is_custom_selected() {
            let value = self.custom_input.trim().to_owned();
            if value.is_empty() {
                return Err(ChatError::Session("custom selection cannot be empty".to_owned()));
            }
            SelectionPromptChoice::Custom(value)
        } else {
            SelectionPromptChoice::Option {
                index: self.selected,
                label: self.request.options[self.selected].clone(),
            }
        };

        Ok(SelectionPromptAnswer {
            choice,
            comment: non_empty_trimmed(&self.comment),
        })
    }

    /// Moves selection to the previous option with wrapping.
    fn select_previous(&mut self) {
        let count = self.choice_count();
        self.selected = if self.selected == 0 {
            count.saturating_sub(1)
        } else {
            self.selected - 1
        };
    }

    /// Moves selection to the next option with wrapping.
    fn select_next(&mut self) {
        self.selected = (self.selected + 1) % self.choice_count();
    }
    /// Moves selection to the custom option when it is available.
    fn select_custom(&mut self) {
        if self.request.allow_custom {
            self.selected = self.request.options.len();
        }
    }
    /// Toggles between option navigation and comment editing modes.
    fn toggle_input_mode(&mut self) {
        self.input_mode = match self.input_mode {
            SelectionInputMode::Options => SelectionInputMode::Comment,
            SelectionInputMode::Comment => SelectionInputMode::Options,
        };
    }
    /// Returns the number of selectable rows including the optional custom row.
    fn choice_count(&self) -> usize {
        self.request.options.len() + usize::from(self.request.allow_custom)
    }
    /// Returns whether the optional custom free-text row is selected.
    fn is_custom_selected(&self) -> bool {
        self.request.allow_custom && self.selected == self.request.options.len()
    }
    /// Returns the visible label for the currently selected row.
    fn selected_label(&self) -> String {
        if self.is_custom_selected() {
            if self.custom_input.trim().is_empty() {
                return "custom option".to_owned();
            }
            return self.custom_input.trim().to_owned();
        }
        self.request.options[self.selected].clone()
    }
}

include!("selection_helpers.rs");
