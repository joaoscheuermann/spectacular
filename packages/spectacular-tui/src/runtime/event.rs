use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::runtime::{system_clipboard, ClipboardService, PasteBurst};
use crate::session::{PromptState, SelectionPromptState};
use crate::state::State;
use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use std::time::{Duration, Instant};

pub const SPINNER_TICK_INTERVAL: Duration = Duration::from_millis(90);
pub const MAX_PASTE_BYTES: usize = 1_000_000;

const CLIPBOARD_UNAVAILABLE_NOTICE: &str = "Clipboard is unavailable";
const CLIPBOARD_READ_FAILED_NOTICE: &str = "Clipboard read failed";
const CLIPBOARD_WRITE_FAILED_NOTICE: &str = "Clipboard write failed";
const MULTILINE_SLASH_PASTE_NOTICE: &str = "Slash commands accept single-line paste only";
const PASTE_TOO_LARGE_NOTICE: &str = "Paste is too large (limit 1 MB)";

/// Effect requested by local TUI event handling without performing side effects directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventEffect {
    Action(Box<ChatTuiAction>),
    RequestExit,
}

/// Wraps a reducer action into an event effect without inflating enum size.
fn action_effects(action: ChatTuiAction) -> Vec<EventEffect> {
    vec![EventEffect::Action(Box::new(action))]
}

/// Builds a reducer-owned, prompt-local input notice effect.
fn input_notice_effect(message: impl Into<String>) -> Vec<EventEffect> {
    action_effects(ChatTuiAction::InputNoticeReported {
        message: message.into(),
    })
}

/// Converts one IOCraft terminal event into reducer actions or outer-shell requests.
pub fn effects(state: &State, event: TerminalEvent) -> Vec<EventEffect> {
    let mut paste_burst = PasteBurst::default();
    match event {
        TerminalEvent::Key(key) => {
            let mut clipboard = if key_needs_system_clipboard(state, &key) {
                system_clipboard()
            } else {
                None
            };
            let clipboard = clipboard
                .as_mut()
                .map(|value| value.as_mut() as &mut dyn ClipboardService);
            key_effects(state, key, clipboard, &mut paste_burst, Instant::now())
        }
        TerminalEvent::Paste(value) => terminal_paste_effects(state, &value, &mut paste_burst),
        _ => Vec::new(),
    }
}

/// Converts one terminal event using an injected clipboard service for testable copy/paste behavior.
pub fn effects_with_clipboard(
    state: &State,
    event: TerminalEvent,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let mut paste_burst = PasteBurst::default();
    effects_with_clipboard_and_paste(state, event, clipboard, &mut paste_burst)
}

/// Converts one terminal event using caller-owned clipboard and paste-burst state.
pub(crate) fn effects_with_clipboard_and_paste(
    state: &State,
    event: TerminalEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
) -> Vec<EventEffect> {
    match event {
        TerminalEvent::Key(key) => key_effects(state, key, clipboard, paste_burst, Instant::now()),
        TerminalEvent::Paste(value) => terminal_paste_effects(state, &value, paste_burst),
        _ => Vec::new(),
    }
}

/// Returns true when the key path may need native clipboard access.
fn key_needs_system_clipboard(state: &State, key: &KeyEvent) -> bool {
    if key.kind == KeyEventKind::Release {
        return false;
    }
    if state.selection.is_some() {
        return is_ctrl_char(key, 'v');
    }
    if is_ctrl_char(key, 'v') {
        return true;
    }
    if is_ctrl_char(key, 'c') || is_ctrl_char(key, 'x') {
        return state.session.prompt.selection_range().is_some();
    }

    false
}

/// Returns the effects emitted by the fixed-cadence spinner timer source.
pub fn timer_tick_effects() -> Vec<EventEffect> {
    action_effects(ChatTuiAction::SpinnerTick)
}

