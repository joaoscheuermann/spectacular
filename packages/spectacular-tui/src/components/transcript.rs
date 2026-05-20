use crate::components::transcript_projection::transcript_total_render_rows;
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
pub fn Transcript(props: &TranscriptProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("Transcript requires state");
    let capacity = props.capacity.unwrap_or_default();
    let items = transcript_item_elements(&state);
    let total_rows = transcript_total_render_rows(&state);

    element!(TranscriptScrollView(
        key: state.session.id.as_str().to_owned(),
        scroll: state.scroll.clone(),
        total_rows: total_rows,
        visible_rows: capacity,
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
