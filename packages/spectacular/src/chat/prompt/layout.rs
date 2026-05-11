fn suggestion_row(suggestion: &PromptSuggestion, selected: bool) -> String {
    let text = if suggestion.summary.is_empty() {
        format!("  {}", suggestion.label)
    } else {
        format!("  {:<18} {}", suggestion.label, suggestion.summary)
    };
    if suggestion.kind == PromptSuggestionKind::Info {
        return paint(dim_style(), text);
    }

    if selected {
        return paint(user_style(), text);
    }

    paint(dim_style(), text)
}

/// Renders command guidance with missing-field emphasis and normal dim details.
fn render_guidance_line(line: &PromptGuidanceLine) -> String {
    match line {
        PromptGuidanceLine::Missing(fields) => format!(
            "  {}{}",
            paint(missing_label_style(), "missing"),
            paint(missing_value_style(), format!(": {}.", fields.join(", ")))
        ),
        PromptGuidanceLine::Detail(text) => paint(dim_style(), format!("  {text}")),
        PromptGuidanceLine::Info(text) => paint(missing_value_style(), format!("  {text}")),
    }
}

/// Adds one visual spacer row between guidance and picker options.
fn guidance_suggestion_gap(
    guidance: &[PromptGuidanceLine],
    suggestions: &[PromptSuggestion],
) -> usize {
    usize::from(!guidance.is_empty() && !suggestions.is_empty())
}

/// Counts the spacer and footer rows rendered below the editable prompt.
fn footer_rendered_lines(footer: Option<&ChatPromptFooterModel>) -> usize {
    footer.map_or(0, |_| PROMPT_FOOTER_RENDERED_LINES)
}

/// Styles the missing-label prefix as dim orange and bold.
fn missing_label_style() -> anstyle::Style {
    MISSING_ORANGE.on_default().dimmed().bold()
}

/// Styles missing-field content as dim orange.
fn missing_value_style() -> anstyle::Style {
    MISSING_ORANGE.on_default().dimmed()
}

fn render_buffer_range(buffer: &str, range: Range<usize>, selection: Option<Range<usize>>) {
    if range.is_empty() {
        if selection
            .is_some_and(|selection| selection.start <= range.start && selection.end > range.start)
        {
            print!("{}", paint(selection_style(), " "));
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
        "{}",
        paint(
            selection_style(),
            display_text(&buffer[selected_start..selected_end])
        )
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
