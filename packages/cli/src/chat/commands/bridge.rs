use crate::chat::command_event::CommandEvent;
use crate::chat::model::HistoryTableModel;
use crate::chat::selection::{
    SelectionPromptAnswer, SelectionPromptChoice, SelectionPromptRequest,
};
use crate::chat::session::ChatRecord;
use crate::chat::tui::TuiEventAdapter;
use crate::chat::ChatError;
use ::agent::ToolStorage;
use ::tui::{ChatTuiAction, Intent};
use std::cell::RefCell;
use std::future::poll_fn;

pub(super) struct TuiCommandBridge<'a> {
    dispatch: RefCell<&'a mut (dyn FnMut(ChatTuiAction) + Send)>,
    adapter: RefCell<TuiEventAdapter>,
    selection_receiver: RefCell<&'a mut tokio::sync::mpsc::UnboundedReceiver<Intent>>,
}

impl<'a> TuiCommandBridge<'a> {
    pub(super) fn new(
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        selection_receiver: &'a mut tokio::sync::mpsc::UnboundedReceiver<Intent>,
    ) -> Self {
        Self {
            dispatch: RefCell::new(dispatch),
            adapter: RefCell::new(TuiEventAdapter::new()),
            selection_receiver: RefCell::new(selection_receiver),
        }
    }
}

impl TuiCommandBridge<'_> {
    /// Dispatches one reducer action into the TUI controller.
    pub(super) fn dispatch(&self, action: ChatTuiAction) {
        (self.dispatch.borrow_mut())(action);
    }

    /// Converts and dispatches one persisted command lifecycle event.
    pub(super) fn dispatch_command_event(&self, event: &CommandEvent) {
        let actions = self.adapter.borrow_mut().adapt_command_event(event);
        for action in actions {
            self.dispatch(action);
        }
    }

    /// Replays records into TUI reducer actions without writing to the terminal.
    pub(super) fn render_records(&self, records: &[ChatRecord], tools: &ToolStorage) {
        let mut adapter = TuiEventAdapter::new();
        for record in records {
            let Some(event) = record.event() else {
                continue;
            };
            if let Some(command_event) = event.to_command_event() {
                for action in adapter.adapt_command_event(&command_event) {
                    self.dispatch(action);
                }
                continue;
            }
            let Some(agent_event) = event.to_agent_event() else {
                continue;
            };
            for action in adapter.adapt_agent_event_with_tools(&agent_event, tools) {
                self.dispatch(action);
            }
        }
    }

    /// Renders a compact session history listing as TUI notices.
    pub(super) fn render_history(&self, table: &HistoryTableModel) {
        self.dispatch(ChatTuiAction::NoticeReported {
            message: "sessions".to_owned(),
        });
        for row in &table.rows {
            let marker = if row.corrupt { "*" } else { "" };
            self.dispatch(ChatTuiAction::NoticeReported {
                message: format!(
                    "{}  {}  {}  {}{}",
                    row.id, row.updated, row.title, row.messages, marker
                ),
            });
        }
        if table.remaining > 0 {
            self.dispatch(ChatTuiAction::NoticeReported {
                message: format!("{} more sessions", table.remaining),
            });
        }
    }

    /// Projects a command-owned selection prompt into TUI state and awaits the answer.
    pub(super) async fn ask(
        &self,
        request: SelectionPromptRequest,
    ) -> Result<SelectionPromptAnswer, ChatError> {
        validate_selection_request(&request)?;
        self.show_selection_prompt(&request);
        self.wait_for_selection_answer().await
    }

    fn show_selection_prompt(&self, request: &SelectionPromptRequest) {
        self.dispatch(ChatTuiAction::SelectionPromptChanged(Some(
            selection_state_from_request(request),
        )));
    }

    async fn wait_for_selection_answer(&self) -> Result<SelectionPromptAnswer, ChatError> {
        loop {
            match self.next_selection_intent().await {
                Some(Intent::SelectionPromptSubmitted(answer)) => {
                    self.dispatch(ChatTuiAction::SelectionPromptSubmitted(answer.clone()));
                    return Ok(selection_answer_from_tui(answer));
                }
                Some(Intent::SelectionPromptCancelled) | None => {
                    self.dispatch(ChatTuiAction::SelectionPromptCancelled);
                    return Err(ChatError::Exit);
                }
                Some(_) => {}
            }
        }
    }

    async fn next_selection_intent(&self) -> Option<Intent> {
        poll_fn(|cx| {
            let mut receiver = self.selection_receiver.borrow_mut();
            receiver.poll_recv(cx)
        })
        .await
    }
}

fn validate_selection_request(request: &SelectionPromptRequest) -> Result<(), ChatError> {
    if request.options.is_empty() && !request.allow_custom {
        return Err(ChatError::Session(
            "selection prompt requires an option or custom input".to_owned(),
        ));
    }

    Ok(())
}

/// Converts a command-owned selection request into reducer-owned TUI state.
fn selection_state_from_request(request: &SelectionPromptRequest) -> tui::SelectionPromptState {
    tui::SelectionPromptState::new(
        request.title.clone(),
        request.description.clone(),
        request.options.clone(),
    )
    .with_inputs(request.allow_custom, request.allow_comment)
}

/// Converts a TUI selection answer back into the command-owned answer type.
fn selection_answer_from_tui(answer: tui::SelectionPromptAnswer) -> SelectionPromptAnswer {
    SelectionPromptAnswer {
        choice: match answer.choice {
            tui::SelectionPromptChoice::Option { index, label } => {
                SelectionPromptChoice::Option { index, label }
            }
            tui::SelectionPromptChoice::Custom(value) => SelectionPromptChoice::Custom(value),
        },
        comment: answer.comment,
    }
}
