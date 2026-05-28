mod projection;
mod ranges;
mod styling;
mod wrapping;

use crate::state::State;
use crate::transcript::TranscriptLayout;
use std::cmp::Ordering;

pub use projection::{SelectableProjection, SelectableRow, SelectionRowKey};
pub use styling::{
    style_line_for_source, style_line_for_source_at_columns, style_line_for_source_with_plan,
    style_line_for_source_with_projection, SelectionStylingPlan,
};

const DEFAULT_SCREEN_WIDTH: u16 = 120;
const PROMPT_MARKER_WIDTH: usize = 2;
const TRANSCRIPT_SCROLL_STEP: i32 = 1;

/// User-visible feedback after rendered text is copied.
pub const COPIED_SELECTION_NOTICE: &str = "Copied Selection";

/// App-wide rendered-text selection state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RenderedSelectionState {
    pub anchor: Option<SelectionPoint>,
    pub focus: Option<SelectionPoint>,
    pub dragging: bool,
    pub selected_surface: Option<SelectableSurface>,
    pub viewport_edge: Option<ViewportEdge>,
    pub copy_feedback: Option<String>,
}

impl RenderedSelectionState {
    /// Returns true when the state contains a non-empty rendered selection.
    pub fn has_selection(&self) -> bool {
        matches!((self.anchor, self.focus), (Some(anchor), Some(focus)) if anchor != focus)
    }

    /// Starts a drag selection from one projected point.
    pub fn started(point: SelectionPoint) -> Self {
        Self {
            anchor: Some(point),
            focus: Some(point),
            dragging: true,
            selected_surface: Some(point.surface),
            viewport_edge: None,
            copy_feedback: None,
        }
    }

    /// Returns this selection with a new focus point and drag state.
    fn with_focus(mut self, point: SelectionPoint, dragging: bool) -> Self {
        self.focus = Some(point);
        self.dragging = dragging;
        self.selected_surface = Some(point.surface);
        self.viewport_edge = None;
        self.copy_feedback = None;
        self
    }

    /// Returns this selection with only drag state changed.
    fn with_dragging(mut self, dragging: bool) -> Self {
        self.dragging = dragging;
        self.viewport_edge = None;
        self.copy_feedback = None;
        if !self.has_selection() {
            return Self::default();
        }

        self
    }
}

/// A concrete point in the visible app-wide rendered text projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionPoint {
    pub surface: SelectableSurface,
    pub logical_row: SelectionRowKey,
    pub column: usize,
}

impl SelectionPoint {
    /// Creates a selection point for one visible projected row.
    fn new(row: &SelectableRow, column: usize) -> Self {
        Self {
            surface: row.surface,
            logical_row: row.logical_row,
            column,
        }
    }
}

/// Visible app surface that can participate in rendered-text selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SelectableSurface {
    Transcript,
    Working,
    InputNotice,
    SelectionPrompt,
    Prompt,
    Footer,
}

/// Transcript viewport edge reached while a drag selection is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewportEdge {
    AboveTranscript,
    BelowTranscript,
}

impl ViewportEdge {
    /// Returns the transcript scroll delta represented by this edge.
    pub fn scroll_delta(self) -> i32 {
        match self {
            Self::AboveTranscript => TRANSCRIPT_SCROLL_STEP,
            Self::BelowTranscript => -TRANSCRIPT_SCROLL_STEP,
        }
    }
}

/// Stable source row identity used to copy semantic rows and style owned components.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SelectableSource {
    Transcript { item_id: String, line: usize },
    Working,
    InputNotice,
    SelectionPrompt { line: usize },
    Prompt { line: usize },
    Footer,
}

/// Starts a rendered text selection at a terminal coordinate.
pub fn start_selection_at(state: &State, column: u16, row: u16) -> Option<RenderedSelectionState> {
    SelectableProjection::for_state(state)
        .point_at(column, row)
        .map(RenderedSelectionState::started)
}

/// Starts a rendered text selection using cached transcript layout metadata.
pub(crate) fn start_selection_at_with_layout(
    state: &State,
    layout: &TranscriptLayout,
    column: u16,
    row: u16,
) -> Option<RenderedSelectionState> {
    SelectableProjection::for_state_with_layout(state, layout)
        .point_at(column, row)
        .map(RenderedSelectionState::started)
}

/// Updates an active rendered selection drag at a terminal coordinate.
pub fn drag_selection_to(state: &State, column: u16, row: u16) -> Option<RenderedSelectionState> {
    let width = screen_width(state);
    let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
    let layout = TranscriptLayout::for_state(state, transcript_content_width);
    drag_selection_to_with_layout(state, &layout, column, row)
}

