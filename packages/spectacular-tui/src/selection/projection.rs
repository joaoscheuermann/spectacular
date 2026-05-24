use crate::components::{
    input_notice_render_line, prompt_render_lines_with_width, selection_prompt_render_lines,
    transcript_item_render_lines, working_render_line, TranscriptLayout,
};
use crate::selection::ranges::{
    nearest_selectable_boundary, selectable_range, split_around_excluded,
};
use crate::selection::wrapping::{wrapped_text_rows, SourceProjectionRow};
use crate::selection::{
    line_width, normalized_points, screen_width, RenderedSelectionState, SelectableSource,
    SelectableSurface, SelectionPoint, ViewportEdge, PROMPT_MARKER_WIDTH,
};
use crate::state::State;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use std::ops::Range;

/// One visible row in the app-wide rendered-text projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectableRow {
    pub order: usize,
    pub screen_row: usize,
    pub logical_row: SelectionRowKey,
    pub surface: SelectableSurface,
    pub source: SelectableSource,
    pub text: String,
    pub source_columns: Range<usize>,
    pub source_width: usize,
    pub selectable_columns: Range<usize>,
    pub excluded_columns: Vec<Range<usize>>,
}

/// Stable row identity used to keep rendered selection anchored across scrolling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectionRowKey {
    pub surface: SelectableSurface,
    pub index: usize,
}

/// Complete visible selectable row projection for the current app frame.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectableProjection {
    pub(super) rows: Vec<SelectableRow>,
    transcript_rows: Range<usize>,
    transcript_content_width: usize,
}

impl SelectableProjection {
    /// Builds the app-wide selectable row projection from reducer-owned state.
    pub fn for_state(state: &State) -> Self {
        let width = screen_width(state);
        let screen_width = usize::from(width).max(1);
        let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
        let layout = TranscriptLayout::for_state(state, transcript_content_width);
        let visible_rows = transcript_visible_rows(state, layout.total_rows);
        let scroll_offset =
            scroll_offset_from_top(layout.total_rows, visible_rows, state.scroll.offset);

        Self::for_transcript_window(
            state,
            transcript_content_width,
            screen_width,
            &layout,
            scroll_offset..scroll_offset.saturating_add(visible_rows),
        )
    }

    /// Builds the bounded projection needed for semantic copy of the current selection.
    pub fn for_selection_copy(state: &State, selection: &RenderedSelectionState) -> Self {
        let width = screen_width(state);
        let screen_width = usize::from(width).max(1);
        let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
        let layout = TranscriptLayout::for_state(state, transcript_content_width);
        let window =
            transcript_selection_window(selection, layout.total_rows).unwrap_or_else(|| {
                let visible_rows = transcript_visible_rows(state, layout.total_rows);
                let scroll_offset =
                    scroll_offset_from_top(layout.total_rows, visible_rows, state.scroll.offset);
                scroll_offset..scroll_offset.saturating_add(visible_rows)
            });

        Self::for_transcript_window(
            state,
            transcript_content_width,
            screen_width,
            &layout,
            window,
        )
    }

    /// Builds a selectable projection from a precomputed transcript layout and visible row window.
    pub(crate) fn for_transcript_window(
        state: &State,
        transcript_content_width: usize,
        screen_width: usize,
        layout: &TranscriptLayout,
        transcript_window: Range<usize>,
    ) -> Self {
        let mut builder = ProjectionBuilder::new(screen_width);

        let transcript_start = builder.len();
        builder.append_transcript_rows(state, transcript_content_width, layout, transcript_window);
        let transcript_end = builder.len();

        let width = u16::try_from(screen_width).unwrap_or(u16::MAX);
        builder.append_working(state);
        builder.append_input_notice(state);
        builder.append_prompt_area(state, width);
        builder.append_footer(state, width);

        Self {
            rows: builder.finish(),
            transcript_rows: transcript_start..transcript_end,
            transcript_content_width,
        }
    }