/// Converts one key event into a reducer action or shell-level exit request.
fn key_effects(
    state: &State,
    key: KeyEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
    now: Instant,
) -> Vec<EventEffect> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }

    if state.selection.is_some() {
        return selection_key_effects(state, key, clipboard, paste_burst, now);
    }

    if is_ctrl_char(&key, 'q') {
        paste_burst.clear();
        return vec![EventEffect::RequestExit];
    }

    if is_ctrl_char(&key, 'c') {
        paste_burst.clear();
        return copy_effects(state, clipboard);
    }
    if is_ctrl_char(&key, 'x') {
        paste_burst.clear();
        return cut_effects(state, clipboard);
    }
    if is_ctrl_char(&key, 'v') {
        paste_burst.clear();
        return paste_effects(state, clipboard);
    }
    if is_ctrl_char(&key, 'z') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::undo);
    }
    if is_ctrl_char(&key, 'r') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::redo);
    }
    if is_ctrl_char(&key, 'a') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::select_all);
    }
    if is_ctrl_char(&key, 'u') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::kill_to_line_start);
    }
    if is_ctrl_char(&key, 'k') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::kill_to_line_end);
    }
    if is_ctrl_char(&key, 'y') {
        paste_burst.clear();
        return prompt_change_effect(state, PromptState::yank);
    }
    if let Some(line_break) = line_break_char_key(&key) {
        paste_burst.note_line_break(now);
        return prompt_change_effect(state, |prompt| prompt.insert_text(line_break));
    }
    if is_submit_key(&key) {
        if paste_burst.should_insert_line_break(now) {
            paste_burst.note_line_break(now);
            return prompt_change_effect(state, |prompt| prompt.insert_text("\n"));
        }
        paste_burst.clear();
        return submit_or_compose_command_effects(state);
    }
    if is_line_break_key(&key) {
        if paste_burst.should_insert_line_break(now) {
            paste_burst.note_line_break(now);
            return prompt_change_effect(state, |prompt| prompt.insert_text("\n"));
        }
        paste_burst.clear();
        if crate::prompt::has_command_context(&state.session.prompt) {
            return submit_or_compose_command_effects(state);
        }
        return prompt_change_effect(state, PromptState::insert_newline);
    }

    let selecting = is_shift_arrow(&key) || key.modifiers.contains(KeyModifiers::SHIFT);
    let by_word = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Tab => {
            paste_burst.clear();
            tab_effects(state)
        }
        KeyCode::Esc => {
            paste_burst.clear();
            escape_effects(state)
        }
        KeyCode::Char(' ') => {
            paste_burst.note_text_key(now);
            space_effects(state)
        }
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            if is_paste_burst_text_key(&key) {
                paste_burst.note_text_key(now);
            } else {
                paste_burst.clear();
            }
            prompt_change_effect(state, |prompt| {
                prompt.insert_character(character);
            })
        }
        KeyCode::Backspace if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::delete_previous_word)
        }
        KeyCode::Delete if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::delete_next_word)
        }
        KeyCode::Backspace => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::backspace)
        }
        KeyCode::Delete => {
            paste_burst.clear();
            prompt_change_effect(state, PromptState::delete_forward)
        }
        KeyCode::Left if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_word_left(selecting))
        }
        KeyCode::Right if by_word => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_word_right(selecting))
        }
        KeyCode::Left => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_left(selecting))
        }
        KeyCode::Right => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_right(selecting))
        }
        KeyCode::Up => {
            paste_burst.clear();
            prompt_up_effects(state, selecting)
        }
        KeyCode::Down => {
            paste_burst.clear();
            prompt_down_effects(state, selecting)
        }
        KeyCode::Home if key.modifiers.contains(KeyModifiers::SHIFT) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_start(selecting))
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::SHIFT) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_end(selecting))
        }
        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_start(selecting))
        }
        KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_to_end(selecting))
        }
        KeyCode::Home => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_line_start(selecting))
        }
        KeyCode::End => {
            paste_burst.clear();
            prompt_change_effect(state, |prompt| prompt.move_line_end(selecting))
        }
        _ => {
            paste_burst.clear();
            Vec::new()
        }
    }
}

/// Builds Escape behavior from current status without mutating state directly.
fn escape_effects(state: &State) -> Vec<EventEffect> {
    let dismissed = prompt_change_effect(state, |prompt| {
        crate::prompt::dismiss_command_suggestions(prompt, &state.commands);
    });
    if !dismissed.is_empty() {
        return dismissed;
    }

    if state.status.is_cancellable() {
        return action_effects(ChatTuiAction::CancelRun);
    }

    if state.session.prompt.is_empty() {
        return vec![EventEffect::RequestExit];
    }

    prompt_change_effect(state, PromptState::escape)
}

