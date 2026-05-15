use crate::format::app_lines;
use crate::state::State;
use iocraft::prelude::*;

/// Composes the full read-only IOCraft application layout from explicit state.
#[component]
pub fn App<'a>(props: &AppProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("App requires state");
    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, height: 100pct) {
            #(visible_lines(state))
        }
    }
}

/// Props for the root application component.
#[derive(Default, Props)]
pub struct AppProps<'a> {
    pub state: Option<&'a State>,
}

/// Converts the state projection into IOCraft text row elements.
fn visible_lines<'a>(state: &'a State) -> Vec<AnyElement<'a>> {
    app_lines(state)
        .into_iter()
        .map(|line: String| element!(Text(content: line)).into())
        .collect()
}
