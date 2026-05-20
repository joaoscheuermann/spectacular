use crate::components::transcript_content::TRANSCRIPT_SEPARATOR;
use crate::format_directory::format_directory;
use crate::metadata::{ContextTokenUsage, TokenUsageTotal};
use crate::render_model::{
    context_pressure_style, iocraft_content, RenderLine, RenderSpan, RenderStyle,
};
use crate::state::State;
use iocraft::prelude::*;
use std::path::Path;

/// Renders the footer metadata row.
#[component]
pub fn Footer(props: &FooterProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("Footer requires state");
    let left_contents = iocraft_content(&footer_left_render_line(&state));
    let right_contents = footer_right_render_line(&state).map(|line| iocraft_content(&line));

    element!(View(width: 100pct, flex_direction: FlexDirection::Row) {
        MixedText(wrap: TextWrap::NoWrap, contents: left_contents)
        View(flex_grow: 1.0)
        #(right_contents.map(|contents| element!(MixedText(wrap: TextWrap::NoWrap, contents))))
    })
}

/// Formats footer metadata with compact separators and optional usage.
pub fn footer_render_line(state: &State) -> RenderLine {
    let mut line = footer_left_render_line(state);
    if let Some(right) = footer_right_render_line(state) {
        line.spans
            .push(RenderSpan::new(TRANSCRIPT_SEPARATOR, RenderStyle::Dim));
        line.spans.extend(right.spans);
    }

    line
}

/// Formats the left-aligned footer metadata segment.
pub fn footer_left_render_line(state: &State) -> RenderLine {
    let mut spans = vec![
        RenderSpan::new(&state.display.session_label, RenderStyle::Dim),
        RenderSpan::new(TRANSCRIPT_SEPARATOR, RenderStyle::Dim),
        RenderSpan::new(
            format_directory(Path::new(&state.display.current_directory)),
            RenderStyle::Dim,
        ),
        RenderSpan::new(TRANSCRIPT_SEPARATOR, RenderStyle::Dim),
        RenderSpan::new(&state.display.model_label, RenderStyle::Dim),
        RenderSpan::new(
            format!(" ({})", state.display.reasoning_label),
            RenderStyle::Dim,
        ),
    ];
    if let Some(worktree) = &state.display.worktree {
        spans.push(RenderSpan::new(TRANSCRIPT_SEPARATOR, RenderStyle::Dim));
        spans.push(RenderSpan::new(&worktree.label, RenderStyle::Dim));
    }

    RenderLine::from_spans(spans)
}

/// Formats the right-aligned footer usage segment when usage is available.
pub fn footer_right_render_line(state: &State) -> Option<RenderLine> {
    footer_usage_segment(state).map(|(text, style)| RenderLine::styled(text, style))
}

/// Formats footer metadata as plain visible text for compatibility tests.
pub fn footer_text(state: &State) -> String {
    footer_render_line(state).plain_text()
}

/// Formats optional context token usage for status/footer display.
pub fn usage_text(usage: ContextTokenUsage) -> String {
    context_usage_text(usage)
}

/// Formats context usage as an explicit estimate fallback.
pub fn context_usage_text(usage: ContextTokenUsage) -> String {
    let Some(window) = usage.context_window_tokens else {
        return format!("~{} ctx", compact_token_count(usage.input_tokens));
    };
    format!(
        "~{}/{} ctx",
        compact_token_count(usage.input_tokens),
        compact_token_count(window)
    )
}

/// Formats accumulated provider usage for compact footer display.
pub fn turn_usage_text(usage: TokenUsageTotal) -> Option<String> {
    token_usage_text_with_context(usage, None)
}

/// Formats accumulated provider usage with context window when available.
fn token_usage_text_with_context(
    usage: TokenUsageTotal,
    context_usage: Option<ContextTokenUsage>,
) -> Option<String> {
    if !usage.has_provider_metadata {
        return None;
    }
    if usage.total_tokens > 0 {
        return Some(total_turn_usage_text(usage.total_tokens, context_usage));
    }
    if usage.input_tokens > 0 && usage.output_tokens > 0 {
        return Some(total_turn_usage_text(
            usage.input_tokens.saturating_add(usage.output_tokens),
            context_usage,
        ));
    }
    if usage.input_tokens > 0 {
        return Some(format!(
            "{} in tks",
            compact_token_count(usage.input_tokens)
        ));
    }
    if usage.output_tokens > 0 {
        return Some(format!(
            "{} out tks",
            compact_token_count(usage.output_tokens)
        ));
    }

    None
}

/// Formats provider turn usage totals while preserving context-window visibility.
fn total_turn_usage_text(tokens: u64, context_usage: Option<ContextTokenUsage>) -> String {
    let Some(context_usage) = context_usage else {
        return format!("{} tks", compact_token_count(tokens));
    };
    let Some(context_window_tokens) = context_usage.context_window_tokens else {
        return format!("{} tks", compact_token_count(tokens));
    };

    format!(
        "{}/{} tks",
        compact_token_count(tokens),
        compact_token_count(context_window_tokens)
    )
}

/// Returns the preferred footer usage segment text and style.
fn footer_usage_segment(state: &State) -> Option<(String, RenderStyle)> {
    let context_usage = state.session.context_usage.or(state.display.context_usage);
    let style = context_pressure_style(context_usage);

    if let Some(total_usage) = state.session.total_usage.or(state.display.total_usage) {
        if let Some(text) = token_usage_text_with_context(total_usage, context_usage) {
            return Some((text, style));
        }
    }

    context_usage.map(|usage| (context_usage_text(usage), style))
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