/// Copies selected prompt text to the native clipboard.
fn copy_effects(state: &State, clipboard: Option<&mut dyn ClipboardService>) -> Vec<EventEffect> {
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

/// Cuts selected prompt text to the native clipboard.
fn cut_effects(state: &State, clipboard: Option<&mut dyn ClipboardService>) -> Vec<EventEffect> {
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
fn paste_effects(state: &State, clipboard: Option<&mut dyn ClipboardService>) -> Vec<EventEffect> {
    let value = match clipboard_paste_text(clipboard) {
        Ok(value) => value,
        Err(message) => return input_notice_effect(message),
    };
    prompt_paste_text_effects(state, &value)
}

/// Inserts caller-supplied paste text into the main prompt after sanitization.
fn prompt_paste_text_effects(state: &State, value: &str) -> Vec<EventEffect> {
    if value.len() > MAX_PASTE_BYTES {
        return input_notice_effect(PASTE_TOO_LARGE_NOTICE);
    }
    let value = sanitize_paste_text(&value);
    if value.is_empty() {
        return Vec::new();
    }
    if is_multiline_slash_paste(state, &value) {
        return input_notice_effect(MULTILINE_SLASH_PASTE_NOTICE);
    }

    prompt_change_effect(state, |prompt| prompt.insert_paste(&value))
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
fn sanitize_paste_text(value: &str) -> String {
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

/// Handles terminal-native paste events using the terminal-provided bytes.
fn terminal_paste_effects(
    state: &State,
    value: &str,
    paste_burst: &mut PasteBurst,
) -> Vec<EventEffect> {
    paste_burst.clear();
    if let Some(selection) = state.selection.as_ref() {
        return selection_paste_text_effects(selection, value);
    }

    prompt_paste_text_effects(state, value)
}

/// Pastes clipboard text into the active selection prompt field when one is editable.
fn selection_paste_effects(
    selection: &SelectionPromptState,
    clipboard: Option<&mut dyn ClipboardService>,
) -> Vec<EventEffect> {
    let value = match clipboard_paste_text(clipboard) {
        Ok(value) => value,
        Err(message) => return input_notice_effect(message),
    };
    selection_paste_text_effects(selection, &value)
}

/// Inserts caller-supplied paste text into the active selection prompt field.
fn selection_paste_text_effects(selection: &SelectionPromptState, value: &str) -> Vec<EventEffect> {
    if value.len() > MAX_PASTE_BYTES {
        return input_notice_effect(PASTE_TOO_LARGE_NOTICE);
    }

    let value = sanitize_paste_text(&value).replace('\n', " ");
    if value.is_empty() {
        return Vec::new();
    }

    selection_change_effect(selection, |selection| selection.insert_text(&value))
}

/// Builds a prompt submission action for non-empty prompt text.
fn submit_prompt_effects(state: &State) -> Vec<EventEffect> {
    let text = state.session.prompt.text().trim().to_owned();
    if text.is_empty() {
        return Vec::new();
    }

    action_effects(ChatTuiAction::SubmitPrompt {
        id: next_local_prompt_id(state),
        text,
    })
}

/// Accepts suggestions or guides slash-command fields before allowing submission.
fn submit_or_compose_command_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_command_suggestion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    let (guided, did_guide) = guide_command_field_effects(state);
    if did_guide {
        return guided;
    }

    submit_prompt_effects(state)
}

/// Applies a local prompt edit and returns a PromptChanged action when state changed.
fn prompt_change_effect<F>(state: &State, edit: F) -> Vec<EventEffect>
where
    F: FnOnce(&mut PromptState),
{
    let mut prompt = state.session.prompt.clone();
    edit(&mut prompt);
    prompt_changed_if_needed(state, prompt)
}

/// Returns a prompt changed action only when prompt state differs.
fn prompt_changed_if_needed(state: &State, prompt: PromptState) -> Vec<EventEffect> {
    if prompt == state.session.prompt {
        return Vec::new();
    }

    action_effects(ChatTuiAction::PromptChanged(prompt))
}

/// Accepts the selected command-composer suggestion into prompt text when available.
fn accept_command_suggestion_effects(state: &State) -> Vec<EventEffect> {
    let suggestions = crate::prompt::command_suggestions(&state.session.prompt, &state.commands);
    let Some(suggestion) = suggestions
        .get(state.session.prompt.selected_completion)
        .filter(|suggestion| suggestion.kind != crate::prompt::CommandSuggestionKind::Info)
        .or_else(|| {
            suggestions
                .iter()
                .find(|suggestion| suggestion.kind != crate::prompt::CommandSuggestionKind::Info)
        })
        .cloned()
    else {
        return Vec::new();
    };

    prompt_change_effect(state, |prompt| {
        crate::prompt::accept_command_suggestion(prompt, &suggestion, &state.commands);
    })
}

/// Guides the command composer to the next invalid or required field.
fn guide_command_field_effects(state: &State) -> (Vec<EventEffect>, bool) {
    let mut prompt = state.session.prompt.clone();
    if !crate::prompt::guide_command_field(&mut prompt, &state.commands) {
        return (Vec::new(), false);
    }

    (prompt_changed_if_needed(state, prompt), true)
}

/// Handles Tab as command completion, command field guidance, or a literal tab outside slash mode.
fn tab_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_command_suggestion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    let (guided, did_guide) = guide_command_field_effects(state);
    if did_guide {
        return guided;
    }

    if crate::prompt::has_command_context(&state.session.prompt) {
        return Vec::new();
    }

    prompt_change_effect(state, |prompt| prompt.insert_text("\t"))
}

/// Accepts a command completion with Space, otherwise guides or inserts a literal space.
fn space_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_command_suggestion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    let (guided, did_guide) = guide_command_field_effects(state);
    if did_guide {
        return guided;
    }

    prompt_change_effect(state, |prompt| prompt.insert_text(" "))
}

/// Moves through slash completions before falling back to prompt cursor up movement.
fn prompt_up_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    if !selecting && selectable_command_suggestion_count(state) > 0 {
        return prompt_change_effect(state, PromptState::select_previous_completion);
    }

    prompt_change_effect(state, |prompt| {
        prompt.move_vertical_with_width(-1, selecting, state.prompt_layout.content_width);
    })
}

