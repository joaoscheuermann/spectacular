use crate::components::{
    footer_render_line_with_width, input_notice_render_line, prompt_render_lines,
    selection_prompt_render_lines, transcript_render_lines, working_render_line, Footer,
    InputNotice, Prompt, SelectionPrompt, Transcript, Working,
};
use crate::render::{RenderLine, RenderStyle};
use crate::state::State;
use crate::status::Status;
use iocraft::prelude::*;
use std::sync::Arc;

/// Composes the full-screen application layout from owned state for runtime rendering.
#[component]
pub fn App(mut hooks: Hooks, props: &AppProps) -> impl Into<AnyElement<'static>> {
    let (terminal_width, terminal_height) = hooks.use_terminal_size();

    let width = props.width.unwrap_or(terminal_width);
    let height = props.height.or_else(|| non_zero_size(terminal_height));

    let mut state = props.state.clone();

    let Some(height) = height else {
        return element!(View(width)).into_any();
    };

    state.prompt_layout = crate::state::PromptLayoutMetrics::from_terminal_size(width, height);
    let transcript_capacity = transcript_capacity_rows(&state, height, Some(width));
    state.scroll.visible_rows = u32::from(transcript_capacity);
    let has_working_row = status_has_working_row(&state.status);
    let has_input_notice = state.input_notice.is_some();
    let state = Arc::new(state);
    let transcript =
        transcript_element(Arc::clone(&state), transcript_capacity, Some(width)).into_any();
    let working = has_working_row.then(|| working_element(Arc::clone(&state)).into_any());
    let input_notice =
        has_input_notice.then(|| input_notice_element(Arc::clone(&state)).into_any());
    let selection_prompt = state
        .selection
        .is_some()
        .then(|| selection_prompt_element(Arc::clone(&state)).into_any());
    let prompt = state
        .selection
        .is_none()
        .then(|| prompt_element(Arc::clone(&state), Some(width)).into_any());
    let footer = footer_element(state, Some(width)).into_any();

    element!(View(flex_direction: FlexDirection::Column, width, height) {
        #(transcript)
        #(working)
        View(flex_direction: FlexDirection::Column, width: 100pct, flex_shrink: 0.0) {
            #(input_notice)
            #(selection_prompt)
            #(prompt)
            #(footer)
        }
    })
    .into_any()
}

/// Builds a type-safe App element with required state supplied by the caller.
pub fn app_element(state: State, width: Option<u16>, height: Option<u16>) -> Element<'static, App> {
    Element {
        key: ElementKey::new("tui-app"),
        props: AppProps {
            state,
            width,
            height,
        },
    }
}

/// Formats the complete visible app projection as semantic render rows.
///
/// This is a compatibility adapter for parity tests and legacy callers. Live
/// fullscreen rendering should compose the IOCraft components above instead.
pub fn app_render_lines(state: &State) -> Vec<RenderLine> {
    let mut lines = transcript_render_lines(state);
    if let Some(working) = working_render_line(state) {
        lines.push(working);
        lines.push(RenderLine::styled("", RenderStyle::Text));
    }
    if let Some(notice) = input_notice_render_line(state) {
        lines.push(notice);
    }
    lines.extend(input_render_lines(state));
    lines.push(RenderLine::styled("", RenderStyle::Text));
    lines.push(footer_render_line_with_width(
        state,
        footer_width_from_state(state),
    ));
    lines
}

/// Formats the complete visible app projection using original chat UI text shapes.
pub(crate) fn app_lines(state: &State) -> Vec<String> {
    crate::components::plain_lines(app_render_lines(state))
}

/// Returns rows available to transcript content after fixed chrome is accounted for.
pub(crate) fn transcript_capacity_rows(state: &State, height: u16, width: Option<u16>) -> u16 {
    let working_rows = usize::from(status_has_working_row(&state.status)) * 2;
    let notice_rows = usize::from(state.input_notice.is_some());
    let chrome_rows = input_surface_row_count(state, width)
        .saturating_add(working_rows)
        .saturating_add(notice_rows)
        .saturating_add(2);
    let chrome_rows = u16::try_from(chrome_rows).unwrap_or(u16::MAX);

    height.saturating_sub(chrome_rows)
}

/// Formats the active input surface, choosing command selection over free-form prompt input.
///
/// This remains a narrow compatibility adapter for flattened app projections.
fn input_render_lines(state: &State) -> Vec<RenderLine> {
    if state.selection.is_some() {
        return selection_prompt_render_lines(state);
    }

    prompt_render_lines(state)
}

fn input_surface_row_count(state: &State, width: Option<u16>) -> usize {
    if state.selection.is_some() {
        return crate::components::selection_prompt_row_count(state);
    }

    crate::components::prompt_row_count(state, width)
}

/// Props for the full-screen root application component.
#[derive(Props)]
pub struct AppProps {
    pub state: State,
    pub width: Option<u16>,
    pub height: Option<u16>,
}

fn transcript_element(
    state: Arc<State>,
    capacity: u16,
    width: Option<u16>,
) -> Element<'static, Transcript> {
    Element {
        key: ElementKey::new("tui-transcript"),
        props: crate::components::TranscriptProps {
            state,
            capacity,
            width,
        },
    }
}

fn working_element(state: Arc<State>) -> Element<'static, Working> {
    Element {
        key: ElementKey::new("tui-working"),
        props: crate::components::WorkingProps { state },
    }
}

fn input_notice_element(state: Arc<State>) -> Element<'static, InputNotice> {
    Element {
        key: ElementKey::new("tui-input-notice"),
        props: crate::components::InputNoticeProps { state },
    }
}

fn selection_prompt_element(state: Arc<State>) -> Element<'static, SelectionPrompt> {
    Element {
        key: ElementKey::new("tui-selection-prompt"),
        props: crate::components::SelectionPromptProps { state },
    }
}

fn prompt_element(state: Arc<State>, width: Option<u16>) -> Element<'static, Prompt> {
    Element {
        key: ElementKey::new("tui-prompt"),
        props: crate::components::PromptProps { state, width },
    }
}

fn footer_element(state: Arc<State>, width: Option<u16>) -> Element<'static, Footer> {
    Element {
        key: ElementKey::new("tui-footer"),
        props: crate::components::FooterProps { state, width },
    }
}

/// Returns a terminal size only when IOCraft has observed a real non-zero dimension.
fn non_zero_size(size: u16) -> Option<u16> {
    if size == 0 {
        return None;
    }

    Some(size)
}

fn footer_width_from_state(state: &State) -> u16 {
    u16::try_from(state.prompt_layout.content_width.saturating_add(2))
        .unwrap_or(u16::MAX)
        .max(1)
}

fn status_has_working_row(status: &Status) -> bool {
    matches!(status, Status::Running { .. } | Status::Cancelling)
}
