mod builder;
mod chrome;
mod prompt;
mod transcript;

use self::builder::ProjectionBuilder;
use crate::selection::ranges::{nearest_selectable_boundary, split_around_excluded};
use crate::selection::{
    screen_width, RenderedSelectionState, SelectableSource, SelectableSurface, SelectionPoint,
    ViewportEdge,
};
use crate::state::State;
use crate::transcript::TranscriptLayout;
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
        let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
        let layout = TranscriptLayout::for_state(state, transcript_content_width);
        Self::for_state_with_layout(state, &layout)
    }

    /// Builds the app-wide selectable projection from cached transcript layout metadata.
    pub(crate) fn for_state_with_layout(state: &State, layout: &TranscriptLayout) -> Self {
        let width = screen_width(state);
        let screen_width = usize::from(width).max(1);
        let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
        let visible_rows = transcript::visible_rows(state, layout.total_rows);
        let scroll_offset = transcript::scroll_offset_from_top(
            layout.total_rows,
            visible_rows,
            state.scroll.offset,
        );

        Self::for_transcript_window(
            state,
            transcript_content_width,
            screen_width,
            layout,
            scroll_offset..scroll_offset.saturating_add(visible_rows),
        )
    }

    /// Builds the bounded projection needed for semantic copy of the current selection.
    pub fn for_selection_copy(state: &State, selection: &RenderedSelectionState) -> Self {
        let width = screen_width(state);
        let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
        let layout = TranscriptLayout::for_state(state, transcript_content_width);
        Self::for_selection_copy_with_layout(state, selection, &layout)
    }

    /// Builds bounded selection-copy projection from cached transcript layout metadata.
    pub(crate) fn for_selection_copy_with_layout(
        state: &State,
        selection: &RenderedSelectionState,
        layout: &TranscriptLayout,
    ) -> Self {
        let width = screen_width(state);
        let screen_width = usize::from(width).max(1);
        let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
        let window =
            transcript::selection_window(selection, layout.total_rows).unwrap_or_else(|| {
                let visible_rows = transcript::visible_rows(state, layout.total_rows);
                let scroll_offset = transcript::scroll_offset_from_top(
                    layout.total_rows,
                    visible_rows,
                    state.scroll.offset,
                );
                scroll_offset..scroll_offset.saturating_add(visible_rows)
            });

        Self::for_transcript_window(
            state,
            transcript_content_width,
            screen_width,
            layout,
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
        transcript::append_rows(
            &mut builder,
            state,
            transcript_content_width,
            layout,
            transcript_window,
        );
        let transcript_end = builder.len();

        let width = u16::try_from(screen_width).unwrap_or(u16::MAX);
        chrome::append_working(&mut builder, state);
        chrome::append_input_notice(&mut builder, state);
        prompt::append_area(&mut builder, state, width);
        chrome::append_footer(&mut builder, state, width);

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
