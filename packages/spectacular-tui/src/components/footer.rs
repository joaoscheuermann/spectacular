use crate::components::TRANSCRIPT_SEPARATOR;
use crate::metadata::{ContextTokenUsage, TokenUsageTotal};
use crate::render::format_directory;
use crate::render::{
    context_pressure_style, iocraft_content_with_selection_colors, RenderLine, RenderSpan,
    RenderStyle,
};
use crate::selection::{style_line_for_source_at_columns, SelectableSource};
use crate::state::State;
use iocraft::prelude::*;
use std::path::Path;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

const DEFAULT_FOOTER_WIDTH: u16 = 120;
const FOOTER_CENTER_SIDE_PADDING: usize = 1;

/// Renders the footer metadata row.
#[component]
pub fn Footer(props: &FooterProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.as_ref();
    let width = props
        .width
        .unwrap_or_else(|| footer_width_from_state(state));
    if footer_center_layout(state, width).is_some() {
        let line = footer_render_line_with_width(state, width);
        let contents = iocraft_content_with_selection_colors(
            &style_line_for_source_at_columns(state, line, SelectableSource::Footer, 0),
            state.selection_colors,
        );
        return element!(View(width: 100pct) {
            MixedText(wrap: TextWrap::NoWrap, contents)
        })
        .into_any();
    }

    let left = footer_left_render_line(state);
    let left_width = UnicodeWidthStr::width(left.plain_text().as_str());
    let left_contents = iocraft_content_with_selection_colors(
        &style_line_for_source_at_columns(state, left, SelectableSource::Footer, 0),
        state.selection_colors,
    );
    let right_contents = footer_right_render_line(state).map(|line| {
        iocraft_content_with_selection_colors(
            &style_line_for_source_at_columns(
                state,
                line,
                SelectableSource::Footer,
                left_width.saturating_add(UnicodeWidthStr::width(TRANSCRIPT_SEPARATOR)),
            ),
            state.selection_colors,
        )
    });

    element!(View(width: 100pct, flex_direction: FlexDirection::Row) {
        MixedText(wrap: TextWrap::NoWrap, contents: left_contents)
        View(flex_grow: 1.0)
        #(right_contents.map(|contents| element!(MixedText(wrap: TextWrap::NoWrap, contents))))
    })
    .into_any()
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

/// Formats the footer row for a known terminal width, centering copy feedback when it fits.
pub fn footer_render_line_with_width(state: &State, width: u16) -> RenderLine {
    let Some(layout) = footer_center_layout(state, width) else {
        return footer_render_line(state);
    };
    let Some(center) = footer_center_render_line(state) else {
        return footer_render_line(state);
    };

    let left = footer_left_render_line(state);
    let left_width = line_width(&left);
    let center_width = line_width(&center);
    let right = footer_right_render_line(state);

    let mut spans = left.spans;
    push_padding(&mut spans, layout.center_start.saturating_sub(left_width));
    spans.extend(center.spans);
    push_padding(
        &mut spans,
        layout
            .right_start
            .saturating_sub(layout.center_start.saturating_add(center_width)),
    );
    if let Some(right) = right {
        spans.extend(right.spans);
    }

    RenderLine::from_spans(spans)
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

/// Formats rendered-selection copy feedback for the center footer zone.
pub fn footer_center_render_line(state: &State) -> Option<RenderLine> {
    if !state.app_selection.has_selection() {
        return None;
    }

    state
        .app_selection
        .copy_feedback
        .as_ref()
        .filter(|message| !message.is_empty())
        .map(|message| RenderLine::styled(message, RenderStyle::Dim))
}

/// Formats the right-aligned footer usage segment when usage is available.
pub fn footer_right_render_line(state: &State) -> Option<RenderLine> {
    footer_usage_segment(state).map(|(text, style)| RenderLine::styled(text, style))
}

/// Formats footer metadata as plain visible text for compatibility tests.
pub fn footer_text(state: &State) -> String {
    footer_render_line(state).plain_text()
}

/// Formats width-aware footer metadata as plain visible text for compatibility tests.
pub fn footer_text_with_width(state: &State, width: u16) -> String {
    footer_render_line_with_width(state, width).plain_text()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FooterCenterLayout {
    center_start: usize,
    right_start: usize,
}

fn footer_center_layout(state: &State, width: u16) -> Option<FooterCenterLayout> {
    let center = footer_center_render_line(state)?;
    let width = usize::from(width).max(1);
    let left_width = line_width(&footer_left_render_line(state));
    let right_width = footer_right_render_line(state)
        .as_ref()
        .map(line_width)
        .unwrap_or(0);
    let right_start = width.checked_sub(right_width)?;
    let gap_width = right_start.checked_sub(left_width)?;
    let center_width = line_width(&center);
    let required_width = center_width.saturating_add(FOOTER_CENTER_SIDE_PADDING * 2);
    if gap_width < required_width {
        return None;
    }

    let center_start = left_width.saturating_add(gap_width.saturating_sub(center_width) / 2);
    let center_end = center_start.saturating_add(center_width);
    if center_start.saturating_sub(left_width) < FOOTER_CENTER_SIDE_PADDING
        || right_start.saturating_sub(center_end) < FOOTER_CENTER_SIDE_PADDING
    {
        return None;
    }

    Some(FooterCenterLayout {
        center_start,
        right_start,
    })
}

fn footer_width_from_state(state: &State) -> u16 {
    u16::try_from(state.prompt_layout.content_width.saturating_add(2))
        .unwrap_or(DEFAULT_FOOTER_WIDTH)
        .max(1)
}

fn push_padding(spans: &mut Vec<RenderSpan>, width: usize) {
    if width == 0 {
        return;
    }

    spans.push(RenderSpan::new(" ".repeat(width), RenderStyle::Dim));
}

fn line_width(line: &RenderLine) -> usize {
    UnicodeWidthStr::width(line.plain_text().as_str())
}

/// Props for the footer component.
#[derive(Props)]
pub struct FooterProps {
    pub state: Arc<State>,
    pub width: Option<u16>,
}
