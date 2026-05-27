use super::{SelectableRow, SelectionRowKey};
use crate::selection::{SelectableSource, SelectableSurface};
use std::ops::Range;

pub(super) struct ProjectionBuilder {
    rows: Vec<SelectableRow>,
    screen_row: usize,
    screen_width: usize,
}

pub(super) struct ProjectedRowSpec {
    pub(super) surface: SelectableSurface,
    pub(super) logical_index: usize,
    pub(super) source: SelectableSource,
    pub(super) text: String,
    pub(super) source_columns: Range<usize>,
    pub(super) source_width: usize,
    pub(super) selectable_columns: Range<usize>,
    pub(super) excluded_columns: Vec<Range<usize>>,
}

impl ProjectionBuilder {
    pub(super) fn new(screen_width: usize) -> Self {
        Self {
            rows: Vec::new(),
            screen_row: 0,
            screen_width,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn screen_width(&self) -> usize {
        self.screen_width
    }

    pub(super) fn finish(self) -> Vec<SelectableRow> {
        self.rows
    }

    pub(super) fn push_row(&mut self, spec: ProjectedRowSpec) {
        self.rows.push(SelectableRow {
            order: self.rows.len(),
            screen_row: self.screen_row,
            logical_row: SelectionRowKey {
                surface: spec.surface,
                index: spec.logical_index,
            },
            surface: spec.surface,
            source: spec.source,
            text: spec.text,
            source_columns: spec.source_columns,
            source_width: spec.source_width,
            selectable_columns: spec.selectable_columns,
            excluded_columns: spec.excluded_columns,
        });
        self.screen_row = self.screen_row.saturating_add(1);
    }

    pub(super) fn push_blank(
        &mut self,
        surface: SelectableSurface,
        logical_index: usize,
        source: SelectableSource,
    ) {
        self.push_row(ProjectedRowSpec {
            surface,
            logical_index,
            source,
            text: String::new(),
            source_columns: 0..0,
            source_width: 0,
            selectable_columns: 0..self.screen_width,
            excluded_columns: Vec::new(),
        });
    }
}
