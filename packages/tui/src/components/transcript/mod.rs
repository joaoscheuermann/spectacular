mod assistant;
mod banner;
mod cancellation;
mod command;
mod content;
mod error;
mod notice;
mod projection;
mod reasoning;
mod scroll;
mod success;
mod summary;
mod tool;
mod user;
mod warning;

pub use assistant::{Assistant, AssistantProps};
pub use banner::{Banner, BannerProps};
pub use cancellation::{Cancellation, CancellationProps};
pub use command::{Command, CommandProps};
pub(crate) use content::plain_lines;
pub use content::TRANSCRIPT_SEPARATOR;
pub use error::{Error, ErrorProps};
pub use notice::{Notice, NoticeProps};
pub(crate) use projection::transcript_render_lines;
pub use projection::{
    transcript_item_layout_rows, transcript_item_lines, transcript_layout_item_range,
    transcript_layout_row_starts, transcript_layout_total_rows, wrapped_layout_text_rows,
};
pub use reasoning::{Reasoning, ReasoningProps};
pub use scroll::{Scroll, ScrollProps};
pub use success::{Success, SuccessProps};
pub use summary::{Summary, SummaryProps};
pub use tool::{Tool, ToolProps};
pub use user::{User, UserProps};
pub use warning::{Warning, WarningProps};

use crate::state::State;
use crate::transcript::{
    TranscriptItem, TranscriptItemContent, TranscriptLayout, TranscriptLayoutCache,
};
use iocraft::prelude::*;
use scroll::{scroll_offset_from_top, TranscriptViewportState};
use std::sync::{Arc, Mutex};

/// Renders the transcript as scrollable IOCraft item components.
#[component]
pub fn Transcript(mut hooks: Hooks, props: &TranscriptProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.as_ref();
    let capacity = props.capacity;
    let (terminal_width, _) = hooks.use_terminal_size();
    let width = props.width.unwrap_or(terminal_width);
    let content_width = transcript_content_width(width);
    let layout_cache = hooks.use_ref(|| Mutex::new(TranscriptLayoutCache::default()));
    let layout = {
        let cache_ref = layout_cache.read();
        let mut cache = cache_ref
            .lock()
            .expect("transcript layout cache lock poisoned");
        cache.snapshot_for_state(state, content_width)
    };
    let layout = layout.layout;
    let height = transcript_height(layout.total_rows, capacity);
    let normalized = TranscriptViewportState::from_scroll(&state.scroll, layout.total_rows)
        .with_render_context(layout.total_rows, height);

    let scroll_offset = scroll_offset_from_top(layout.total_rows, height, normalized.offset);
    let window = transcript_virtual_window(layout.total_rows, height, scroll_offset);
    let item_range = layout.item_range(window);
    let slice_start_row = layout.item_start_row(item_range.start);
    let visible_window = scroll_offset
        ..scroll_offset
            .saturating_add(usize::from(height))
            .min(layout.total_rows);
    let context = TranscriptRenderContext::new(state, content_width, &layout, visible_window);
    let items = transcript_item_elements(context, state, item_range);

    element!(Scroll(
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
#[derive(Props)]
pub struct TranscriptProps {
    pub state: Arc<State>,
    pub capacity: u16,
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
    context: TranscriptRenderContext,
    state: &State,
    item_range: std::ops::Range<usize>,
) -> Vec<AnyElement<'static>> {
    state
        .session
        .transcript
        .iter()
        .skip(item_range.start)
        .take(item_range.end.saturating_sub(item_range.start))
        .map(|item| transcript_item_element(&context, item))
        .collect()
}

/// Selects the sibling component that owns rendering for one transcript item.
fn transcript_item_element(
    context: &TranscriptRenderContext,
    item: &TranscriptItem,
) -> AnyElement<'static> {
    let key = item.id.as_str().to_owned();
    let source_id = item.id.as_str().to_owned();

    match &item.content {
        TranscriptItemContent::OpeningBanner(banner) => Element::<Banner> {
            key: ElementKey::new(key),
            props: BannerProps {
                source_id,
                version: banner.version.clone(),
                model: banner.model.clone(),
                reasoning: banner.reasoning.clone(),
                directory: banner.directory.clone(),
                session_id: banner.session_id.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::UserPrompt(prompt) => Element::<User> {
            key: ElementKey::new(key),
            props: UserProps {
                source_id,
                text: prompt.text.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::AssistantMessage(message) => Element::<Assistant> {
            key: ElementKey::new(key),
            props: AssistantProps {
                source_id,
                text: message.text.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Reasoning(reasoning) => Element::<Reasoning> {
            key: ElementKey::new(key),
            props: ReasoningProps {
                source_id,
                text: reasoning.text.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::ToolCall(tool) => Element::<Tool> {
            key: ElementKey::new(key),
            props: ToolProps {
                source_id,
                name: tool.name.clone(),
                arguments_preview: tool.arguments_preview.clone(),
                output_preview: tool.output_preview.clone(),
                display: tool.display.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Command(command) => Element::<Command> {
            key: ElementKey::new(key),
            props: CommandProps {
                source_id,
                command: command.command.clone(),
                status: command.status,
                output: command.output.clone(),
                exit_code: command.exit_code,
                display: command.display.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Error(error) => Element::<Error> {
            key: ElementKey::new(key),
            props: ErrorProps {
                source_id,
                message: error.message.clone(),
                details: error.details.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Warning(warning) => Element::<Warning> {
            key: ElementKey::new(key),
            props: WarningProps {
                source_id,
                message: warning.message.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Success(success) => Element::<Success> {
            key: ElementKey::new(key),
            props: SuccessProps {
                source_id,
                message: success.message.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Notice(notice) => Element::<Notice> {
            key: ElementKey::new(key),
            props: NoticeProps {
                source_id,
                message: notice.message.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::Cancellation(cancellation) => Element::<Cancellation> {
            key: ElementKey::new(key),
            props: CancellationProps {
                source_id,
                reason: cancellation.reason.clone(),
                context: context.clone(),
            },
        }
        .into_any(),
        TranscriptItemContent::WorkedSummary(summary) => Element::<Summary> {
            key: ElementKey::new(key),
            props: SummaryProps {
                source_id,
                duration: summary.duration.clone(),
                turn_tokens: summary.turn_tokens,
                context: context.clone(),
            },
        }
        .into_any(),
    }
}

/// Render-only transcript context shared by visible transcript item components.
#[derive(Clone, Debug)]
pub struct TranscriptRenderContext {
    pub(crate) selection_colors: crate::render::TuiSelectionColors,
    pub(crate) content_width: usize,
    pub(crate) selection_plan: Arc<crate::selection::SelectionStylingPlan>,
}

impl TranscriptRenderContext {
    fn new(
        state: &State,
        content_width: usize,
        layout: &TranscriptLayout,
        visible_window: std::ops::Range<usize>,
    ) -> Self {
        let screen_width = content_width.saturating_add(1);
        let projection = Arc::new(
            crate::selection::SelectableProjection::for_transcript_window(
                state,
                content_width,
                screen_width,
                layout,
                visible_window,
            ),
        );
        let selection_plan = Arc::new(crate::selection::SelectionStylingPlan::new(
            &state.app_selection,
            &projection,
        ));

        Self {
            selection_colors: state.selection_colors,
            content_width,
            selection_plan,
        }
    }
}
