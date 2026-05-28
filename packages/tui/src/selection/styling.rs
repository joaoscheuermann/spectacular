use crate::render::{RenderHighlight, RenderLine, RenderSpan, RenderStyle};
use crate::selection::projection::{SelectableProjection, SelectableRow};
use crate::selection::ranges::{intersect_range, split_around_excluded};
use crate::selection::{
    display_width, line_width, normalized_points, RenderedSelectionState, SelectableSource,
    SelectionPoint,
};
use crate::state::State;
use std::collections::HashMap;
use std::ops::Range;
use unicode_width::UnicodeWidthChar;

/// Applies rendered-selection styling to one component-owned semantic line.
pub fn style_line_for_source(
    state: &State,
    line: RenderLine,
    source: SelectableSource,
) -> RenderLine {
    if !state.app_selection.has_selection() {
        return line;
    }

    let projection = SelectableProjection::for_state(state);
    style_line_for_source_with_projection(&state.app_selection, &projection, line, source, 0, true)
}

/// Applies rendered-selection styling to a segment that starts at a source column offset.
pub fn style_line_for_source_at_columns(
    state: &State,
    line: RenderLine,
    source: SelectableSource,
    column_offset: usize,
) -> RenderLine {
    if !state.app_selection.has_selection() {
        return line;
    }

    let projection = SelectableProjection::for_state(state);
    style_line_for_source_with_projection(
        &state.app_selection,
        &projection,
        line,
        source,
        column_offset,
        false,
    )
}

/// Applies rendered-selection styling using a projection already built for this frame.
pub fn style_line_for_source_with_projection(
    selection: &RenderedSelectionState,
    projection: &SelectableProjection,
    line: RenderLine,
    source: SelectableSource,
    column_offset: usize,
    append_virtual_selection: bool,
) -> RenderLine {
    if !selection.has_selection() {
        return line;
    }

    let plan = SelectionStylingPlan::new(selection, projection);
    style_line_for_source_with_plan(&plan, line, source, column_offset, append_virtual_selection)
}

/// Applies rendered-selection styling using a precomputed source-range plan.
pub fn style_line_for_source_with_plan(
    plan: &SelectionStylingPlan,
    line: RenderLine,
    source: SelectableSource,
    column_offset: usize,
    append_virtual_selection: bool,
) -> RenderLine {
    style_line_for_source_at_columns_inner(
        plan,
        line,
        source,
        column_offset,
        append_virtual_selection,
    )
}

pub(super) fn selected_text_from_projection(
    selection: &RenderedSelectionState,
    projection: &SelectableProjection,
) -> Option<String> {
    if !selection.has_selection() {
        return None;
    }

    let selected_rows = selected_rows(selection, projection);
    if selected_rows.is_empty() {
        return None;
    }

    Some(selected_text_from_rows(selected_rows))
}

fn style_line_for_source_at_columns_inner(
    plan: &SelectionStylingPlan,
    line: RenderLine,
    source: SelectableSource,
    column_offset: usize,
    append_virtual_selection: bool,
) -> RenderLine {
    let line_width = line_width(&line);
    let ranges = plan
        .ranges_for(&source)
        .iter()
        .cloned()
        .filter_map(|range| {
            local_selection_range(range, column_offset, line_width, append_virtual_selection)
        })
        .collect::<Vec<_>>();
    if ranges.is_empty() {
        return line;
    }

    apply_column_ranges(line, &ranges, append_virtual_selection)
}

/// Precomputed selected source-column ranges for one visible projection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionStylingPlan {
    ranges_by_source: HashMap<SelectableSource, Vec<Range<usize>>>,
}

impl SelectionStylingPlan {
    /// Builds a source-range lookup once for the current render context.
    pub fn new(selection: &RenderedSelectionState, projection: &SelectableProjection) -> Self {
        if !selection.has_selection() {
            return Self::default();
        }

        let mut ranges_by_source: HashMap<SelectableSource, Vec<Range<usize>>> = HashMap::new();
        for selected in selected_rows(selection, projection) {
            let source = selected.row.source.clone();
            ranges_by_source
                .entry(source)
                .or_default()
                .extend(selected_source_ranges_for_row(selected));
        }
        for ranges in ranges_by_source.values_mut() {
            ranges.retain(|range| range.start < range.end);
            ranges.sort_by_key(|range| (range.start, range.end));
        }

        Self { ranges_by_source }
    }

    fn ranges_for(&self, source: &SelectableSource) -> &[Range<usize>] {
        self.ranges_by_source
            .get(source)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

fn local_selection_range(
    range: Range<usize>,
    column_offset: usize,
    line_width: usize,
    append_virtual_selection: bool,
) -> Option<Range<usize>> {
    let start = range.start.max(column_offset);
    let end = if append_virtual_selection {
        range.end
    } else {
        range.end.min(column_offset.saturating_add(line_width))
    };

    (start < end).then(|| start.saturating_sub(column_offset)..end.saturating_sub(column_offset))
}

fn selected_text_from_rows(selected_rows: Vec<SelectedRow<'_>>) -> String {
    let mut output_lines = Vec::new();
    let mut current_source: Option<SelectableSource> = None;
    let mut current_line = String::new();

    for selected in selected_rows {
        let piece = display_substring(&selected.row.text, selected.columns.clone());
        if current_source.as_ref() != Some(&selected.row.source) {
            if current_source.is_some() {
                output_lines.push(std::mem::take(&mut current_line));
            }
            current_source = Some(selected.row.source.clone());
        }
        current_line.push_str(&piece);
    }

    if current_source.is_some() {
        output_lines.push(current_line);
    }

    output_lines.join("\n")
}

fn selected_rows<'a>(
    selection: &RenderedSelectionState,
    projection: &'a SelectableProjection,
) -> Vec<SelectedRow<'a>> {
    let Some((start, end)) = normalized_points(selection) else {
        return Vec::new();
    };

