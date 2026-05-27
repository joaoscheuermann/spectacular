use super::builder::{ProjectedRowSpec, ProjectionBuilder};
use crate::components::{input_notice_render_line, working_render_line};
use crate::selection::{line_width, SelectableSource, SelectableSurface};
use crate::state::State;

pub(super) fn append_working(builder: &mut ProjectionBuilder, state: &State) {
    let Some(line) = working_render_line(state) else {
        return;
    };

    let width = line_width(&line);
    builder.push_row(ProjectedRowSpec {
        surface: SelectableSurface::Working,
        logical_index: 0,
        source: SelectableSource::Working,
        text: line.plain_text(),
        source_columns: 0..width,
        source_width: width,
        selectable_columns: 0..builder.screen_width(),
        excluded_columns: Vec::new(),
    });
    builder.push_blank(SelectableSurface::Working, 1, SelectableSource::Working);
}

pub(super) fn append_input_notice(builder: &mut ProjectionBuilder, state: &State) {
    let Some(line) = input_notice_render_line(state) else {
        return;
    };

    let width = line_width(&line);
    builder.push_row(ProjectedRowSpec {
        surface: SelectableSurface::InputNotice,
        logical_index: 0,
        source: SelectableSource::InputNotice,
        text: line.plain_text(),
        source_columns: 0..width,
        source_width: width,
        selectable_columns: 0..builder.screen_width(),
        excluded_columns: Vec::new(),
    });
}

pub(super) fn append_footer(builder: &mut ProjectionBuilder, state: &State, width: u16) {
    let footer = crate::components::footer_render_line_with_width(state, width);
    let footer_width = line_width(&footer);
    builder.push_row(ProjectedRowSpec {
        surface: SelectableSurface::Footer,
        logical_index: 0,
        source: SelectableSource::Footer,
        text: footer.plain_text(),
        source_columns: 0..footer_width,
        source_width: footer_width,
        selectable_columns: 0..builder.screen_width(),
        excluded_columns: Vec::new(),
    });
}
