use crate::state::State;
use iocraft::prelude::*;

/// Renders model, reasoning, directory, and session metadata from application state.
#[component]
pub fn Header<'a>(props: &HeaderProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("Header requires state");
    element! {
        View(
            border_style: BorderStyle::Single,
            border_edges: Edges::Bottom,
            padding_bottom: 1,
            flex_direction: FlexDirection::Column,
        ) {
            Text(content: "Spectacular")
            Text(content: format!("model: {}", state.display.model_label))
            Text(content: format!("reasoning: {}", state.display.reasoning_label))
            Text(content: format!("directory: {}", state.display.current_directory))
            Text(content: format!("session: {}", state.display.session_label))
        }
    }
}

/// Props for the header component.
#[derive(Default, Props)]
pub struct HeaderProps<'a> {
    pub state: Option<&'a State>,
}
