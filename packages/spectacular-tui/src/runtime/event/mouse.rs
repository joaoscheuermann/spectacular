use super::{view_action_effects, EventEffect};
use crate::selection::{drag_selection_to, finish_selection_at, start_selection_at};
use crate::state::State;
use crate::view::ViewAction;
use iocraft::prelude::{FullscreenMouseEvent, MouseEventKind};

/// Converts one fullscreen mouse event into rendered-selection or scroll effects.
pub(super) fn effects(state: &State, event: FullscreenMouseEvent) -> Vec<EventEffect> {
    match event.kind {
        MouseEventKind::Down(_) => start_effects(state, event.column, event.row),
        MouseEventKind::Drag(_) => drag_effects(state, event.column, event.row),
        MouseEventKind::Up(_) => finish_effects(state, event.column, event.row),
        MouseEventKind::ScrollUp => view_action_effects(ViewAction::ScrollTranscript(3)),
        MouseEventKind::ScrollDown => view_action_effects(ViewAction::ScrollTranscript(-3)),
        MouseEventKind::Moved | MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => {
            Vec::new()
        }
    }
}

fn start_effects(state: &State, column: u16, row: u16) -> Vec<EventEffect> {
    let Some(selection) = start_selection_at(state, column, row) else {
        return Vec::new();
    };

    view_action_effects(ViewAction::RenderedSelectionChanged(selection))
}

fn drag_effects(state: &State, column: u16, row: u16) -> Vec<EventEffect> {
    let Some(selection) = drag_selection_to(state, column, row) else {
        return Vec::new();
    };
    if let Some(edge) = selection.viewport_edge {
        return view_action_effects(ViewAction::RenderedSelectionAutoScroll(edge));
    }

    view_action_effects(ViewAction::RenderedSelectionChanged(selection))
}

fn finish_effects(state: &State, column: u16, row: u16) -> Vec<EventEffect> {
    let Some(selection) = finish_selection_at(state, column, row) else {
        return Vec::new();
    };
    if !selection.has_selection() {
        return view_action_effects(ViewAction::RenderedSelectionCleared);
    }

    view_action_effects(ViewAction::RenderedSelectionChanged(selection))
}
