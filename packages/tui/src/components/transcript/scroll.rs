use crate::scroll::TranscriptScrollState;
use iocraft::prelude::*;

/// Scrollable transcript viewport that preserves Doric's bottom-relative scroll behavior.
#[component]
pub fn Scroll<'a>(_hooks: Hooks, props: &mut ScrollProps<'a>) -> impl Into<AnyElement<'a>> {
    let total_rows = props.total_rows;
    let visible_rows = props.visible_rows;
    let scroll_offset = props.scroll_offset.unwrap_or_default();
    let slice_start_row = props.slice_start_row;
    let children = std::mem::take(&mut props.children);
    let top_offset = slice_top_offset(slice_start_row, scroll_offset);
    let scrollbar_marks = scrollbar_marks(total_rows, visible_rows, props.scroll_offset_from_tail);
    let scrollbar = (!scrollbar_marks.is_empty()).then_some(element!(TranscriptScrollbar(
        marks: scrollbar_marks
    )));

    element!(View(
        flex_direction: FlexDirection::Row,
        width: 100pct,
        height: visible_rows,
        overflow: Overflow::Hidden,
    ) {
        View(
            flex_basis: FlexBasis::Length(0),
            flex_grow: 1.0,
            min_width: 0,
            height: 100pct,
            overflow: Overflow::Hidden,
        ) {
            View(
                position: Position::Absolute,
                top: top_offset,
                flex_direction: FlexDirection::Column,
                width: 100pct,
                min_width: 0,
                overflow: Overflow::Hidden,
            ) {
                #(children.into_iter())
            }
        }
        #(scrollbar)
    })
}

/// Props for the scroll viewport.
#[derive(Default, Props)]
pub struct ScrollProps<'a> {
    pub children: Vec<AnyElement<'a>>,
    pub scroll_offset: Option<usize>,
    pub scroll_offset_from_tail: u32,
    pub slice_start_row: usize,
    pub total_rows: usize,
    pub visible_rows: u16,
}

/// Component-owned transcript scroll position derived from layout rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptViewportState {
    pub(crate) offset: u32,
    pub(crate) follow_tail: bool,
    total_rows: usize,
}

impl TranscriptViewportState {
    /// Creates component viewport state from an externally supplied scroll snapshot.
    pub(crate) fn from_scroll(scroll: &TranscriptScrollState, total_rows: usize) -> Self {
        Self {
            offset: scroll.offset,
            follow_tail: scroll.follow_tail,
            total_rows,
        }
    }

    /// Returns viewport state normalized to the current rendered row count and height.
    pub(crate) fn with_render_context(self, total_rows: usize, visible_rows: u16) -> Self {
        let offset = self.render_offset(total_rows, visible_rows);
        Self {
            offset,
            follow_tail: offset == 0,
            total_rows,
        }
    }

    /// Computes the scroll offset to render for the current row count and viewport height.
    fn render_offset(self, total_rows: usize, visible_rows: u16) -> u32 {
        if self.follow_tail {
            return 0;
        }

        let row_growth =
            u32::try_from(total_rows.saturating_sub(self.total_rows)).unwrap_or(u32::MAX);
        self.offset
            .saturating_add(row_growth)
            .min(max_scroll_offset(total_rows, visible_rows))
    }
}

/// Converts a bottom-relative transcript offset into a top-relative layout offset.
pub(crate) fn scroll_offset_from_top(total_rows: usize, visible_rows: u16, offset: u32) -> usize {
    let max_offset = max_scroll_offset(total_rows, visible_rows);
    let offset_from_top = max_offset.saturating_sub(offset.min(max_offset));

    usize::try_from(offset_from_top).unwrap_or(usize::MAX)
}

