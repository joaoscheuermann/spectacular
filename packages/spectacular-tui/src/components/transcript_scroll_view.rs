use crate::scroll::TranscriptScrollState;
use iocraft::prelude::*;
use iocraft::taffy;

/// Scrollable transcript viewport that preserves Spectacular's bottom-relative scroll behavior.
#[component]
pub fn TranscriptScrollView<'a>(
    mut hooks: Hooks,
    props: &mut TranscriptScrollViewProps<'a>,
) -> impl Into<AnyElement<'a>> {
    let scroll = props.scroll.clone().unwrap_or_default();
    let total_rows = props.total_rows;
    let visible_rows = props.visible_rows;
    let selection_active = props.selection_active;
    let children = std::mem::take(&mut props.children);
    let mut viewport =
        hooks.use_state(|| TranscriptViewportState::from_scroll(&scroll, total_rows));
    let normalized = viewport.get().with_render_context(total_rows, visible_rows);

    hooks.use_terminal_events({
        let mut viewport = viewport;
        move |event| {
            let Some(delta) = transcript_scroll_delta(event, visible_rows, selection_active) else {
                return;
            };

            let mut next = viewport.get().with_render_context(total_rows, visible_rows);
            next.scroll_by(delta, total_rows, visible_rows);
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
            normalized.total_rows,
            visible_rows,
        ),
    );

    let top_offset = scroll_offset_from_top(total_rows, visible_rows, normalized.offset);
    let scrollbar_marks = scrollbar_marks(total_rows, visible_rows, normalized.offset);
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
                top: -top_offset,
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

/// Props for the transcript scroll viewport.
#[derive(Default, Props)]
pub struct TranscriptScrollViewProps<'a> {
    pub children: Vec<AnyElement<'a>>,
    pub scroll: Option<TranscriptScrollState>,
    pub total_rows: usize,
    pub visible_rows: u16,
    pub selection_active: bool,
}


/// Component-owned transcript scroll position derived from layout rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TranscriptViewportState {
    offset: u32,
    follow_tail: bool,
    total_rows: usize,
}

impl TranscriptViewportState {
    /// Creates component viewport state from an externally supplied scroll snapshot.
    fn from_scroll(scroll: &TranscriptScrollState, total_rows: usize) -> Self {
        Self {
            offset: scroll.offset,
            follow_tail: scroll.follow_tail,
            total_rows,
        }
    }

    /// Returns viewport state normalized to the current rendered row count and height.
    fn with_render_context(self, total_rows: usize, visible_rows: u16) -> Self {
        let offset = self.render_offset(total_rows, visible_rows);
        Self {
            offset,
            follow_tail: offset == 0,
            total_rows,
        }
    }

    /// Applies a relative scroll delta after first preserving review position across row growth.
    fn scroll_by(&mut self, delta: i32, total_rows: usize, visible_rows: u16) {
        *self = self.with_render_context(total_rows, visible_rows);
        if delta > 0 {
            self.offset = self
                .offset
                .saturating_add(delta.unsigned_abs())
                .min(max_scroll_offset(total_rows, visible_rows));
            self.follow_tail = self.offset == 0;
            return;
        }

        if delta < 0 {
            self.offset = self.offset.saturating_sub(delta.unsigned_abs());
            self.follow_tail = self.offset == 0;
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
fn scroll_offset_from_top(total_rows: usize, visible_rows: u16, offset: u32) -> i32 {
    let max_offset = max_scroll_offset(total_rows, visible_rows);
    let offset_from_top = max_offset.saturating_sub(offset.min(max_offset));

    i32::try_from(offset_from_top).unwrap_or(i32::MAX)
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

/// Renders the transcript scrollbar in one fixed-width component.
#[derive(Default, Props)]
struct TranscriptScrollbarProps {
    marks: Vec<ScrollbarMark>,
}

/// Fixed-width transcript scrollbar drawn directly into its layout box.
#[derive(Default)]
struct TranscriptScrollbar {
    marks: Vec<ScrollbarMark>,
}

impl Component for TranscriptScrollbar {
    type Props<'a> = TranscriptScrollbarProps;

    fn new(_props: &Self::Props<'_>) -> Self {
        Self::default()
    }

    fn update(
        &mut self,
        props: &mut Self::Props<'_>,
        _hooks: Hooks,
        updater: &mut ComponentUpdater,
    ) {
        self.marks = std::mem::take(&mut props.marks);
        updater.set_layout_style(taffy::style::Style {
            size: taffy::geometry::Size {
                width: taffy::style::Dimension::Length(1.0),
                height: taffy::style::Dimension::Percent(1.0),
            },
            min_size: taffy::geometry::Size {
                width: taffy::style::Dimension::Length(1.0),
                height: taffy::style::Dimension::Auto,
            },
            max_size: taffy::geometry::Size {
                width: taffy::style::Dimension::Length(1.0),
                height: taffy::style::Dimension::Auto,
            },
            flex_shrink: 0.0,
            ..Default::default()
        });
    }

    fn draw(&mut self, drawer: &mut ComponentDrawer<'_>) {
        let mut canvas = drawer.canvas();
        for (row, mark) in self.marks.iter().enumerate() {
            canvas.set_text(0, row as isize, mark.glyph(), mark.style());
        }
    }
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

    /// Returns the color style used for this scrollbar row.
    fn style(self) -> CanvasTextStyle {
        let color = if self.thumb {
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
        };

        let mut style = CanvasTextStyle::default();
        style.color = Some(color);
        style
    }
}

/// Returns the maximum valid rendered-row offset from the transcript tail.
fn max_scroll_offset(total_rows: usize, visible_rows: u16) -> u32 {
    u32::try_from(total_rows)
        .unwrap_or(u32::MAX)
        .saturating_sub(u32::from(visible_rows))
}

/// Converts terminal input handled by the viewport into row deltas.
fn transcript_scroll_delta(
    event: TerminalEvent,
    visible_rows: u16,
    selection_active: bool,
) -> Option<i32> {
    match event {
        TerminalEvent::Key(KeyEvent {
            code: KeyCode::PageUp,
            kind,
            ..
        }) if kind != KeyEventKind::Release && !selection_active => {
            Some(page_scroll_delta(visible_rows))
        }
        TerminalEvent::Key(KeyEvent {
            code: KeyCode::PageDown,
            kind,
            ..
        }) if kind != KeyEventKind::Release && !selection_active => {
            Some(-page_scroll_delta(visible_rows))
        }
        TerminalEvent::FullscreenMouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollUp => Some(3),
            MouseEventKind::ScrollDown => Some(-3),
            _ => None,
        },
        _ => None,
    }
}

/// Returns a page-sized scroll delta from current layout rows.
fn page_scroll_delta(visible_rows: u16) -> i32 {
    i32::from(visible_rows).max(1)
}
