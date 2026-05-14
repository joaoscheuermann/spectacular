use crate::state::State;
use iocraft::prelude::*;

/// Renders a read-only prompt placeholder with reserved future interaction regions.
#[component]
pub fn PromptInput<'a>(props: &PromptInputProps<'a>) -> impl Into<AnyElement<'a>> {
    let state = props.state.expect("PromptInput requires state");
    let prompt = prompt_text(&state.session.prompt.text);
    element! {
        View(border_style: BorderStyle::Single, padding: 1, flex_direction: FlexDirection::Column) {
            Text(content: format!("Prompt: {prompt}"))
            Text(content: "Completions: reserved")
            Text(content: "Guidance: reserved")
        }
    }
}

/// Props for the prompt input placeholder component.
#[derive(Default, Props)]
pub struct PromptInputProps<'a> {
    pub state: Option<&'a State>,
}

/// Returns user prompt text or an empty placeholder label.
fn prompt_text(text: &str) -> &str {
    if text.is_empty() {
        return "<empty>";
    }
    text
}