    projection
        .rows
        .iter()
        .filter(|row| row.logical_row >= start.logical_row && row.logical_row <= end.logical_row)
        .flat_map(|row| selected_rows_for_projection_row(row, start, end))
        .collect()
}

fn selected_rows_for_projection_row(
    row: &SelectableRow,
    start: SelectionPoint,
    end: SelectionPoint,
) -> Vec<SelectedRow<'_>> {
    selected_columns_for_row(row, start, end)
        .into_iter()
        .map(|columns| SelectedRow { row, columns })
        .collect()
}

fn selected_columns_for_row(
    row: &SelectableRow,
    start: SelectionPoint,
    end: SelectionPoint,
) -> Vec<Range<usize>> {
    let columns = if start.logical_row == end.logical_row {
        start.column.min(end.column)..start.column.max(end.column)
    } else if row.logical_row == start.logical_row {
        start.column..row.selectable_columns.end
    } else if row.logical_row == end.logical_row {
        row.selectable_columns.start..end.column
    } else {
        row.selectable_columns.clone()
    };

    selected_selectable_segments(columns, row)
}

fn selected_selectable_segments(columns: Range<usize>, row: &SelectableRow) -> Vec<Range<usize>> {
    let Some(columns) = intersect_range(columns, row.selectable_columns.clone()) else {
        return Vec::new();
    };

    split_around_excluded(columns, &row.excluded_columns)
}

fn apply_column_ranges(
    line: RenderLine,
    ranges: &[Range<usize>],
    append_virtual_selection: bool,
) -> RenderLine {
    let mut spans = Vec::new();
    let mut column = 0usize;

    for span in line.spans {
        for character in span.text.chars() {
            let width = character.width().unwrap_or(0);
            let next_column = column.saturating_add(width);
            let highlight =
                selection_highlight_for_cell(column, next_column, ranges, span.highlight);
            push_char_span(&mut spans, character, span.style, highlight);
            column = next_column;
        }
    }

    if append_virtual_selection {
        append_virtual_selection_spans(&mut spans, column, ranges);
    }

    RenderLine::from_spans(spans)
}

fn selection_highlight_for_cell(
    column: usize,
    next_column: usize,
    ranges: &[Range<usize>],
    highlight: Option<RenderHighlight>,
) -> Option<RenderHighlight> {
    let selected = ranges
        .iter()
        .any(|range| column < range.end && range.start < next_column);
    if selected && highlight != Some(RenderHighlight::Cursor) {
        return Some(RenderHighlight::Selection);
    }

    highlight
}

fn push_char_span(
    spans: &mut Vec<RenderSpan>,
    character: char,
    style: RenderStyle,
    highlight: Option<RenderHighlight>,
) {
    if let Some(last) = spans.last_mut() {
        if last.style == style && last.highlight == highlight {
            last.text.push(character);
            return;
        }
    }

    let mut span = RenderSpan::new(character.to_string(), style);
    span.highlight = highlight;
    spans.push(span);
}

fn append_virtual_selection_spans(
    spans: &mut Vec<RenderSpan>,
    rendered_width: usize,
    ranges: &[Range<usize>],
) {
    let mut column = rendered_width;
    for range in ranges
        .iter()
        .filter(|range| range.end > rendered_width)
        .cloned()
    {
        let start = range.start.max(rendered_width);
        if column < start {
            push_space_span(spans, start.saturating_sub(column), None);
            column = start;
        }
        if column < range.end {
            push_space_span(
                spans,
                range.end.saturating_sub(column),
                Some(RenderHighlight::Selection),
            );
            column = range.end;
        }
    }
}

fn push_space_span(spans: &mut Vec<RenderSpan>, count: usize, highlight: Option<RenderHighlight>) {
    if count == 0 {
        return;
    }

    push_text_span(spans, " ".repeat(count), RenderStyle::Text, highlight);
}

fn push_text_span(
    spans: &mut Vec<RenderSpan>,
    text: String,
    style: RenderStyle,
    highlight: Option<RenderHighlight>,
) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut() {
        if last.style == style && last.highlight == highlight {
            last.text.push_str(&text);
            return;
        }
    }

    let mut span = RenderSpan::new(text, style);
    span.highlight = highlight;
    spans.push(span);
}

fn selected_source_ranges_for_row(selected: SelectedRow<'_>) -> Vec<Range<usize>> {
    let row = selected.row;
    let rendered_width = display_width(&row.text);
    let mut ranges = Vec::new();

    if let Some(real) = intersect_range(selected.columns.clone(), 0..rendered_width) {
        ranges.push(
            row.source_columns.start.saturating_add(real.start)
                ..row.source_columns.start.saturating_add(real.end),
        );
    }

    let virtual_start = selected.columns.start.max(rendered_width);
    if virtual_start < selected.columns.end && row.source_columns.end >= row.source_width {
        ranges.push(
            row.source_width
                .saturating_add(virtual_start.saturating_sub(rendered_width))
                ..row
                    .source_width
                    .saturating_add(selected.columns.end.saturating_sub(rendered_width)),
        );
    }

    ranges
}

fn display_substring(text: &str, columns: Range<usize>) -> String {
    let mut output = String::new();
    let mut column = 0usize;
    for character in text.chars() {
        let width = character.width().unwrap_or(0);
        let next_column = column.saturating_add(width);
        if column < columns.end && columns.start < next_column {
            output.push(character);
        }
        column = next_column;
    }

    output
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedRow<'a> {
    row: &'a SelectableRow,
    columns: Range<usize>,
}