/// Updates an active rendered selection drag using cached transcript layout metadata.
pub(crate) fn drag_selection_to_with_layout(
    state: &State,
    layout: &TranscriptLayout,
    column: u16,
    row: u16,
) -> Option<RenderedSelectionState> {
    let mut selection = state.app_selection.clone();
    if !selection.dragging {
        return None;
    }

    let projection = SelectableProjection::for_state_with_layout(state, layout);
    if let Some(point) = projection.focus_point_at(column, row) {
        return Some(selection.with_focus(point, true));
    }

    if projection.has_screen_row(row) {
        return None;
    }

    let edge = projection.transcript_edge_for_drag(row)?;
    selection.viewport_edge = Some(edge);
    Some(selection)
}

/// Finishes an active rendered selection drag at a terminal coordinate.
pub fn finish_selection_at(state: &State, column: u16, row: u16) -> Option<RenderedSelectionState> {
    let width = screen_width(state);
    let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
    let layout = TranscriptLayout::for_state(state, transcript_content_width);
    finish_selection_at_with_layout(state, &layout, column, row)
}

/// Finishes an active rendered selection drag using cached transcript layout metadata.
pub(crate) fn finish_selection_at_with_layout(
    state: &State,
    layout: &TranscriptLayout,
    column: u16,
    row: u16,
) -> Option<RenderedSelectionState> {
    let selection = state.app_selection.clone();
    if !selection.dragging {
        return None;
    }

    let projection = SelectableProjection::for_state_with_layout(state, layout);
    if let Some(point) = projection.focus_point_at(column, row) {
        return Some(selection.with_focus(point, false));
    }

    Some(selection.with_dragging(false))
}

/// Scrolls transcript during edge drag and extends focus to the newly exposed edge row.
pub fn auto_scroll_selection(state: &mut State, edge: ViewportEdge) {
    let width = screen_width(state);
    let transcript_content_width = usize::from(width.saturating_sub(1)).max(1);
    let layout = TranscriptLayout::for_state(state, transcript_content_width);
    auto_scroll_selection_with_layout(state, edge, &layout);
}

/// Scrolls transcript during edge drag using cached transcript layout metadata.
pub(crate) fn auto_scroll_selection_with_layout(
    state: &mut State,
    edge: ViewportEdge,
    layout: &TranscriptLayout,
) {
    let total_rows = layout.total_rows;
    let max_offset = u32::try_from(total_rows)
        .unwrap_or(u32::MAX)
        .saturating_sub(state.scroll.visible_rows);
    state.scroll.scroll_by(edge.scroll_delta(), max_offset);

    let projection = SelectableProjection::for_state_with_layout(state, layout);
    if let Some(point) = projection.transcript_edge_point(edge) {
        state.app_selection = state.app_selection.clone().with_focus(point, true);
        state.app_selection.viewport_edge = Some(edge);
    }
}

/// Returns the currently selected rendered plain text, if any.
pub fn selected_text(state: &State) -> Option<String> {
    let projection = SelectableProjection::for_selection_copy(state, &state.app_selection);
    styling::selected_text_from_projection(&state.app_selection, &projection)
}

/// Returns the currently selected rendered plain text using cached transcript layout metadata.
pub(crate) fn selected_text_with_layout(
    state: &State,
    layout: &TranscriptLayout,
) -> Option<String> {
    let projection =
        SelectableProjection::for_selection_copy_with_layout(state, &state.app_selection, layout);
    styling::selected_text_from_projection(&state.app_selection, &projection)
}

pub(super) fn normalized_points(
    selection: &RenderedSelectionState,
) -> Option<(SelectionPoint, SelectionPoint)> {
    let anchor = selection.anchor?;
    let focus = selection.focus?;
    match compare_points(anchor, focus) {
        Ordering::Less | Ordering::Equal => Some((anchor, focus)),
        Ordering::Greater => Some((focus, anchor)),
    }
}

fn compare_points(left: SelectionPoint, right: SelectionPoint) -> Ordering {
    (left.logical_row, left.column).cmp(&(right.logical_row, right.column))
}

pub(super) fn screen_width(state: &State) -> u16 {
    u16::try_from(state.prompt_layout.content_width.saturating_add(2))
        .unwrap_or(DEFAULT_SCREEN_WIDTH)
        .max(1)
}

pub(super) fn line_width(line: &crate::render::RenderLine) -> usize {
    display_width(&line.plain_text())
}

pub(super) fn display_width(value: &str) -> usize {
    use unicode_width::UnicodeWidthChar;

    value
        .chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
}
