use crate::format::{
    footer_render_line, prompt_render_lines, transcript_render_lines, working_render_line,
};
use crate::render_model::{iocraft_content, RenderLine};
use crate::state::State;
use iocraft::prelude::*;

/// Composes the full-screen application layout from owned state for runtime rendering.
#[component]
pub fn AppState(props: &AppStateProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("AppState requires state");
    let working_line = working_render_line(&state);
    let transcript_lines = transcript_render_lines(&state);
    let prompt_lines = prompt_render_lines(&state);
    let footer_line = footer_render_line(&state);

    println!();
    println!("-------------");
    for node in state.session.transcript {
        println!("{:?}", node);
    }

    element!(View() {})

    // element!(AppLayout(
    //     transcript_lines,
    //     working_line,
    //     prompt_lines,
    //     footer_line,
    //     height: 100pct
    // ))
}

/// Props for the owned-state root application component.
#[derive(Default, Props)]
pub struct AppStateProps {
    pub state: Option<State>,
}

/// Composes the read-only application layout from explicit state.
#[component]
pub fn App<'a>(props: &AppProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("App requires state");
    let working_line = working_render_line(state);
    let transcript_lines = transcript_render_lines(state);
    let prompt_lines = prompt_render_lines(state);
    let footer_line = footer_render_line(state);

    println!("{:?}", state.session.transcript);

    element!(View() {})
}

/// Props for the root application component.
#[derive(Default, Props)]
pub struct AppProps<'a> {
    pub state: Option<&'a State>,
}

/// Renders a full application layout from preformatted semantic regions.
#[component]
fn AppLayout(props: &AppLayoutProps) -> impl Into<AnyElement<'static>> {
    let transcript_lines = props.transcript_lines.clone();
    let working_line = props.working_line.clone();
    let prompt_lines = props.prompt_lines.clone();
    let footer_line = props
        .footer_line
        .clone()
        .expect("AppLayout requires footer line");

    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, height: props.height) {
            Body(transcript_lines, working_line)
            Footer(prompt_lines, footer_line)
        }
    }
}

/// Props for the preformatted application layout component.
#[derive(Default, Props)]
struct AppLayoutProps {
    transcript_lines: Vec<RenderLine>,
    working_line: Option<RenderLine>,
    prompt_lines: Vec<RenderLine>,
    footer_line: Option<RenderLine>,
    height: Size,
}

/// Renders the scrollable transcript area and active working indicator.
#[component]
fn Body(props: &BodyProps) -> impl Into<AnyElement<'static>> {
    let transcript_lines = props.transcript_lines.clone();
    let working_line = props.working_line.clone();

    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, flex_grow: 1.0, overflow: Overflow::Hidden) {
            TranscriptScroll(lines: transcript_lines)
            #(working_line_element(working_line))
        }
    }
}

/// Props for the body component.
#[derive(Default, Props)]
struct BodyProps {
    transcript_lines: Vec<RenderLine>,
    working_line: Option<RenderLine>,
}

/// Renders transcript rows inside an IOCraft-managed scroll viewport.
#[component]
fn TranscriptScroll(props: &TranscriptScrollProps) -> impl Into<AnyElement<'static>> {
    let lines = props.lines.clone();
    element! {
        View(width: 100pct, flex_grow: 1.0, overflow: Overflow::Hidden) {
            ScrollView(auto_scroll: true, keyboard_scroll: false, scrollbar: Some(false)) {
                #(render_lines(lines))
            }
        }
    }
}

/// Props for the transcript scroll component.
#[derive(Default, Props)]
struct TranscriptScrollProps {
    lines: Vec<RenderLine>,
}

/// Renders prompt, suggestions, and metadata footer outside the transcript viewport.
#[component]
fn Footer(props: &FooterProps) -> impl Into<AnyElement<'static>> {
    let prompt_lines = props.prompt_lines.clone();
    let footer_line = props
        .footer_line
        .clone()
        .expect("Footer requires footer line");

    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, flex_shrink: 0.0) {
            #(render_lines(prompt_lines))
            #(render_line(footer_line))
        }
    }
}

/// Props for the footer component.
#[derive(Default, Props)]
struct FooterProps {
    prompt_lines: Vec<RenderLine>,
    footer_line: Option<RenderLine>,
}

/// Converts semantic rows into IOCraft elements.
fn render_lines(lines: Vec<RenderLine>) -> Vec<AnyElement<'static>> {
    lines.into_iter().map(render_line).collect()
}

/// Returns the active working indicator row when the app is busy.
fn working_line_element(line: Option<RenderLine>) -> Vec<AnyElement<'static>> {
    let Some(line) = line else {
        return Vec::new();
    };

    vec![render_line(line)]
}

/// Converts one semantic line into an IOCraft mixed-text element.
fn render_line(line: RenderLine) -> AnyElement<'static> {
    let contents = iocraft_content(&line);
    element!(MixedText(wrap: TextWrap::NoWrap, contents)).into()
}