    /// Returns projected rows in visible screen order.
    pub fn rows(&self) -> &[SelectableRow] {
        &self.rows
    }

    /// Maps a terminal coordinate to a selectable point, ignoring the transcript scrollbar column.
    pub fn point_at(&self, column: u16, row: u16) -> Option<SelectionPoint> {
        let projected = self.row_at_screen(usize::from(row))?;
        selectable_point_at(
            projected,
            usize::from(column),
            self.transcript_content_width,
        )
    }

    pub(super) fn focus_point_at(&self, column: u16, row: u16) -> Option<SelectionPoint> {
        let projected = self.row_at_screen(usize::from(row))?;
        clamped_point_at(
            projected,
            usize::from(column),
            self.transcript_content_width,
        )
    }

    pub(super) fn has_screen_row(&self, row: u16) -> bool {
        self.row_at_screen(usize::from(row)).is_some()
    }

    /// Returns the transcript edge reached by a drag coordinate, if any.
    pub fn transcript_edge_for_drag(&self, row: u16) -> Option<ViewportEdge> {
        if self.transcript_rows.is_empty() {
            return None;
        }

        let first = self.rows.get(self.transcript_rows.start)?.screen_row;
        let last = self
            .rows
            .get(self.transcript_rows.end.saturating_sub(1))?
            .screen_row;
        let row = usize::from(row);
        if row <= first {
            return Some(ViewportEdge::AboveTranscript);
        }
        if row >= last {
            return Some(ViewportEdge::BelowTranscript);
        }

        None
    }

    /// Returns a selection focus point at the current transcript edge.
    pub fn transcript_edge_point(&self, edge: ViewportEdge) -> Option<SelectionPoint> {
        if self.transcript_rows.is_empty() {
            return None;
        }

        let index = match edge {
            ViewportEdge::AboveTranscript => self.transcript_rows.start,
            ViewportEdge::BelowTranscript => self.transcript_rows.end.saturating_sub(1),
        };
        let row = self.rows.get(index)?;
        let column = match edge {
            ViewportEdge::AboveTranscript => selectable_segments(row).first()?.start,
            ViewportEdge::BelowTranscript => selectable_segments(row).last()?.end,
        };

        Some(SelectionPoint::new(row, column))
    }

    fn row_at_screen(&self, row: usize) -> Option<&SelectableRow> {
        self.rows
            .iter()
            .find(|candidate| candidate.screen_row == row)
    }
}

struct ProjectionBuilder {
    rows: Vec<SelectableRow>,
    screen_row: usize,
    screen_width: usize,
}

impl ProjectionBuilder {
    fn new(screen_width: usize) -> Self {
        Self {
            rows: Vec::new(),
            screen_row: 0,
            screen_width,
        }
    }

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn finish(self) -> Vec<SelectableRow> {
        self.rows
    }

    fn append_transcript_rows(
        &mut self,
        state: &State,
        width: usize,
        layout: &TranscriptLayout,
        window: Range<usize>,
    ) {
        let window = window.start.min(layout.total_rows)..window.end.min(layout.total_rows);
        let item_range = layout.item_range(window.clone());

        for item_layout in layout
            .items
            .iter()
            .skip(item_range.start)
            .take(item_range.end.saturating_sub(item_range.start))
        {
            let Some(item) = state.session.transcript.get(item_layout.item_index) else {
                continue;
            };
            let mut global_row = item_layout.start_row;
            for row in transcript_projection_rows(item, width) {
                if global_row >= window.start && global_row < window.end {
                    self.push_projection_row(row, width, global_row);
                }
                global_row = global_row.saturating_add(1);
                if global_row >= window.end {
                    break;
                }
            }
        }
    }

    fn push_projection_row(
        &mut self,
        row: SourceProjectionRow,
        width: usize,
        logical_index: usize,
    ) {
        self.push_row(
            SelectableSurface::Transcript,
            logical_index,
            row.source,
            row.text,
            row.source_columns,
            row.source_width,
            0..width,
        );
    }