/// Moves through slash completions before falling back to prompt cursor down movement.
fn prompt_down_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    let count = selectable_command_suggestion_count(state);
    if !selecting && count > 0 {
        return prompt_change_effect(state, |prompt| prompt.select_next_completion(count));
    }

    prompt_change_effect(state, |prompt| {
        prompt.move_vertical_with_width(1, selecting, state.prompt_layout.content_width);
    })
}

/// Returns the count of selectable command-composer suggestions.
fn selectable_command_suggestion_count(state: &State) -> usize {
    crate::prompt::command_suggestions(&state.session.prompt, &state.commands)
        .into_iter()
        .filter(|suggestion| suggestion.kind != crate::prompt::CommandSuggestionKind::Info)
        .count()
}

/// Handles one key event while a modal selection prompt is active.
fn selection_key_effects(
    state: &State,
    key: KeyEvent,
    clipboard: Option<&mut dyn ClipboardService>,
    paste_burst: &mut PasteBurst,
    now: Instant,
) -> Vec<EventEffect> {
    if is_ctrl_char(&key, 'q') {
        paste_burst.clear();
        return vec![EventEffect::RequestExit];
    }
    if is_ctrl_char(&key, 'c') {
        paste_burst.clear();
        return Vec::new();
    }
    if is_ctrl_char(&key, 'x') {
        paste_burst.clear();
        return Vec::new();
    }

    let Some(selection) = state.selection.as_ref() else {
        return Vec::new();
    };
    if is_ctrl_char(&key, 'v') {
        paste_burst.clear();
        return selection_paste_effects(selection, clipboard);
    }
    if line_break_char_key(&key).is_some() {
        paste_burst.note_line_break(now);
        return selection_line_break_char_effect(selection);
    }
    if key.code == KeyCode::Enter
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && paste_burst.should_insert_line_break(now)
    {
        paste_burst.note_line_break(now);
        return selection_line_break_char_effect(selection);
    }
    match key.code {
        KeyCode::Enter if key.modifiers == KeyModifiers::NONE => {
            paste_burst.clear();
            selection_submit_effect(selection)
        }
        KeyCode::Esc => {
            paste_burst.clear();
            selection_escape_effect(selection)
        }
        KeyCode::Tab => {
            paste_burst.clear();
            selection_change_effect(selection, |selection| {
                selection.toggle_comment_mode();
            })
        }
        KeyCode::Up | KeyCode::Char('k') => {
            paste_burst.clear();
            selection_change_effect(selection, SelectionPromptState::select_previous)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            paste_burst.clear();
            selection_change_effect(selection, SelectionPromptState::select_next)
        }
        KeyCode::Backspace => {
            paste_burst.clear();
            selection_change_effect(selection, SelectionPromptState::backspace)
        }
        KeyCode::Delete => {
            paste_burst.clear();
            selection_change_effect(selection, SelectionPromptState::delete_forward)
        }
        KeyCode::Char(character) if should_insert_char(&key, character) => {
            if is_paste_burst_text_key(&key) {
                paste_burst.note_text_key(now);
            } else {
                paste_burst.clear();
            }
            selection_change_effect(selection, |selection| {
                selection.insert_text(&character.to_string());
            })
        }
        _ => {
            paste_burst.clear();
            Vec::new()
        }
    }
}

