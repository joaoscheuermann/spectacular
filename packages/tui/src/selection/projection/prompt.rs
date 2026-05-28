use super::builder::{ProjectedRowSpec, ProjectionBuilder};
use crate::components::{prompt_render_lines_with_width, selection_prompt_render_lines};
use crate::selection::ranges::selectable_range;
use crate::selection::{line_width, SelectableSource, SelectableSurface, PROMPT_MARKER_WIDTH};
use crate::state::State;
use std::ops::Range;

pub(super) fn append_area(builder: &mut ProjectionBuilder, state: &State, width: u16) {
    if state.selection.is_some() {
        append_selection_prompt(builder, state);
        return;
    }

    append_prompt(builder, state, width);
}

fn append_selection_prompt(builder: &mut ProjectionBuilder, state: &State) {
    let selection_lines = selection_prompt_render_lines(state);
    let selection_line_count = selection_lines.len();
    for (line, render_line) in selection_lines.into_iter().enumerate() {
        let width = line_width(&render_line);
        builder.push_row(ProjectedRowSpec {
            surface: SelectableSurface::SelectionPrompt,
            logical_index: line,
            source: SelectableSource::SelectionPrompt { line },
            text: render_line.plain_text(),
            source_columns: 0..width,
            source_width: width,
            selectable_columns: 0..builder.screen_width(),
            excluded_columns: Vec::new(),
        });
    }
    builder.push_blank(
        SelectableSurface::SelectionPrompt,
        selection_line_count,
        SelectableSource::SelectionPrompt {
            line: selection_line_count,
        },
    );
}

fn append_prompt(builder: &mut ProjectionBuilder, state: &State, width: u16) {
    let prompt_selection = prompt_row_selection_metadata(state, width, builder.screen_width());
    let prompt_lines = prompt_render_lines_with_width(state, Some(width));
    let prompt_line_count = prompt_lines.len();

    for (line, render_line) in prompt_lines.into_iter().enumerate() {
        let width = line_width(&render_line);
        let metadata = prompt_selection
            .get(line)
            .cloned()
            .unwrap_or_else(PromptRowSelection::empty);
        builder.push_row(ProjectedRowSpec {
            surface: SelectableSurface::Prompt,
            logical_index: line,
            source: SelectableSource::Prompt { line },
            text: render_line.plain_text(),
            source_columns: 0..width,
            source_width: width,
            selectable_columns: metadata.selectable_columns,
            excluded_columns: metadata.excluded_columns,
        });
    }
    builder.push_blank(
        SelectableSurface::Prompt,
        prompt_line_count,
        SelectableSource::Prompt {
            line: prompt_line_count,
        },
    );
}

fn prompt_row_selection_metadata(
    state: &State,
    width: u16,
    screen_width: usize,
) -> Vec<PromptRowSelection> {
    let prompt = &state.session.prompt;
    if prompt.is_empty() {
        return vec![PromptRowSelection::empty()];
    }

    let text = prompt.text();
    let rows = crate::prompt::layout::visual_rows(&text, prompt_content_width(width));
    let cursor_row = crate::prompt::layout::row_for_cursor(&rows, prompt.cursor);
    let selectable_columns = selectable_range(PROMPT_MARKER_WIDTH, screen_width);
    rows.into_iter()
        .enumerate()
        .skip(prompt.scroll_top_row)
        .map(|(index, row)| {
            let mut excluded_columns = Vec::new();
            if index == cursor_row && prompt.cursor == row.end {
                let cursor_column = PROMPT_MARKER_WIDTH.saturating_add(
                    crate::prompt::layout::display_width(&text[row.start..row.end]),
                );
                excluded_columns.push(cursor_column..cursor_column.saturating_add(1));
            }

            PromptRowSelection {
                selectable_columns: selectable_columns.clone(),
                excluded_columns,
            }
        })
        .collect()
}

fn prompt_content_width(width: u16) -> usize {
    usize::from(width)
        .saturating_sub(PROMPT_MARKER_WIDTH)
        .max(1)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromptRowSelection {
    selectable_columns: Range<usize>,
    excluded_columns: Vec<Range<usize>>,
}

impl PromptRowSelection {
    fn empty() -> Self {
        Self {
            selectable_columns: 0..0,
            excluded_columns: Vec::new(),
        }
    }
}