    fn append_working(&mut self, state: &State) {
        let Some(line) = working_render_line(state) else {
            return;
        };

        let width = line_width(&line);
        self.push_row(
            SelectableSurface::Working,
            0,
            SelectableSource::Working,
            line.plain_text(),
            0..width,
            width,
            0..self.screen_width,
        );
        self.push_blank(SelectableSurface::Working, 1, SelectableSource::Working);
    }

    fn append_input_notice(&mut self, state: &State) {
        let Some(line) = input_notice_render_line(state) else {
            return;
        };

        let width = line_width(&line);
        self.push_row(
            SelectableSurface::InputNotice,
            0,
            SelectableSource::InputNotice,
            line.plain_text(),
            0..width,
            width,
            0..self.screen_width,
        );
    }

    fn append_prompt_area(&mut self, state: &State, width: u16) {
        if state.selection.is_some() {
            self.append_selection_prompt(state);
            return;
        }

        self.append_prompt(state, width);
    }

    fn append_selection_prompt(&mut self, state: &State) {
        let selection_lines = selection_prompt_render_lines(state);
        let selection_line_count = selection_lines.len();
        for (line, render_line) in selection_lines.into_iter().enumerate() {
            let width = line_width(&render_line);
            self.push_row(
                SelectableSurface::SelectionPrompt,
                line,
                SelectableSource::SelectionPrompt { line },
                render_line.plain_text(),
                0..width,
                width,
                0..self.screen_width,
            );
        }
        self.push_blank(
            SelectableSurface::SelectionPrompt,
            selection_line_count,
            SelectableSource::SelectionPrompt {
                line: selection_line_count,
            },
        );
    }

    fn append_prompt(&mut self, state: &State, width: u16) {
        let prompt_selection = prompt_row_selection_metadata(state, width, self.screen_width);
        let prompt_lines = prompt_render_lines_with_width(state, Some(width));
        let prompt_line_count = prompt_lines.len();

        for (line, render_line) in prompt_lines.into_iter().enumerate() {
            let width = line_width(&render_line);
            let metadata = prompt_selection
                .get(line)
                .cloned()
                .unwrap_or_else(PromptRowSelection::empty);
            self.push_row_with_selectable(
                SelectableSurface::Prompt,
                line,
                SelectableSource::Prompt { line },
                render_line.plain_text(),
                0..width,
                width,
                metadata.selectable_columns,
                metadata.excluded_columns,
            );
        }
        self.push_blank(
            SelectableSurface::Prompt,
            prompt_line_count,
            SelectableSource::Prompt {
                line: prompt_line_count,
            },
        );
    }

    fn append_footer(&mut self, state: &State, width: u16) {
        let footer = crate::components::footer_render_line_with_width(state, width);
        let footer_width = line_width(&footer);
        self.push_row(
            SelectableSurface::Footer,
            0,
            SelectableSource::Footer,
            footer.plain_text(),
            0..footer_width,
            footer_width,
            0..self.screen_width,
        );
    }

    fn push_row(
        &mut self,
        surface: SelectableSurface,
        logical_index: usize,
        source: SelectableSource,
        text: String,
        source_columns: Range<usize>,
        source_width: usize,
        selectable_columns: Range<usize>,
    ) {
        self.push_row_with_selectable(
            surface,
            logical_index,
            source,
            text,
            source_columns,
            source_width,
            selectable_columns,
            Vec::new(),
        );
    }

