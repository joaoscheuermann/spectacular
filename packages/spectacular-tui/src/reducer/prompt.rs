use crate::action::ChatTuiAction;
use crate::ids::{SessionId, TranscriptItemId};
use crate::reducer::lookup::{find_content_index_by_id, transcript_contains_id};
use crate::reducer::transcript::append_transcript_item;
use crate::session::{PromptState, Session};
use crate::state::{default_display_context_usage, PromptLayoutMetrics, State};
use crate::transcript::{OpeningBannerItem, TranscriptItemContent, UserPromptItem};

pub(super) fn reduce(state: &mut State, action: ChatTuiAction) {
    match action {
        ChatTuiAction::PromptChanged(mut prompt) => {
            state.input_notice = None;
            ensure_prompt_cursor_visible(&mut prompt, state.prompt_layout);
            state.session.prompt = prompt;
        }
        ChatTuiAction::SubmitPrompt { id, text } => {
            state.input_notice = None;
            upsert_user_prompt(state, id, text);
            state.session.prompt = PromptState::empty();
        }
        ChatTuiAction::SelectionPromptChanged(selection) => {
            state.input_notice = None;
            state.selection = selection;
        }
        ChatTuiAction::SelectionPromptSubmitted(_) | ChatTuiAction::SelectionPromptCancelled => {
            state.input_notice = None;
            state.selection = None;
        }
        ChatTuiAction::CommandsLoaded(commands) => {
            state.commands = commands;
        }
        ChatTuiAction::TranscriptCleared => {
            clear_transcript(state);
        }
        ChatTuiAction::SessionChanged { id } => {
            start_session(state, id);
        }
        ChatTuiAction::SessionCreated { id, banner } => {
            start_session(state, id);
            append_opening_banner(state, banner);
        }
        ChatTuiAction::Resize { width, height } => {
            state.prompt_layout = PromptLayoutMetrics::from_terminal_size(width, height);
            ensure_prompt_cursor_visible(&mut state.session.prompt, state.prompt_layout);
        }
        _ => unreachable!("prompt reducer received non-prompt action"),
    }
}

/// Starts a fresh TUI session while keeping global metadata in sync with it.
fn start_session(state: &mut State, id: SessionId) {
    state.display.session_label = id.as_str().to_owned();
    state.display.context_usage = None;
    state.display.turn_usage = None;
    state.display.total_usage = None;
    default_display_context_usage(&state.runtime, &mut state.display);
    state.session = Session::new(id);
}

/// Clears visible transcript history without changing session metadata.
fn clear_transcript(state: &mut State) {
    state.session.transcript.clear();
    state.session.refresh_next_timestamp();
}

/// Keeps the prompt cursor visible within the current textarea viewport.
fn ensure_prompt_cursor_visible(prompt: &mut PromptState, metrics: PromptLayoutMetrics) {
    prompt.ensure_cursor_visible(metrics.content_width, metrics.viewport_height);
}

/// Inserts a user prompt unless the transcript already contains the prompt occurrence ID.
fn upsert_user_prompt(state: &mut State, id: TranscriptItemId, text: String) {
    if let Some(index) = find_content_index_by_id(state, &id) {
        let TranscriptItemContent::UserPrompt(item) = &mut state.session.transcript[index].content
        else {
            return;
        };
        item.text = text;
        return;
    }

    if transcript_contains_id(state, &id) {
        return;
    }

    append_transcript_item(
        state,
        id,
        TranscriptItemContent::UserPrompt(UserPromptItem::new(text)),
    );
}

/// Appends the session opening banner as the first transcript item.
fn append_opening_banner(state: &mut State, banner: OpeningBannerItem) {
    append_transcript_item(
        state,
        TranscriptItemId::new(format!("opening-banner-{}", state.session.id.as_str())),
        TranscriptItemContent::OpeningBanner(banner),
    );
}
