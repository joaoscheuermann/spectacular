use crate::components::transcript_content::TRANSCRIPT_SEPARATOR;
use crate::format_directory::format_directory;
use crate::metadata::ContextTokenUsage;
use crate::render_model::{
    context_usage_style, iocraft_content, RenderLine, RenderSpan, RenderStyle,
};
use crate::state::State;
use iocraft::prelude::*;
use std::path::Path;

/// Renders the footer metadata row.
#[component]
pub fn Footer(props: &FooterProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("Footer requires state");
    let line = footer_render_line(&state);
    let contents = iocraft_content(&line);

    element!(View(width: 100pct) { MixedText(wrap: TextWrap::NoWrap, contents) })
}

/// Formats footer metadata with compact separators and optional usage.
pub fn footer_render_line(state: &State) -> RenderLine {
    let mut spans = vec![
        RenderSpan::new(
            format_directory(Path::new(&state.display.current_directory)),
            RenderStyle::Task,
        ),
        RenderSpan::new(TRANSCRIPT_SEPARATOR, RenderStyle::Dim),
        RenderSpan::new(&state.display.model_label, RenderStyle::Model),
        RenderSpan::new(
            format!(" ({})", state.display.reasoning_label),
            RenderStyle::Dim,
        ),
    ];
    if let Some(usage) = state.session.usage.or(state.display.usage) {
        spans.push(RenderSpan::new(TRANSCRIPT_SEPARATOR, RenderStyle::Dim));
        spans.push(RenderSpan::new(
            usage_text(usage),
            context_usage_style(usage),
        ));
    }

    RenderLine::from_spans(spans)
}

/// Formats footer metadata as plain visible text for compatibility tests.
pub fn footer_text(state: &State) -> String {
    footer_render_line(state).plain_text()
}

/// Formats optional context token usage for status/footer display.
pub fn usage_text(usage: ContextTokenUsage) -> String {
    let Some(window) = usage.context_window_tokens else {
        return format!("{} tks", compact_token_count(usage.input_tokens));
    };
    format!(
        "{}/{} tks",
        compact_token_count(usage.input_tokens),
        compact_token_count(window)
    )
}

/// Formats token counts with compact `k` suffixes for whole thousands.
fn compact_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }

    format!("{}k", tokens / 1_000)
}

/// Props for the footer component.
#[derive(Default, Props)]
pub struct FooterProps {
    pub state: Option<State>,
}