/// Converts paste-emitted line-break character keys into single-line selection text.
fn selection_line_break_char_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    selection_change_effect(selection, |selection| selection.insert_text(" "))
}

/// Handles Escape by leaving comment mode or cancelling the modal selection prompt.
fn selection_escape_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    let mut next = selection.clone();
    if next.escape() {
        return action_effects(ChatTuiAction::SelectionPromptCancelled);
    }

    if &next == selection {
        return Vec::new();
    }

    action_effects(ChatTuiAction::SelectionPromptChanged(Some(next)))
}

/// Builds a selection prompt submit action when the selected answer is valid.
fn selection_submit_effect(selection: &SelectionPromptState) -> Vec<EventEffect> {
    let Some(answer) = selection.answer() else {
        return Vec::new();
    };

    action_effects(ChatTuiAction::SelectionPromptSubmitted(answer))
}

/// Applies a local selection prompt edit and returns an action when state changed.
fn selection_change_effect<F>(selection: &SelectionPromptState, edit: F) -> Vec<EventEffect>
where
    F: FnOnce(&mut SelectionPromptState),
{
    let mut next = selection.clone();
    edit(&mut next);
    if &next == selection {
        return Vec::new();
    }

    action_effects(ChatTuiAction::SelectionPromptChanged(Some(next)))
}

/// Returns true for printable key events without control or alt chords.
fn should_insert_char(key: &KeyEvent, character: char) -> bool {
    !character.is_control()
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Returns true for text keys that can be part of an unbracketed paste burst.
fn is_paste_burst_text_key(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char(character) if !character.is_control())
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Allocates a deterministic local prompt ID from current semantic transcript length.
fn next_local_prompt_id(state: &State) -> TranscriptItemId {
    TranscriptItemId::new(format!(
        "local-prompt-{}",
        state.session.transcript.len().saturating_add(1)
    ))
}

/// Returns true when a key event should submit the active prompt.
fn is_submit_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Returns true when a key event should insert a multiline prompt line break.
fn is_line_break_key(key: &KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return false;
    }

    key.code == KeyCode::Enter
}

/// Returns normalized text for raw line-break character key events emitted by paste streams.
fn line_break_char_key(key: &KeyEvent) -> Option<&'static str> {
    match key.code {
        KeyCode::Char('\n' | '\r') => Some("\n"),
        _ => None,
    }
}

/// Returns true when a key event is a Shift+arrow selection chord.
fn is_shift_arrow(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::SHIFT)
        && matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
        )
}

/// Returns true when a key event is a specific Ctrl+character chord.
fn is_ctrl_char(key: &KeyEvent, expected: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(character) if character.eq_ignore_ascii_case(&expected))
}