/// Computes the visible slice's absolute position inside the viewport.
fn slice_top_offset(slice_start_row: usize, scroll_offset: usize) -> i32 {
    let relative_offset = i64::try_from(slice_start_row).unwrap_or(i64::MAX)
        - i64::try_from(scroll_offset).unwrap_or(i64::MAX);

    i32::try_from(relative_offset).unwrap_or_else(|_| {
        if relative_offset.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

/// Returns one scrollbar cell per transcript viewport row when overflow exists.
fn scrollbar_marks(total_rows: usize, visible_rows: u16, offset: u32) -> Vec<ScrollbarMark> {
    if visible_rows == 0 || total_rows <= usize::from(visible_rows) {
        return Vec::new();
    }

    let visible_rows = usize::from(visible_rows);
    let max_offset = max_scroll_offset(total_rows, u16::try_from(visible_rows).unwrap_or(u16::MAX));
    let thumb_height = scrollbar_thumb_height(total_rows, visible_rows);
    let thumb_top = scrollbar_thumb_top(
        offset.min(max_offset),
        max_offset,
        visible_rows,
        thumb_height,
    );

    (0..visible_rows)
        .map(|row| ScrollbarMark {
            thumb: row >= thumb_top && row < thumb_top + thumb_height,
        })
        .collect()
}

/// Returns the scrollbar thumb height for the visible fraction of the transcript.
fn scrollbar_thumb_height(total_rows: usize, visible_rows: usize) -> usize {
    let minimum_thumb_height = visible_rows.min(2);

    visible_rows
        .saturating_mul(visible_rows)
        .saturating_add(total_rows.saturating_sub(1))
        .checked_div(total_rows)
        .unwrap_or(minimum_thumb_height)
        .clamp(minimum_thumb_height, visible_rows)
}

/// Returns the top row for a scrollbar thumb from the bottom-relative transcript offset.
fn scrollbar_thumb_top(
    offset: u32,
    max_offset: u32,
    visible_rows: usize,
    thumb_height: usize,
) -> usize {
    let scrollable_rows = visible_rows.saturating_sub(thumb_height);
    if max_offset == 0 || scrollable_rows == 0 {
        return scrollable_rows;
    }

    let offset_from_top = max_offset.saturating_sub(offset);
    let numerator = u128::from(offset_from_top) * scrollable_rows as u128;
    let denominator = u128::from(max_offset);

    usize::try_from((numerator + denominator / 2) / denominator)
        .unwrap_or(scrollable_rows)
        .min(scrollable_rows)
}

/// Renders the transcript scrollbar as one fixed-width IOCraft layout column.
#[derive(Default, Props)]
struct TranscriptScrollbarProps {
    marks: Vec<ScrollbarMark>,
}

#[component]
fn TranscriptScrollbar(props: &TranscriptScrollbarProps) -> impl Into<AnyElement<'static>> {
    let marks = props.marks.clone();

    element!(View(
        flex_direction: FlexDirection::Column,
        width: 1,
        min_width: 1,
        max_width: 1,
        height: 100pct,
        flex_shrink: 0.0,
        overflow: Overflow::Hidden,
    ) {
        #(marks.into_iter().enumerate().map(|(row, mark)| element!(Text(
            key: row,
            content: mark.glyph().to_string(),
            color: mark.color(),
            wrap: TextWrap::NoWrap,
        ))))
    })
}

/// One rendered scrollbar row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScrollbarMark {
    thumb: bool,
}

impl ScrollbarMark {
    /// Returns the glyph used for this scrollbar row.
    fn glyph(self) -> &'static str {
        if self.thumb {
            return "┃";
        }

        "│"
    }

    /// Returns the color used for this scrollbar row.
    fn color(self) -> Color {
        if self.thumb {
            Color::Rgb {
                r: 71,
                g: 85,
                b: 105,
            }
        } else {
            Color::Rgb {
                r: 30,
                g: 41,
                b: 59,
            }
        }
    }
}

/// Returns the maximum valid rendered-row offset from the transcript tail.
fn max_scroll_offset(total_rows: usize, visible_rows: u16) -> u32 {
    u32::try_from(total_rows)
        .unwrap_or(u32::MAX)
        .saturating_sub(u32::from(visible_rows))
}
