use super::{
    input_notice_effect, prompt_change_effect, prompt_changed_if_needed, view_action_effects,
    EventEffect,
};
use crate::runtime::{ClipboardService, PasteBurst};
use crate::selection::{selected_text, selected_text_with_layout, COPIED_SELECTION_NOTICE};
use crate::session::SelectionPromptState;
use crate::state::State;
use crate::transcript::TranscriptLayout;
use crate::view::ViewAction;

const CLIPBOARD_UNAVAILABLE_NOTICE: &str = "Clipboard is unavailable";
const CLIPBOARD_READ_FAILED_NOTICE: &str = "Clipboard read failed";
const CLIPBOARD_WRITE_FAILED_NOTICE: &str = "Clipboard write failed";
const MULTILINE_SLASH_PASTE_NOTICE: &str = "Slash commands accept single-line paste only";
const PASTE_TOO_LARGE_NOTICE: &str = "Paste is too large (limit 1 MB)";

/// Copies selected prompt text to the native clipboard.
pub(super) fn copy_effects(
    state: &State,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    if state.app_selection.has_selection() {
        return copy_rendered_selection_effects(state, clipboard);
    }

    let Some(value) = state.session.prompt.selected_text() else {
        return Vec::new();
    };
    let Some(clipboard) = clipboard else {
        return input_notice_effect(CLIPBOARD_UNAVAILABLE_NOTICE);
    };

    if clipboard.set_text(&value).is_err() {
        return input_notice_effect(CLIPBOARD_WRITE_FAILED_NOTICE);
    }

    let mut prompt = state.session.prompt.clone();
    let _ = prompt.copy_selection();
    prompt_changed_if_needed(state, prompt)
}

/// Copies selected rendered app text to the native clipboard.
pub(super) fn copy_rendered_selection_effects(
    state: &State,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let Some(value) = selected_text(state) else {
        return Vec::new();
    };
    let Some(clipboard) = clipboard else {
        return input_notice_effect(CLIPBOARD_UNAVAILABLE_NOTICE);
    };

    if clipboard.set_text(&value).is_err() {
        return input_notice_effect(CLIPBOARD_WRITE_FAILED_NOTICE);
    }

    view_action_effects(ViewAction::RenderedSelectionFeedbackReported {
        message: COPIED_SELECTION_NOTICE.to_owned(),
    })
}

/// Copies selected rendered app text using cached transcript layout metadata.
pub(super) fn copy_rendered_selection_effects_with_layout(
    state: &State,
    layout: &TranscriptLayout,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let Some(value) = selected_text_with_layout(state, layout) else {
        return Vec::new();
    };
    let Some(clipboard) = clipboard else {
        return input_notice_effect(CLIPBOARD_UNAVAILABLE_NOTICE);
    };

    if clipboard.set_text(&value).is_err() {
        return input_notice_effect(CLIPBOARD_WRITE_FAILED_NOTICE);
    }

    view_action_effects(ViewAction::RenderedSelectionFeedbackReported {
        message: COPIED_SELECTION_NOTICE.to_owned(),
    })
}

/// Cuts selected prompt text to the native clipboard.
pub(super) fn cut_effects(
    state: &State,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let Some(value) = state.session.prompt.selected_text() else {
        return Vec::new();
    };
    let Some(clipboard) = clipboard else {
        return input_notice_effect(CLIPBOARD_UNAVAILABLE_NOTICE);
    };

    if clipboard.set_text(&value).is_err() {
        return input_notice_effect(CLIPBOARD_WRITE_FAILED_NOTICE);
    }

    let mut prompt = state.session.prompt.clone();
    let _ = prompt.cut_selection();
    prompt_changed_if_needed(state, prompt)
}

/// Pastes native clipboard text into the prompt when available.
pub(super) fn effects(
    state: &State,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let value = match clipboard_paste_text(clipboard) {
        Ok(value) => value,
        Err(message) => return input_notice_effect(message),
    };
    prompt_text_effects(state, &value)
}

/// Handles terminal-native paste events using the terminal-provided bytes.
pub(super) fn terminal_effects(
    state: &State,
    value: &str,
    paste_burst: &mut PasteBurst,
) -> Vec<EventEffect> {
    paste_burst.clear();
    if let Some(selection) = state.selection.as_ref() {
        return selection_text_effects(selection, value);
    }

    prompt_text_effects(state, value)
}

/// Pastes clipboard text into the active selection prompt field when one is editable.
pub(super) fn selection_effects(
    selection: &SelectionPromptState,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let value = match clipboard_paste_text(clipboard) {
        Ok(value) => value,
        Err(message) => return input_notice_effect(message),
    };
    selection_text_effects(selection, &value)
}

/// Inserts caller-supplied paste text into the main prompt after sanitization.
fn prompt_text_effects(state: &State, value: &str) -> Vec<EventEffect> {
    if value.len() > super::MAX_PASTE_BYTES {
        return input_notice_effect(PASTE_TOO_LARGE_NOTICE);
    }
    let value = sanitize_text(value);
    if value.is_empty() {
        return Vec::new();
    }
    if is_multiline_slash_paste(state, &value) {
        return input_notice_effect(MULTILINE_SLASH_PASTE_NOTICE);
    }

    prompt_change_effect(state, |prompt| prompt.insert_paste(&value))
}

/// Inserts caller-supplied paste text into the active selection prompt field.
fn selection_text_effects(selection: &SelectionPromptState, value: &str) -> Vec<EventEffect> {
    if value.len() > super::MAX_PASTE_BYTES {
        return input_notice_effect(PASTE_TOO_LARGE_NOTICE);
    }

    let value = sanitize_text(value).replace('\n', " ");
    if value.is_empty() {
        return Vec::new();
    }

    super::selection::change_effect(selection, |selection| selection.insert_text(&value))
}

/// Reads clipboard text for a paste request, separating unavailable from read failures.
fn clipboard_paste_text(
    clipboard: Option<&mut dyn ClipboardService>,
) -> Result<String, &'static str> {
    let Some(clipboard) = clipboard else {
        return Err(CLIPBOARD_UNAVAILABLE_NOTICE);
    };

    clipboard
        .get_text()
        .map_err(|_| CLIPBOARD_READ_FAILED_NOTICE)
}

/// Returns a prompt-safe paste string with normalized newlines and supported controls only.
fn sanitize_text(value: &str) -> String {
    crate::prompt::normalize_paste(value)
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

/// Returns true when a multiline paste would split a leading slash command token.
fn is_multiline_slash_paste(state: &State, value: &str) -> bool {
    if !value.contains('\n') {
        return false;
    }

    let text = state.session.prompt.text();
    crate::prompt::slash_command_query(&text, state.session.prompt.cursor).is_some()
        || crate::prompt::has_command_context(&state.session.prompt)
}
