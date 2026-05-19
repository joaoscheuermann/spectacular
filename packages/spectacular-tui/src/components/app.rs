use crate::components::assistant_message::AssistantMessage;
use crate::components::cancellation::Cancellation;
use crate::components::command::Command;
use crate::components::error::Error;
use crate::components::footer::Footer;
use crate::components::notice::Notice;
use crate::components::opening_banner::OpeningBanner;
use crate::components::prompt_area::PromptArea;
use crate::components::reasoning::Reasoning;
use crate::components::success::Success;
use crate::components::tool_call::ToolCall;
use crate::components::user_prompt::UserPrompt;
use crate::components::warning::Warning;
use crate::components::worked_summary::WorkedSummary;
use crate::components::working_indicator::WorkingIndicator;
use crate::state::State;
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use crate::transcript_window::visible_transcript_range;
use iocraft::prelude::*;

/// Composes the full-screen application layout from owned state for runtime rendering.
#[component]
pub fn App(mut hooks: Hooks, props: &AppProps) -> impl Into<AnyElement<'static>> {
    let (terminal_width, terminal_height) = hooks.use_terminal_size();

    let width = props.width.unwrap_or(terminal_width);
    let height = props.height.or_else(|| non_zero_size(terminal_height));

    let state = props.state.clone().expect("App requires state");

    let transcript_range = visible_transcript_range(state.session.transcript.len(), &state.scroll);

    let transcript_items: Vec<AnyElement<'static>> = state
        .session
        .transcript
        .get(transcript_range)
        .unwrap_or_default()
        .iter()
        .map(transcript_item_element)
        .collect();

    let Some(height) = height else {
        return element!(View(width)).into_any();
    };

    element!(View(flex_direction: FlexDirection::Column, width, height) {
        View(flex_direction: FlexDirection::Column, width: 100pct, flex_grow: 1.0, overflow: Overflow::Hidden) {
            View(width: 100pct, flex_grow: 1.0, overflow: Overflow::Hidden) {
                ScrollView(auto_scroll: true, keyboard_scroll: false, scrollbar: Some(false)) {
                  View(flex_direction: FlexDirection::Column, width: 100pct) {
                    #(transcript_items.into_iter())
                  }
                }
            }
            WorkingIndicator(state: state.clone())
        }
        View(flex_direction: FlexDirection::Column, width: 100pct, flex_shrink: 0.0) {
            PromptArea(state: state.clone())
            Footer(state: state.clone())
        }
    }).into_any()
}

/// Renders one keyed transcript item while preserving its original identity.
fn transcript_item_element(item: &TranscriptItem) -> AnyElement<'static> {
    let key = item.id.as_str().to_owned();
    match &item.content {
        TranscriptItemContent::OpeningBanner(_) => {
            element!(OpeningBanner(key, item: item.clone())).into()
        }
        TranscriptItemContent::UserPrompt(_) => {
            element!(UserPrompt(key, item: item.clone())).into()
        }
        TranscriptItemContent::AssistantMessage(_) => {
            element!(AssistantMessage(key, item: item.clone())).into()
        }
        TranscriptItemContent::Reasoning(_) => element!(Reasoning(key, item: item.clone())).into(),
        TranscriptItemContent::ToolCall(_) => element!(ToolCall(key, item: item.clone())).into(),
        TranscriptItemContent::Command(_) => element!(Command(key, item: item.clone())).into(),
        TranscriptItemContent::Error(_) => element!(Error(key, item: item.clone())).into(),
        TranscriptItemContent::Warning(_) => element!(Warning(key, item: item.clone())).into(),
        TranscriptItemContent::Success(_) => element!(Success(key, item: item.clone())).into(),
        TranscriptItemContent::Notice(_) => element!(Notice(key, item: item.clone())).into(),
        TranscriptItemContent::Cancellation(_) => {
            element!(Cancellation(key, item: item.clone())).into()
        }
        TranscriptItemContent::WorkedSummary(_) => {
            element!(WorkedSummary(key, item: item.clone())).into()
        }
    }
}

/// Props for the full-screen root application component.
#[derive(Default, Props)]
pub struct AppProps {
    pub state: Option<State>,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

/// Returns a terminal size only when IOCraft has observed a real non-zero dimension.
fn non_zero_size(size: u16) -> Option<u16> {
    if size == 0 {
        return None;
    }

    Some(size)
}
