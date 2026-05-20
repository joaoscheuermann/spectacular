use crate::components::transcript_content::{render_lines_elements, styled_visible_lines};
use crate::render_model::{RenderLine, RenderStyle};
use crate::transcript::{TranscriptItem, TranscriptItemContent};
use iocraft::prelude::*;

/// Renders an error transcript item.
#[component]
pub fn Error(props: &ErrorProps) -> impl Into<AnyElement<'static>> {
    let item = props.item.clone().expect("Error requires item");
    let TranscriptItemContent::Error(error) = item.content else {
        panic!("Error requires error content");
    };
    let lines = render_lines_elements(error_render_lines(&error.message, error.details.as_deref()));

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Formats an error transcript item as semantic rows.
pub fn error_render_lines(message: &str, details: Option<&str>) -> Vec<RenderLine> {
    let mut lines = vec![RenderLine::styled(
        format!("error: {message}"),
        RenderStyle::Error,
    )];
    if let Some(details) = details {
        lines.extend(styled_visible_lines(details, RenderStyle::CommandOutput));
    }
    lines
}

/// Counts rows for an error item without allocating detail rows.
pub fn error_row_count(details: Option<&str>) -> usize {
    1 + details
        .map(crate::components::transcript_content::visible_text_row_count)
        .unwrap_or(0)
}

/// Props for the error component.
#[derive(Default, Props)]
pub struct ErrorProps {
    pub item: Option<TranscriptItem>,
}
