use super::builder::{ProjectedRowSpec, ProjectionBuilder};
use crate::selection::wrapping::{wrapped_text_rows, SourceProjectionRow};
use crate::selection::{
    display_width, normalized_points, RenderedSelectionState, SelectableSource, SelectableSurface,
};
use crate::state::State;
use crate::transcript::{
    transcript_item_rows, TranscriptItem, TranscriptLayout, TranscriptRowWrap,
};
use std::ops::Range;

pub(super) fn append_rows(
    builder: &mut ProjectionBuilder,
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
        for row in projection_rows(item, width) {
            if global_row >= window.start && global_row < window.end {
                push_projection_row(builder, row, width, global_row);
            }
            global_row = global_row.saturating_add(1);
            if global_row >= window.end {
                break;
            }
        }
    }
}

pub(super) fn visible_rows(state: &State, total_rows: usize) -> usize {
    if state.scroll.visible_rows == 0 {
        return total_rows.min(200);
    }

    usize::try_from(state.scroll.visible_rows)
        .unwrap_or(usize::MAX)
        .min(total_rows)
}

pub(super) fn selection_window(
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

pub(super) fn scroll_offset_from_top(total_rows: usize, visible_rows: usize, offset: u32) -> usize {
    let max_offset = total_rows.saturating_sub(visible_rows);
    max_offset.saturating_sub(
        usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(max_offset),
    )
}

fn push_projection_row(
    builder: &mut ProjectionBuilder,
    row: SourceProjectionRow,
    width: usize,
    logical_index: usize,
) {
    builder.push_row(ProjectedRowSpec {
        surface: SelectableSurface::Transcript,
        logical_index,
        source: row.source,
        text: row.text,
        source_columns: row.source_columns,
        source_width: row.source_width,
        selectable_columns: 0..width,
        excluded_columns: Vec::new(),
    });
}

fn projection_rows(item: &TranscriptItem, width: usize) -> Vec<SourceProjectionRow> {
    transcript_item_rows(item)
        .into_iter()
        .flat_map(|row| {
            let source = SelectableSource::Transcript {
                item_id: item.id.as_str().to_owned(),
                line: row.line,
            };
            if row.wrap == TranscriptRowWrap::Wrap {
                return wrapped_text_rows(&row.text, width, source);
            }

            let width = display_width(&row.text);
            vec![SourceProjectionRow {
                source,
                text: row.text,
                source_columns: 0..width,
                source_width: width,
            }]
        })
        .collect()
}
