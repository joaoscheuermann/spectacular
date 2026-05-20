use crate::components::transcript_projection::{
    transcript_total_render_rows, transcript_total_render_rows_for_width,
};
use crate::components::transcript_scroll_view::TranscriptScrollView;
use crate::components::{
    AssistantMessage, Cancellation, Command, Error, Notice, OpeningBanner, Reasoning, Success,
    ToolCall, UserPrompt, Warning, WorkedSummary,
};
use crate::state::State;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders the transcript as scrollable IOCraft item components.
#[component]
pub fn Transcript(mut hooks: Hooks, props: &TranscriptProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("Transcript requires state");
    let capacity = props.capacity.unwrap_or_default();
    let (terminal_width, _) = hooks.use_terminal_size();
    let width = props.width.unwrap_or(terminal_width);
    let total_rows = transcript_layout_rows(&state, width);
    let height = transcript_height(total_rows, capacity);
    let items = transcript_item_elements(&state);

    element!(TranscriptScrollView(
        key: state.session.id.as_str().to_owned(),
        scroll: state.scroll.clone(),
        total_rows: total_rows,
        visible_rows: height,
        selection_active: state.selection.is_some(),
    ) {
        #(items.into_iter())
    })
}

/// Props for the transcript component.
#[derive(Default, Props)]
pub struct TranscriptProps {
    pub state: Option<State>,
    pub capacity: Option<u16>,
    pub width: Option<u16>,
}

/// Returns the estimated laid-out transcript row count for the current width.
fn transcript_layout_rows(state: &State, width: u16) -> usize {
    let content_width = width.saturating_sub(1);
    if content_width == 0 {
        return transcript_total_render_rows(state);
    }

    transcript_total_render_rows_for_width(state, usize::from(content_width))
}

/// Returns the transcript pane height, growing until content reaches capacity.
fn transcript_height(total_rows: usize, capacity: u16) -> u16 {
    u16::try_from(total_rows).unwrap_or(u16::MAX).min(capacity)
}

/// Builds keyed child component elements for every semantic transcript item.
fn transcript_item_elements(state: &State) -> Vec<AnyElement<'static>> {
    state
        .session
        .transcript
        .iter()
        .map(transcript_item_element)
        .collect()
}

/// Selects the sibling component that owns rendering for one transcript item.
fn transcript_item_element(item: &TranscriptItem) -> AnyElement<'static> {
    let key = item.id.as_str().to_owned();
    let item = item.clone();

    match &item.content {
        TranscriptItemContent::OpeningBanner(_) => {
            element!(OpeningBanner(key: key, item: item)).into_any()
        }
        TranscriptItemContent::UserPrompt(_) => {
            element!(UserPrompt(key: key, item: item)).into_any()
        }
        TranscriptItemContent::AssistantMessage(_) => {
            element!(AssistantMessage(key: key, item: item)).into_any()
        }
        TranscriptItemContent::Reasoning(_) => element!(Reasoning(key: key, item: item)).into_any(),
        TranscriptItemContent::ToolCall(_) => element!(ToolCall(key: key, item: item)).into_any(),
        TranscriptItemContent::Command(_) => element!(Command(key: key, item: item)).into_any(),
        TranscriptItemContent::Error(_) => element!(Error(key: key, item: item)).into_any(),
        TranscriptItemContent::Warning(_) => element!(Warning(key: key, item: item)).into_any(),
        TranscriptItemContent::Success(_) => element!(Success(key: key, item: item)).into_any(),
        TranscriptItemContent::Notice(_) => element!(Notice(key: key, item: item)).into_any(),
        TranscriptItemContent::Cancellation(_) => {
            element!(Cancellation(key: key, item: item)).into_any()
        }
        TranscriptItemContent::WorkedSummary(_) => {
            element!(WorkedSummary(key: key, item: item)).into_any()
        }
    }
}
