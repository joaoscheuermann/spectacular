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
