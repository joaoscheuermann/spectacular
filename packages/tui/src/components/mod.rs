mod app;
mod footer;
mod input_notice;
mod prompt;
mod selection;
mod transcript;
mod working;

pub(crate) use app::app_lines;
pub(crate) use app::transcript_capacity_rows;
pub use app::{app_element, app_render_lines, App, AppProps};
pub use footer::{
    footer_center_render_line, footer_left_render_line, footer_render_line,
    footer_render_line_with_width, footer_right_render_line, footer_text, footer_text_with_width,
    turn_usage_text, usage_text, Footer, FooterProps,
};
pub(crate) use input_notice::input_notice_render_line;
pub use input_notice::{InputNotice, InputNoticeProps};
pub use prompt::{prompt_lines, prompt_render_lines, Prompt, PromptProps};
pub(crate) use prompt::{prompt_render_lines_with_width, prompt_row_count};
pub(crate) use selection::selection_prompt_render_lines;
pub(crate) use selection::selection_prompt_row_count;
pub use selection::{SelectionPrompt, SelectionPromptProps};
pub(crate) use transcript::{plain_lines, transcript_render_lines};
pub use transcript::{
    transcript_item_layout_rows, transcript_item_lines, transcript_layout_item_range,
    transcript_layout_row_starts, transcript_layout_total_rows, wrapped_layout_text_rows,
    Assistant, AssistantProps, Banner, BannerProps, Cancellation, CancellationProps, Command,
    CommandProps, Error, ErrorProps, Notice, NoticeProps, Reasoning, ReasoningProps, Scroll,
    ScrollProps, Success, SuccessProps, Summary, SummaryProps, Tool, ToolProps, Transcript,
    TranscriptProps, User, UserProps, Warning, WarningProps, TRANSCRIPT_SEPARATOR,
};
pub use working::{working_render_line, Working, WorkingProps};
