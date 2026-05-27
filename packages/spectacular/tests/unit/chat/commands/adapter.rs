use super::*;
use crate::chat::model::ChatModel;
use crate::chat::selection::{
    SelectionPromptAnswer, SelectionPromptChoice, SelectionPromptRequest,
};
use crate::chat::{ChatError, RuntimeSelection};
use spectacular_agent::{AgentEvent, ToolStorage};
use spectacular_commands::CommandInvocation;
use spectacular_config::ReasoningLevel;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Verifies that adapter executes registered command success.
#[tokio::test]
async fn adapter_executes_registered_command_success() {
    let adapter = ChatCommandAdapter::new([session::clear::command()]).unwrap();
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    let result = adapter
        .execute(
            context,
            CommandInvocation {
                name: "clear".to_owned(),
                args: Vec::new(),
            },
        )
        .await;

    assert_eq!(result, ChatCommandResult::Success);
}

/// Verifies that registry exposes command metadata.
#[test]
fn registry_exposes_command_metadata() {
    let adapter = registry().unwrap();

    assert!(adapter
        .metadata()
        .search("", 16)
        .iter()
        .any(|entry| entry.metadata.name == "retry"));
}

/// Verifies that adapter builds completion specs from registered commands.
#[test]
fn adapter_builds_completion_specs_from_registered_commands() {
    const SUBCOMMANDS: &[CompletionSubcommandSpec] = &[CompletionSubcommandSpec {
        name: "set",
        summary: "Set value",
        fields: &[],
    }];

    /// Executes the tool with the provided arguments and cancellation handle.
    fn execute<'a>(_context: ChatCommandContext<'a>, _args: Vec<String>) -> ChatCommandFuture<'a> {
        Box::pin(async { ChatCommandResult::success() })
    }

    let adapter = ChatCommandAdapter::new([ChatCommand {
        name: "owned",
        usage: "/owned set",
        summary: "Owned command",
        completion: SUBCOMMANDS,
        execute,
    }])
    .unwrap();

    assert_eq!(
        adapter.completion_specs(),
        &[CompletionCommandSpec {
            name: "owned",
            subcommands: SUBCOMMANDS,
        }]
    );
}

/// Verifies that registry exposes completion specs from commands.
#[test]
fn registry_exposes_completion_specs_from_commands() {
    let adapter = registry().unwrap();

    assert_eq!(
        adapter
            .completion_specs()
            .iter()
            .map(|spec| spec.name)
            .collect::<Vec<_>>(),
        vec!["provider", "model", "task", "git"]
    );

    let provider = adapter
        .completion_specs()
        .iter()
        .find(|spec| spec.name == "provider")
        .expect("provider completion should be registered");
    assert_eq!(
        provider
            .subcommands
            .iter()
            .map(|subcommand| subcommand.name)
            .collect::<Vec<_>>(),
        vec!["add", "remove", "auth"]
    );

    let model = adapter
        .completion_specs()
        .iter()
        .find(|spec| spec.name == "model")
        .expect("model completion should be registered");
    assert_eq!(
        model
            .subcommands
            .iter()
            .map(|subcommand| subcommand.name)
            .collect::<Vec<_>>(),
        vec!["add", "edit", "remove"]
    );

    let task = adapter
        .completion_specs()
        .iter()
        .find(|spec| spec.name == "task")
        .expect("task completion should be registered");
    assert_eq!(task.subcommands[0].fields[0].name, "task");
    assert_eq!(task.subcommands[0].fields[1].name, "model");
}

/// Verifies that provider completion uses provider type for add and auth.
#[test]
fn provider_completion_uses_provider_type_for_add_and_auth() {
    let adapter = registry().unwrap();
    let provider = adapter
        .completion_specs()
        .iter()
        .find(|spec| spec.name == "provider")
        .expect("provider completion should be registered");

    let add = provider
        .subcommands
        .iter()
        .find(|subcommand| subcommand.name == "add")
        .expect("provider add completion should be registered");
    assert_eq!(add.fields[0].name, "provider");
    assert_eq!(add.fields[1].name, "apikey");
    assert!(add.fields[1].required);

    let auth = provider
        .subcommands
        .iter()
        .find(|subcommand| subcommand.name == "auth")
        .expect("provider auth completion should be registered");
    assert_eq!(auth.fields[0].name, "provider");
    assert_eq!(
        auth.fields[0].validation,
        CompletionValueValidation::OneOfValues
    );
}

/// Verifies that context append agent event persists chat record.
#[test]
fn context_append_agent_event_persists_chat_record() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    {
        let context = ChatCommandContext::new(&mut model, &tools, &mut control);

        context
            .append_agent_event(&AgentEvent::user_prompt("persist me"))
            .unwrap();
    }

    assert!(model.records().unwrap().iter().any(|record| matches!(
        record.event(),
        Some(crate::chat::session::ChatEvent::UserPrompt { content, .. })
            if content == "persist me"
    )));
}

/// Verifies that context render records accepts transient records.
#[tokio::test]
async fn context_render_records_accepts_transient_records() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    context.render_records(&[]).await.unwrap();
}

