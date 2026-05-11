impl<'a, C> PromptEditor<'a, C> {
    pub fn new(
        renderer: &'a Renderer,
        registry: &'a Arc<CommandRegistry<C>>,
        completions: &'a PromptCompletionCatalog<'a>,
    ) -> Self {
        Self {
            renderer,
            registry,
            completions,
            footer: None,
            state: PromptState::default(),
            terminal: PromptTerminal,
            rendered_lines: 0,
            rendered_cursor_row: 0,
            paste_burst: PasteBurst::default(),
        }
    }

    /// Adds contextual footer data to render below the active prompt input.
    pub fn with_footer(mut self, footer: ChatPromptFooterModel) -> Self {
        self.footer = Some(footer);
        self
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
                if !self
                    .state
                    .dismiss_suggestions(self.registry, self.completions)
                {
                    self.state.escape();
                }
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
                if !self.accept_selected_suggestion()
                    && !self.guide_command_before_submit_or_advance()
                    && !self.is_slash_context()
                {
                    self.state.insert_char('\t');
                }
                self.paste_burst.clear_window_after_non_char();
                Ok(PromptAction::Continue)
            }
            KeyCode::Char(' ')
                if key.modifiers == KeyModifiers::NONE && self.is_slash_context() =>
            {
                self.flush_paste_burst_before_modified_input();
                if !self.accept_selected_suggestion()
                    && !self.guide_command_before_submit_or_advance()
                {
                    self.state.insert_char(' ');
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
            if self.accept_selected_suggestion() || self.guide_command_before_submit_or_advance() {
                self.paste_burst.clear_window_after_non_char();
                return PromptAction::Continue;
            }
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
        let guidance = self.guidance();
        self.state.clamp_selection(suggestions.len());

        let content_width = self.content_width();
        let rows = self.state.visual_rows(content_width);
        let cursor_position = self.state.cursor_position(content_width, &rows);

        self.clear_rendered_block()?;
        self.terminal
            .render_prompt_rows(self.renderer, &self.state, &rows);

        if let Some(footer) = &self.footer {
            println!();
            println!();
            print!("{}", crate::chat::renderer::format_prompt_footer(footer));
        }

        for line in &guidance {
            println!();
            print!("{}", render_guidance_line(line));
        }

        if !guidance.is_empty() && !suggestions.is_empty() {
            println!();
        }

        for (index, suggestion) in suggestions.iter().enumerate() {
            println!();
            print!(
                "{}",
                suggestion_row(suggestion, index == self.state.selected)
            );
        }

        self.rendered_lines = saturating_u16(
            rows.len()
                + footer_rendered_lines(self.footer.as_ref())
                + guidance.len()
                + guidance_suggestion_gap(&guidance, &suggestions)
                + suggestions.len(),
        );
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

    /// Applies the selected picker item when command suggestions are visible.
    fn accept_selected_suggestion(&mut self) -> bool {
        let Some(suggestion) = self.suggestions().get(self.state.selected).cloned() else {
            return false;
        };
        if suggestion.kind == PromptSuggestionKind::Info {
            return false;
        }

        self.state.complete_suggestion(&suggestion);
        if matches!(
            suggestion.kind,
            PromptSuggestionKind::Subcommand | PromptSuggestionKind::Value
        ) {
            self.state.guide_command_field(self.completions);
        }
        true
    }

    /// Moves the command composer to the next required or invalid field.
    fn guide_command_before_submit_or_advance(&mut self) -> bool {
        self.state.guide_command_field(self.completions)
    }

    fn suggestions(&self) -> Vec<PromptSuggestion> {
        self.state.suggestions(self.registry, self.completions)
    }

    /// Returns help rows for the command field currently being composed.
    fn guidance(&self) -> Vec<PromptGuidanceLine> {
        prompt_guidance(&self.state.buffer, self.state.cursor, self.completions)
    }

    fn content_width(&self) -> usize {
        let terminal_width = terminal::size()
            .map(|(width, _)| width)
            .unwrap_or(DEFAULT_TERMINAL_WIDTH);
        usize::from(terminal_width.saturating_sub(PROMPT_WIDTH).max(1))
    }
}