    fn push_row_with_selectable(
        &mut self,
        surface: SelectableSurface,
        logical_index: usize,
        source: SelectableSource,
        text: String,
        source_columns: Range<usize>,
        source_width: usize,
        selectable_columns: Range<usize>,
        excluded_columns: Vec<Range<usize>>,
    ) {
        self.rows.push(SelectableRow {
            order: self.rows.len(),
            screen_row: self.screen_row,
            logical_row: SelectionRowKey {
                surface,
                index: logical_index,
            },
            surface,
            source,
            text,
            source_columns,
            source_width,
            selectable_columns,
            excluded_columns,
        });
        self.screen_row = self.screen_row.saturating_add(1);
    }

    fn push_blank(
        &mut self,
        surface: SelectableSurface,
        logical_index: usize,
        source: SelectableSource,
    ) {
        self.push_row(
            surface,
            logical_index,
            source,
            String::new(),
            0..0,
            0,
            0..self.screen_width,
        );
    }
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

pub(super) fn selectable_segments(row: &SelectableRow) -> Vec<Range<usize>> {
    split_around_excluded(row.selectable_columns.clone(), &row.excluded_columns)
}

fn selectable_point_at(
    row: &SelectableRow,
    column: usize,
    transcript_content_width: usize,
) -> Option<SelectionPoint> {
    if row.surface == SelectableSurface::Transcript && column >= transcript_content_width {
        return None;
    }
    if !selectable_segments(row)
        .into_iter()
        .any(|range| range.contains(&column))
    {
        return None;
    }

    Some(SelectionPoint::new(row, column))
}

fn clamped_point_at(
    row: &SelectableRow,
    column: usize,
    transcript_content_width: usize,
) -> Option<SelectionPoint> {
    if row.surface == SelectableSurface::Transcript && column >= transcript_content_width {
        return None;
    }
    let segments = selectable_segments(row);
    let column = nearest_selectable_boundary(column, &segments)?;
    Some(SelectionPoint::new(row, column))
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

fn transcript_projection_rows(item: &TranscriptItem, width: usize) -> Vec<SourceProjectionRow> {
    let wrap = !transcript_item_uses_no_wrap(item);
    transcript_item_render_lines(item)
        .into_iter()
        .enumerate()
        .flat_map(|(line, render_line)| {
            let source = SelectableSource::Transcript {
                item_id: item.id.as_str().to_owned(),
                line,
            };
            let text = render_line.plain_text();
            if wrap {
                return wrapped_text_rows(&text, width, source);
            }

            let width = crate::selection::display_width(&text);
            vec![SourceProjectionRow {
                source,
                text,
                source_columns: 0..width,
                source_width: width,
            }]
        })
        .collect()
}

fn transcript_item_uses_no_wrap(item: &TranscriptItem) -> bool {
    matches!(
        item.content,
        TranscriptItemContent::OpeningBanner(_)
            | TranscriptItemContent::ToolCall(_)
            | TranscriptItemContent::Command(_)
    )
}

fn transcript_visible_rows(state: &State, total_rows: usize) -> usize {
    if state.scroll.visible_rows == 0 {
        return total_rows.min(200);
    }

    usize::try_from(state.scroll.visible_rows)
        .unwrap_or(usize::MAX)
        .min(total_rows)
}

fn transcript_selection_window(
    selection: &RenderedSelectionState,
    total_rows: usize,
) -> Option<Range<usize>> {
    let (start, end) = normalized_points(selection)?;
    let transcript = SelectableSurface::Transcript;
    if total_rows == 0
        || start.logical_row.surface > transcript
        || end.logical_row.surface < transcript
    {
        return None;
    }

    let start = if start.logical_row.surface == transcript {
        start.logical_row.index
    } else {
        0
    };
    let end = if end.logical_row.surface == transcript {
        end.logical_row.index.saturating_add(1)
    } else {
        total_rows
    };

    let start = start.min(total_rows);
    let end = end.min(total_rows);
    (start < end).then_some(start..end)
}

fn scroll_offset_from_top(total_rows: usize, visible_rows: usize, offset: u32) -> usize {
    let max_offset = total_rows.saturating_sub(visible_rows);
    max_offset.saturating_sub(
        usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(max_offset),
    )
}