/// Verifies TUI clear screen dispatches only a visual transcript clear.
#[test]
fn context_clear_screen_when_tui_dispatches_transcript_cleared() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let (_selection_sender, mut selection_receiver) = mpsc::unbounded_channel();
    let mut actions = Vec::new();

    {
        let mut dispatch = |action| actions.push(action);
        let context = ChatCommandContext::new_tui(
            &mut model,
            &tools,
            &mut control,
            None,
            &mut dispatch,
            &mut selection_receiver,
        );
        context.clear_screen();
    }

    assert_eq!(
        actions,
        vec![spectacular_tui::ChatTuiAction::TranscriptCleared]
    );
}

/// Verifies that context render history accepts transient history.
#[test]
fn context_render_history_accepts_transient_history() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let table = model
        .history(crate::chat::session::HistoryQuery::FirstPage)
        .unwrap();
    let mut control = ChatCommandControl::default();
    let context = ChatCommandContext::new(&mut model, &tools, &mut control);

    context.render_history(&table);
}

/// Verifies a TUI command context round-trips any command-owned selection request.
#[tokio::test]
async fn context_tui_selection_prompt_round_trips_command_request() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let (selection_sender, mut selection_receiver) = mpsc::unbounded_channel();
    selection_sender
        .send(spectacular_tui::Intent::SelectionPromptSubmitted(
            spectacular_tui::SelectionPromptAnswer {
                choice: spectacular_tui::SelectionPromptChoice::Custom("typed".to_owned()),
                comment: Some("note".to_owned()),
            },
        ))
        .unwrap();
    let mut actions = Vec::new();

    let answer = {
        let mut dispatch = |action| actions.push(action);
        let context = ChatCommandContext::new_tui(
            &mut model,
            &tools,
            &mut control,
            None,
            &mut dispatch,
            &mut selection_receiver,
        );
        context
            .ask(
                SelectionPromptRequest::new("Pick one", "Choose carefully", vec!["alpha".into()])
                    .with_inputs(true, true),
            )
            .await
            .unwrap()
    };

    assert_eq!(
        answer,
        SelectionPromptAnswer {
            choice: SelectionPromptChoice::Custom("typed".to_owned()),
            comment: Some("note".to_owned()),
        }
    );
    assert!(matches!(
        &actions[0],
        spectacular_tui::ChatTuiAction::SelectionPromptChanged(Some(selection))
            if selection.title == "Pick one"
                && selection.description == "Choose carefully"
                && selection.options == vec!["alpha".to_owned()]
                && selection.allow_custom
                && selection.allow_comment
    ));
    assert!(matches!(
        actions.last(),
        Some(spectacular_tui::ChatTuiAction::SelectionPromptSubmitted(_))
    ));
}

/// Verifies TUI selection cancellation maps to the command prompt exit contract.
#[tokio::test]
async fn context_tui_selection_cancel_returns_exit() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let (selection_sender, mut selection_receiver) = mpsc::unbounded_channel();
    selection_sender
        .send(spectacular_tui::Intent::SelectionPromptCancelled)
        .unwrap();
    let mut actions = Vec::new();

    let result = {
        let mut dispatch = |action| actions.push(action);
        let context = ChatCommandContext::new_tui(
            &mut model,
            &tools,
            &mut control,
            None,
            &mut dispatch,
            &mut selection_receiver,
        );
        context
            .ask(SelectionPromptRequest::new(
                "Pick one",
                "",
                vec!["alpha".into()],
            ))
            .await
    };

    assert!(matches!(result, Err(ChatError::Exit)));
    assert!(matches!(
        actions.last(),
        Some(spectacular_tui::ChatTuiAction::SelectionPromptCancelled)
    ));
}

/// Verifies TUI selection requests fail fast when there is no selectable answer.
#[tokio::test]
async fn context_tui_selection_prompt_requires_option_or_custom_input() {
    let mut model = test_model();
    let tools = ToolStorage::default();
    let mut control = ChatCommandControl::default();
    let (_selection_sender, mut selection_receiver) = mpsc::unbounded_channel();
    let mut actions = Vec::new();

    let result = {
        let mut dispatch = |action| actions.push(action);
        let context = ChatCommandContext::new_tui(
            &mut model,
            &tools,
            &mut control,
            None,
            &mut dispatch,
            &mut selection_receiver,
        );
        context
            .ask(SelectionPromptRequest::new("Pick one", "", Vec::new()))
            .await
    };

    assert!(
        matches!(result, Err(ChatError::Session(message)) if message == "selection prompt requires an option or custom input")
    );
    assert!(actions.is_empty());
}

/// Builds a chat model configured for command tests.
fn test_model() -> ChatModel {
    let session = crate::chat::session::SessionManager::new_in(temp_session_dir("adapter"))
        .expect("session manager should be created");
    let mut model = ChatModel::new(
        session,
        RuntimeSelection {
            provider_type: "openrouter".to_owned(),
            provider_auth: Some(spectacular_config::ProviderAuthMode::ApiKey),
            provider: "openrouter".to_owned(),
            api_key: "sk-or-v1-test".to_owned(),
            model_key: "test-model".to_owned(),
            model: "test/model".to_owned(),
            reasoning: ReasoningLevel::Medium,
            context_window_tokens: None,
        },
    );
    model.start_new_session().unwrap();
    model
}

/// Builds a temporary session directory path for a named test case.
fn temp_session_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-command-adapter-{name}-{suffix}"))
}
