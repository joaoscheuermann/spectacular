use crate::format::app_lines;
use crate::state::State;
use iocraft::prelude::*;

/// Composes the full IOCraft application layout from owned state for runtime rendering.
#[component]
pub fn AppState(props: &AppStateProps) -> impl Into<AnyElement<'static>> {
    let state = props.state.clone().expect("AppState requires state");
    element! {
        View(flex_direction: FlexDirection::Column, width: 100pct, height: 100pct) {
            #(visible_lines(&state))
        }
    }
}

/// Props for the owned-state root application component.
#[derive(Default, Props)]
pub struct AppStateProps {
    pub state: Option<State>,
}

/// Converts the state projection into owned IOCraft text row elements.
fn visible_lines(state: &State) -> Vec<AnyElement<'static>> {
    app_lines(state)
        .into_iter()
        .map(|line: String| element!(Text(content: line)).into())
        .collect()
}
