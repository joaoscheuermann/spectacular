#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectionCursorPosition {
    row: u16,
    column: u16,
}

/// Converts a zero-based option index into the visible option letter.
fn option_letter(index: usize) -> char {
    char::from(b'a' + (index.min(25) as u8))
}

/// Trims text and returns it only when it contains visible content.
fn non_empty_trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_owned())
}

/// Inserts one character at the active cursor byte offset.
fn insert_character(buffer: &mut String, cursor: &mut usize, character: char) {
    buffer.insert(*cursor, character);
    *cursor += character.len_utf8();
}

/// Deletes the character before the active cursor byte offset.
fn delete_previous_character(buffer: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let previous = previous_boundary(buffer, *cursor);
    buffer.replace_range(previous..*cursor, "");
    *cursor = previous;
}

/// Deletes the character after the active cursor byte offset.
fn delete_next_character(buffer: &mut String, cursor: usize) {
    if cursor >= buffer.len() {
        return;
    }
    let next = next_boundary(buffer, cursor);
    buffer.replace_range(cursor..next, "");
}

/// Clears a previously rendered prompt block and returns the cursor to the first row.
fn clear_block(rendered_lines: u16, cursor_row: u16) -> Result<(), ChatError> {
    if rendered_lines == 0 {
        return Ok(());
    }
    let mut stdout = io::stdout();
    queue!(stdout, Show).map_err(ChatError::Io)?;
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
