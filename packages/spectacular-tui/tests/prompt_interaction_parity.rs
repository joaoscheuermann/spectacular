use iocraft::prelude::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, TerminalEvent};
use spectacular_tui::{
    effects, reduce, CachedModelValues, ChatTuiAction, CommandDescriptor, CommandFieldDescriptor,
    CommandSubcommandDescriptor, CommandValueValidation, CompletionValues, DisplayMetadata,
    EventEffect, PromptState, ReasoningLevel, RenderHighlight, RenderStyle, RuntimeSelection,
    SelectionPromptAnswer, SelectionPromptChoice, SelectionPromptState, SessionId, State,
};
use std::collections::BTreeMap;

/// Builds runtime metadata for prompt parity tests.
fn runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        "provider",
        "model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds display metadata for prompt parity tests.
fn display() -> DisplayMetadata {
    DisplayMetadata::new("provider", "model", "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(SessionId::new("session-1"), runtime(), display())
}

/// Builds one IOCraft key event for event-loop tests.
fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalEvent {
    let mut event = KeyEvent::new(KeyEventKind::Press, code);
    event.modifiers = modifiers;
    TerminalEvent::Key(event)
}

/// Extracts a single action from one terminal event.
fn single_action(state: &State, event: TerminalEvent) -> ChatTuiAction {
    let effects = effects(state, event);
    assert_eq!(effects.len(), 1, "effects: {effects:?}");
    match effects.into_iter().next().unwrap() {
        EventEffect::Action(action) => *action,
        EventEffect::ViewAction(_) => panic!("expected action effect"),
        EventEffect::RequestExit => panic!("expected action effect"),
    }
}

/// Applies one key event to reducer state.
fn press(state: &mut State, code: KeyCode, modifiers: KeyModifiers) {
    let action = single_action(state, key(code, modifiers));
    reduce(state, action);
}

/// Builds a structured `/model` command descriptor for command-composer tests.
fn model_command() -> CommandDescriptor {
    let cached_models = CachedModelValues::new(
        BTreeMap::from([
            (
                "openrouter".to_owned(),
                vec![
                    "openai/gpt-5.5".to_owned(),
                    "openai/gpt-5.4".to_owned(),
                    "anthropic/claude-sonnet".to_owned(),
                    "meta/llama".to_owned(),
                    "mistral/large".to_owned(),
                    "qwen/max".to_owned(),
                    "deepseek/reasoner".to_owned(),
                    "google/gemini-flash".to_owned(),
                    "xai/grok".to_owned(),
                ],
            ),
            ("work".to_owned(), vec!["google/gemini".to_owned()]),
        ]),
        BTreeMap::from([("coding".to_owned(), "work".to_owned())]),
    );
    let provider_values =
        CompletionValues::ConfiguredProviders(vec!["openrouter".to_owned(), "work".to_owned()]);
    let reasoning_values = CompletionValues::Static(vec![
        "none".to_owned(),
        "minimal".to_owned(),
        "low".to_owned(),
        "medium".to_owned(),
        "high".to_owned(),
        "xhigh".to_owned(),
    ]);
    let saved_models = CompletionValues::SavedModels(vec!["coding".to_owned()]);

    CommandDescriptor::with_usage(
        "model",
        "Manage saved models",
        "/model add provider:<provider> id:<model-id> reasoning:<level> [name:<name>]",
    )
    .with_subcommands(vec![
        CommandSubcommandDescriptor::new(
            "add",
            "Add model",
            vec![
                CommandFieldDescriptor::new(
                    "provider",
                    "configured provider name",
                    true,
                    provider_values.clone(),
                    CommandValueValidation::None,
                ),
                CommandFieldDescriptor::new(
                    "id",
                    "model ID from the selected provider",
                    true,
                    CompletionValues::CachedModelIds(cached_models.clone()),
                    CommandValueValidation::None,
                ),
                CommandFieldDescriptor::new(
                    "reasoning",
                    "reasoning level",
                    true,
                    reasoning_values.clone(),
                    CommandValueValidation::OneOfValues,
                ),
                CommandFieldDescriptor::new(
                    "name",
                    "optional saved model name",
                    false,
                    saved_models.clone(),
                    CommandValueValidation::None,
                ),
            ],
        ),
        CommandSubcommandDescriptor::new(
            "edit",
            "Edit model",
            vec![
                CommandFieldDescriptor::new(
                    "name",
                    "saved model key",
                    true,
                    saved_models.clone(),
                    CommandValueValidation::None,
                ),
                CommandFieldDescriptor::new(
                    "id",
                    "replacement model ID",
                    false,
                    CompletionValues::CachedModelIds(cached_models),
                    CommandValueValidation::None,
                ),
            ],
        ),
        CommandSubcommandDescriptor::new("remove", "Remove model", Vec::new()),
    ])
}

#[path = "prompt_interaction_parity/prompt_basic.rs"]
mod prompt_basic;
#[path = "prompt_interaction_parity/prompt_selection_prompt.rs"]
mod prompt_selection_prompt;
#[path = "prompt_interaction_parity/prompt_slash.rs"]
mod prompt_slash;
