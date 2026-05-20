use crate::components::transcript_projection::TranscriptLayout;
use crate::components::transcript_scroll_view::{
    scroll_offset_from_top, transcript_scroll_delta, TranscriptScrollView, TranscriptViewportState,
};
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
    let content_width = transcript_content_width(width);
    let layout = TranscriptLayout::for_state(&state, content_width);
    let height = transcript_height(layout.total_rows, capacity);
    let scroll = state.scroll.clone();
    let selection_active = state.selection.is_some();
    let mut viewport =
        hooks.use_state(|| TranscriptViewportState::from_scroll(&scroll, layout.total_rows));
    let normalized = viewport
        .get()
        .with_render_context(layout.total_rows, height);

    hooks.use_terminal_events({
        let mut viewport = viewport;
        move |event| {
            let Some(delta) = transcript_scroll_delta(event, height, selection_active) else {
                return;
            };

            let mut next = viewport
                .get()
                .with_render_context(layout.total_rows, height);
            next.scroll_by(delta, layout.total_rows, height);
            viewport.set(next);
        }
    });

    hooks.use_effect(
        move || {
            viewport.set(normalized);
        },
        (
            normalized.offset,
            normalized.follow_tail,
            layout.total_rows,
            height,
        ),
    );

    let scroll_offset = scroll_offset_from_top(layout.total_rows, height, normalized.offset);
    let window = transcript_virtual_window(layout.total_rows, height, scroll_offset);
    let item_range = layout.item_range(window);
    let slice_start_row = layout.item_start_row(item_range.start);
    let items = transcript_item_elements(&state, item_range);

    element!(TranscriptScrollView(
        key: state.session.id.as_str().to_owned(),
        scroll_offset: scroll_offset,
        scroll_offset_from_tail: normalized.offset,
        slice_start_row: slice_start_row,
        total_rows: layout.total_rows,
        visible_rows: height,
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

/// Returns the transcript content width after reserving the scrollbar column.
fn transcript_content_width(width: u16) -> usize {
    let content_width = width.saturating_sub(1);
    if content_width == 0 {
        return usize::MAX;
    }

    usize::from(content_width)
}

/// Returns the transcript pane height, growing until content reaches capacity.
fn transcript_height(total_rows: usize, capacity: u16) -> u16 {
    u16::try_from(total_rows).unwrap_or(u16::MAX).min(capacity)
}

/// Returns the overscanned virtual row window to materialize for the viewport.
fn transcript_virtual_window(
    total_rows: usize,
    visible_rows: u16,
    scroll_offset: usize,
) -> std::ops::Range<usize> {
    let visible_rows = usize::from(visible_rows);
    let visible_start = scroll_offset.min(total_rows);
    let visible_end = visible_start.saturating_add(visible_rows).min(total_rows);
    let overscan = visible_rows.max(1);

    visible_start.saturating_sub(overscan)..visible_end.saturating_add(overscan).min(total_rows)
}

/// Builds keyed child component elements for the visible semantic transcript item range.
fn transcript_item_elements(
    state: &State,
    item_range: std::ops::Range<usize>,
) -> Vec<AnyElement<'static>> {
    state
        .session
        .transcript
        .iter()
        .skip(item_range.start)
        .take(item_range.end.saturating_sub(item_range.start))
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
