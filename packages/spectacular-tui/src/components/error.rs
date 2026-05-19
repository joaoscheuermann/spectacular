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
    let mut render_lines = vec![RenderLine::styled(
        format!("error: {}", error.message),
        RenderStyle::Error,
    )];
    if let Some(details) = error.details {
        render_lines.extend(styled_visible_lines(&details, RenderStyle::CommandOutput));
    }
    let lines = render_lines_elements(render_lines);

    element!(View(flex_direction: FlexDirection::Column) { #(lines.into_iter()) })
}

/// Props for the error component.
#[derive(Default, Props)]
pub struct ErrorProps {
    pub item: Option<TranscriptItem>,
}
