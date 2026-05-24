use crate::components::{transcript_capacity_rows, transcript_layout_total_rows};
use crate::render::TuiSelectionColors;
use crate::scroll::TranscriptScrollState;
use crate::selection::{RenderedSelectionState, ViewportEdge};
use crate::state::State;
use crate::transcript::{TranscriptLayoutCache, TranscriptLayoutSnapshot};

/// Local, non-durable view state owned by the terminal view layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewState {
    pub scroll: TranscriptScrollState,
    pub app_selection: RenderedSelectionState,
    pub selection_colors: TuiSelectionColors,
    transcript_layout: TranscriptLayoutCache,
}

impl ViewState {
    /// Builds local view state from the legacy render-state mirror.
    pub fn from_state(state: &State) -> Self {
        Self {
            scroll: state.scroll.clone(),
            app_selection: state.app_selection.clone(),
            selection_colors: state.selection_colors,
            transcript_layout: TranscriptLayoutCache::default(),
        }
    }

    /// Resets transient viewport and selection state for a new session.
    pub fn reset_for_session(&mut self) {
        self.scroll = TranscriptScrollState::follow_tail();
        self.app_selection = RenderedSelectionState::default();
        self.transcript_layout.reset_for_session();
    }

    /// Records the earliest semantic transcript index whose layout may have changed.
    pub(crate) fn note_transcript_change(&mut self, start_index: usize) {
        self.transcript_layout.note_change(start_index);
    }
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            scroll: TranscriptScrollState::follow_tail(),
            app_selection: RenderedSelectionState::default(),
            selection_colors: TuiSelectionColors::from_env(),
            transcript_layout: TranscriptLayoutCache::default(),
        }
    }
}

/// Local view-only actions that do not belong in semantic reducer state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewAction {
    ScrollTranscript(i32),
    RenderedSelectionChanged(RenderedSelectionState),
    RenderedSelectionCleared,
    RenderedSelectionAutoScroll(ViewportEdge),
    RenderedSelectionFeedbackReported { message: String },
    Resize { width: u16, height: u16 },
}

/// Applies a view-local action using the current semantic state as layout input.
pub fn apply_view_action(view: &mut ViewState, state: &State, action: ViewAction) {
    match action {
        ViewAction::ScrollTranscript(delta) => {
            let total_rows = total_transcript_rows_with_view(state, view);
            view.scroll.scroll_by(
                delta,
                max_scroll_offset(total_rows, view.scroll.visible_rows),
            );
        }
        ViewAction::RenderedSelectionChanged(selection) => {
            view.app_selection = selection;
        }
        ViewAction::RenderedSelectionCleared => {
            view.app_selection = Default::default();
        }
        ViewAction::RenderedSelectionAutoScroll(edge) => {
            auto_scroll_selection(state, view, edge);
        }
        ViewAction::RenderedSelectionFeedbackReported { message } => {
            if view.app_selection.has_selection() {
                view.app_selection.copy_feedback = Some(message);
            }
        }
        ViewAction::Resize { width, height } => {
            view.scroll.visible_rows =
                u32::from(transcript_capacity_rows(state, height, Some(width)));
            clamp_scroll_to_transcript(view, state);
        }
    }
}

/// Applies a view action to a materialized state frame for compatibility tests.
pub fn apply_view_action_to_state(state: &mut State, action: ViewAction) {
    let semantic = state.clone();
    let mut view = ViewState::from_state(state);
    apply_view_action(&mut view, &semantic, action);
    *state = materialize_state(&semantic, &view);
}

/// Returns semantic state with the local view state mirrored for existing render helpers.
pub fn materialize_state(state: &State, view: &ViewState) -> State {
    let mut state = state.clone();
    state.scroll = view.scroll.clone();
    state.app_selection = view.app_selection.clone();
    state.selection_colors = view.selection_colors;
    state
}

/// Keeps a reviewing viewport pinned to the same rendered rows after transcript growth.
pub fn preserve_review_position_for_growth(view: &mut ViewState, old_rows: usize, new_rows: usize) {
    if view.scroll.follow_tail {
        return;
    }

    let row_delta = u32::try_from(new_rows.saturating_sub(old_rows)).unwrap_or(u32::MAX);
    view.scroll.offset = view.scroll.offset.saturating_add(row_delta);
    let max_offset = max_scroll_offset(new_rows, view.scroll.visible_rows);
    view.scroll.offset = view.scroll.offset.min(max_offset);
    view.scroll.follow_tail = view.scroll.offset == 0;
}

/// Clears rendered selection for semantic changes that invalidate visible coordinates.
pub fn clear_selection(view: &mut ViewState) {
    view.app_selection = RenderedSelectionState::default();
}

/// Returns the total rendered transcript rows at the current view width.
pub fn total_transcript_rows(state: &State) -> usize {
    transcript_layout_total_rows(state, transcript_content_width(state))
}

/// Returns cached transcript layout metadata for the current view width.
pub(crate) fn transcript_layout_snapshot(
    state: &State,
    view: &mut ViewState,
) -> TranscriptLayoutSnapshot {
    view.transcript_layout
        .snapshot_for_state(state, transcript_content_width(state))
}

/// Returns cached total rendered transcript rows for runtime view paths.
pub(crate) fn total_transcript_rows_with_view(state: &State, view: &mut ViewState) -> usize {
    transcript_layout_snapshot(state, view).layout.total_rows
}

fn auto_scroll_selection(state: &State, view: &mut ViewState, edge: ViewportEdge) {
    let mut frame = materialize_state(state, view);
    let layout = transcript_layout_snapshot(&frame, view);
    crate::selection::auto_scroll_selection_with_layout(&mut frame, edge, &layout.layout);
    view.scroll = frame.scroll;
    view.app_selection = frame.app_selection;
    view.selection_colors = frame.selection_colors;
}

fn clamp_scroll_to_transcript(view: &mut ViewState, state: &State) {
    let total_rows = total_transcript_rows_with_view(state, view);
    view.scroll.offset = view
        .scroll
        .offset
        .min(max_scroll_offset(total_rows, view.scroll.visible_rows));
    view.scroll.follow_tail = view.scroll.offset == 0;
}

fn max_scroll_offset(total_rows: usize, visible_rows: u32) -> u32 {
    if visible_rows == 0 {
        return u32::MAX;
    }

    u32::try_from(total_rows)
        .unwrap_or(u32::MAX)
        .saturating_sub(visible_rows)
}

fn transcript_content_width(state: &State) -> usize {
    state.prompt_layout.content_width.saturating_add(1).max(1)
}
