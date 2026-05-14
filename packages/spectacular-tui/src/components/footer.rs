use crate::components::status_line::usage_text;
use crate::state::State;
use iocraft::prelude::*;

/// Renders footer metadata from display/session state without external lookups.
#[component]
pub fn Footer<'a>(props: &FooterProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("Footer requires state");
    let usage = usage_text(state.session.usage.or(state.display.usage));
    element! {
        View(border_style: BorderStyle::Single, border_edges: Edges::Top, padding_top: 1) {
            Text(content: format!(
                "cwd: {} | provider/model: {}/{} | reasoning: {} | context: {}",
                state.display.current_directory,
                state.display.provider_label,
                state.display.model_label,
                state.display.reasoning_label,
                usage,
            ))
        }
    }
}

/// Props for the footer component.
#[derive(Default, Props)]
pub struct FooterProps<'a> {
    pub state: Option<&'a State>,
}
