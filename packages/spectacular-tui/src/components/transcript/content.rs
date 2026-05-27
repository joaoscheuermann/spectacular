use super::TranscriptRenderContext;
use crate::render::{
    iocraft_content_with_selection_colors, RenderHighlight, RenderLine, RenderStyle,
};
use crate::selection::{style_line_for_source_with_plan, SelectableSource};
use crate::transcript::DisplayLine;
use iocraft::prelude::*;
use unicode_width::UnicodeWidthStr;

/// Separator used by completed work summaries.
pub const TRANSCRIPT_SEPARATOR: &str = " \u{00b7} ";

/// Builds the shared frame element for one transcript item.
pub(super) fn transcript_item_frame(
    source_id: String,
    rows: Vec<RenderLine>,
    context: TranscriptRenderContext,
    wrap: TranscriptRowWrap,
) -> AnyElement<'static> {
    let key = source_id.clone();

    Element::<TranscriptItemFrame> {
        key: ElementKey::new(key),
        props: TranscriptItemFrameProps {
            source_id,
            rows,
            context,
            wrap,
        },
    }
    .into_any()
}

/// Renders semantic transcript rows with shared selection styling and item spacing.
#[component]
pub(super) fn TranscriptItemFrame(
    props: &TranscriptItemFrameProps,
) -> impl Into<AnyElement<'static>> {
    let context = &props.context;
    let selection_colors = context.selection_colors;
    let source_id = &props.source_id;
    let wrap_mode = props.wrap;
    let elements = props
        .rows
        .iter()
        .cloned()
        .enumerate()
        .map(move |(index, line)| {
            let line = selectable_line(context, source_id, index, line);
            let wrap = wrap_mode.text_wrap(context, &line);
            let contents = iocraft_content_with_selection_colors(&line, selection_colors);
            element!(MixedText(wrap: wrap, contents))
        });

    element!(View(flex_direction: FlexDirection::Column, margin_bottom: 1) { #(elements) })
}

/// Props for the shared transcript item row frame.
#[derive(Props)]
pub(super) struct TranscriptItemFrameProps {
    pub source_id: String,
    pub rows: Vec<RenderLine>,
    pub context: TranscriptRenderContext,
    pub wrap: TranscriptRowWrap,
}

/// Wrapping policy for semantic transcript rows after selection styling.
#[derive(Clone, Copy)]
pub(super) enum TranscriptRowWrap {
    /// Wrap ordinary message rows, except selected trailing spaces that still fit.
    Auto,
    /// Preserve fixed-shape rows such as banners and command/tool output.
    NoWrap,
}

impl TranscriptRowWrap {
    fn text_wrap(self, context: &TranscriptRenderContext, line: &RenderLine) -> TextWrap {
        match self {
            Self::Auto => selectable_text_wrap(context, line),
            Self::NoWrap => TextWrap::NoWrap,
        }
    }
}

/// Flattens semantic rows into plain visible text rows.
pub fn plain_lines(lines: Vec<RenderLine>) -> Vec<String> {
    lines.into_iter().map(|line| line.plain_text()).collect()
}

/// Converts one adapter display line into one semantic render row.
pub fn display_line_render_line(line: &DisplayLine) -> RenderLine {
    if line.spans.is_empty() {
        return RenderLine::styled(&line.text, RenderStyle::from(line.style));
    }

    RenderLine::from_spans(
        line.spans
            .iter()
            .map(|span| crate::render::RenderSpan::new(&span.text, RenderStyle::from(span.style)))
            .collect(),
    )
}

/// Applies app-wide rendered selection styling to a transcript-owned semantic row.
pub fn selectable_line(
    context: &TranscriptRenderContext,
    item_id: &str,
    line_index: usize,
    line: RenderLine,
) -> RenderLine {
    style_line_for_source_with_plan(
        &context.selection_plan,
        line,
        SelectableSource::Transcript {
            item_id: item_id.to_owned(),
            line: line_index,
        },
        0,
        true,
    )
}

/// Chooses wrapping for a transcript row after rendered-selection styling is applied.
pub fn selectable_text_wrap(context: &TranscriptRenderContext, line: &RenderLine) -> TextWrap {
    if has_trailing_selected_spaces(line) && line_width(line) <= transcript_content_width(context) {
        return TextWrap::NoWrap;
    }

    TextWrap::Wrap
}

/// Formats visible lines with one semantic style per row.
pub fn styled_visible_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    visible_lines(text)
        .into_iter()
        .map(|line| RenderLine::styled(line, style))
        .collect()
}

/// Formats non-blank visible lines with one semantic style per row.
pub fn styled_visible_trimmed_lines(text: &str, style: RenderStyle) -> Vec<RenderLine> {
    visible_trimmed_lines(text)
        .into_iter()
        .map(|line| RenderLine::styled(line, style))
        .collect()
}

fn has_trailing_selected_spaces(line: &RenderLine) -> bool {
    line.spans.last().is_some_and(|span| {
        span.highlight == Some(RenderHighlight::Selection)
            && span.text.chars().all(|character| character == ' ')
    })
}

fn transcript_content_width(context: &TranscriptRenderContext) -> usize {
    context.content_width.max(1)
}

fn line_width(line: &RenderLine) -> usize {
    UnicodeWidthStr::width(line.plain_text().as_str())
}

/// Splits non-empty text into visible rows without trimming row content.
fn visible_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    text.lines().map(ToOwned::to_owned).collect()
}

/// Splits text into visible rows only when it contains non-whitespace content.
fn visible_trimmed_lines(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    visible_lines(text)
}
