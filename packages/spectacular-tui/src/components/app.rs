use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::prompt_input::PromptInput;
use crate::components::status_line::StatusLine;
use crate::components::transcript::TranscriptScrollView;
use crate::state::State;
use iocraft::prelude::*;

/// Composes the full read-only IOCraft application layout from explicit state.
#[component]
pub fn App<'a>(props: &AppProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("App requires state");
    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, height: 100pct) {
            Header(state)
            TranscriptScrollView(state)
            StatusLine(state)
            PromptInput(state)
            Footer(state)
        }
    }
}

/// Props for the root application component.
#[derive(Default, Props)]
pub struct AppProps<'a> {
    pub state: Option<&'a State>,
}
