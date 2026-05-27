use super::{action_effects, prompt_change_effect, prompt_changed_if_needed, EventEffect};
use crate::action::ChatTuiAction;
use crate::ids::TranscriptItemId;
use crate::session::PromptState;
use crate::state::State;

/// Accepts suggestions or guides slash-command fields before allowing submission.
pub(super) fn submit_or_compose_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_suggestion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    let (guided, did_guide) = guide_field_effects(state);
    if did_guide {
        return guided;
    }

    submit_prompt_effects(state)
}

/// Handles Tab as command completion, command field guidance, or a literal tab outside slash mode.
pub(super) fn tab_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_suggestion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    let (guided, did_guide) = guide_field_effects(state);
    if did_guide {
        return guided;
    }

    if crate::prompt::has_command_context(&state.session.prompt) {
        return Vec::new();
    }

    prompt_change_effect(state, |prompt| prompt.insert_text("\t"))
}

/// Accepts a command completion with Space, otherwise guides or inserts a literal space.
pub(super) fn space_effects(state: &State) -> Vec<EventEffect> {
    let accepted = accept_suggestion_effects(state);
    if !accepted.is_empty() {
        return accepted;
    }

    let (guided, did_guide) = guide_field_effects(state);
    if did_guide {
        return guided;
    }

    prompt_change_effect(state, |prompt| prompt.insert_text(" "))
}

/// Moves through slash completions before falling back to prompt cursor up movement.
pub(super) fn prompt_up_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    if !selecting && selectable_suggestion_count(state) > 0 {
        return prompt_change_effect(state, PromptState::select_previous_completion);
    }

    prompt_change_effect(state, |prompt| {
        prompt.move_vertical_with_width(-1, selecting, state.prompt_layout.content_width);
    })
}

/// Moves through slash completions before falling back to prompt cursor down movement.
pub(super) fn prompt_down_effects(state: &State, selecting: bool) -> Vec<EventEffect> {
    let count = selectable_suggestion_count(state);
    if !selecting && count > 0 {
        return prompt_change_effect(state, |prompt| prompt.select_next_completion(count));
    }

    prompt_change_effect(state, |prompt| {
        prompt.move_vertical_with_width(1, selecting, state.prompt_layout.content_width);
    })
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

/// Accepts the selected command-composer suggestion into prompt text when available.
fn accept_suggestion_effects(state: &State) -> Vec<EventEffect> {
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
fn guide_field_effects(state: &State) -> (Vec<EventEffect>, bool) {
    let mut prompt = state.session.prompt.clone();
    if !crate::prompt::guide_command_field(&mut prompt, &state.commands) {
        return (Vec::new(), false);
    }

    (prompt_changed_if_needed(state, prompt), true)
}

/// Returns the count of selectable command-composer suggestions.
fn selectable_suggestion_count(state: &State) -> usize {
    crate::prompt::command_suggestions(&state.session.prompt, &state.commands)
        .into_iter()
        .filter(|suggestion| suggestion.kind != crate::prompt::CommandSuggestionKind::Info)
        .count()
}

/// Allocates a deterministic local prompt ID from current semantic transcript length.
fn next_local_prompt_id(state: &State) -> TranscriptItemId {
    TranscriptItemId::new(format!(
        "local-prompt-{}",
        state.session.transcript.len().saturating_add(1)
    ))
}
