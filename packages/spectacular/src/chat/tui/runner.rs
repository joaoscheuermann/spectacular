use super::adapter::TuiEventAdapter;
use crate::chat::model::{ChatModel, ChatRunRequestModel};
use crate::chat::provider::provider_for_runtime;
use crate::chat::runner::main_chat_agent;
use crate::chat::session::{agent_events_from_records, records_before_latest_user_prompt};
use crate::chat::ChatError;
use spectacular_agent::{AgentEvent, Store, ToolStorage};
use spectacular_tui::ChatTuiAction;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) type TurnFuture<'a> = Pin<Box<dyn Future<Output = Result<(), ChatError>> + Send + 'a>>;

#[derive(Eq, PartialEq)]
enum StreamFlow {
    Continue,
    Stop,
}

struct EventHandler<'a, 'dispatch> {
    model: &'a ChatModel,
    tools: &'a ToolStorage,
    adapter: TuiEventAdapter,
    dispatch: &'dispatch mut (dyn FnMut(ChatTuiAction) + Send),
}

/// Async seam for executing one TUI chat turn without coupling the TUI to runtime APIs.
pub(crate) trait TurnRunner: Send {
    /// Runs a prompt and sends controller-owned TUI actions to the supplied callback.
    fn run<'a>(
        &'a mut self,
        model: &'a ChatModel,
        tools: &'a ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TurnFuture<'a>;

    /// Cancels the active runtime run if one exists.
    fn cancel(&mut self);
}

/// Runtime-backed TUI turn runner that streams real agent events into reducer actions.
#[derive(Default)]
pub(crate) struct AgentRunner {
    active: Option<spectacular_agent::AgentRunStream>,
}

impl TurnRunner for AgentRunner {
    /// Runs a real agent stream and maps runtime events into TUI actions.
    fn run<'a>(
        &'a mut self,
        model: &'a ChatModel,
        tools: &'a ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &'a mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &'a mut mpsc::UnboundedReceiver<()>,
    ) -> TurnFuture<'a> {
        Box::pin(async move {
            self.run_agent_stream(model, tools, request, dispatch, cancellation)
                .await
        })
    }

    /// Cancels the active agent stream when a TUI cancel intent arrives.
    fn cancel(&mut self) {
        if let Some(stream) = &self.active {
            stream.cancel();
        }
    }
}

impl AgentRunner {
    /// Executes the real agent stream while keeping terminal output in the TUI reducer.
    async fn run_agent_stream(
        &mut self,
        model: &ChatModel,
        tools: &ToolStorage,
        request: ChatRunRequestModel,
        dispatch: &mut (dyn FnMut(ChatTuiAction) + Send),
        cancellation: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        self.start_stream(model, tools, request)?;
        let mut handler = EventHandler::new(model, tools, dispatch);
        self.consume_stream(&mut handler, cancellation).await?;
        self.active = None;
        Ok(())
    }

    fn start_stream(
        &mut self,
        model: &ChatModel,
        tools: &ToolStorage,
        request: ChatRunRequestModel,
    ) -> Result<(), ChatError> {
        let agent = main_chat_agent(
            provider_for_runtime(
                &request.runtime,
                model.debug_logger().clone(),
                model.config_io(),
            )?,
            &request.runtime,
            store_for_request(model, &request)?,
            tools.clone(),
        );
        self.active = Some(
            Arc::new(agent)
                .run_stream_with_prompt_event_id(request.prompt, request.prompt_event_id),
        );
        Ok(())
    }

    async fn consume_stream(
        &mut self,
        handler: &mut EventHandler<'_, '_>,
        cancellation: &mut mpsc::UnboundedReceiver<()>,
    ) -> Result<(), ChatError> {
        while let Some(event) = self.next_event(cancellation).await {
            if handler.handle(event)? == StreamFlow::Stop {
                break;
            }
        }
        Ok(())
    }

    /// Receives one event from the active agent stream or maps cancellation into an agent event.
    async fn next_event(
        &mut self,
        cancellation: &mut mpsc::UnboundedReceiver<()>,
    ) -> Option<AgentEvent> {
        let stream = self.active.as_mut()?;
        tokio::select! {
            event = stream.next() => event,
            cancellation = cancellation.recv() => {
                cancellation?;
                stream.cancel();
                Some(AgentEvent::Cancelled {
                    reason: "run cancelled".to_owned(),
                })
            }
        }
    }
}

impl<'a, 'dispatch> EventHandler<'a, 'dispatch> {
    fn new(
        model: &'a ChatModel,
        tools: &'a ToolStorage,
        dispatch: &'dispatch mut (dyn FnMut(ChatTuiAction) + Send),
    ) -> Self {
        Self {
            model,
            tools,
            adapter: TuiEventAdapter::new(),
            dispatch,
        }
    }

    fn handle(&mut self, event: AgentEvent) -> Result<StreamFlow, ChatError> {
        if let AgentEvent::ContextTokenUsage(usage) = event {
            self.model.set_context_token_usage(usage);
            self.dispatch(&AgentEvent::ContextTokenUsage(usage));
            return Ok(StreamFlow::Continue);
        }

        let flow = if matches!(event, AgentEvent::Cancelled { .. }) {
            StreamFlow::Stop
        } else {
            StreamFlow::Continue
        };
        self.model.append_agent_event(&event)?;
        self.dispatch(&event);
        Ok(flow)
    }

    fn dispatch(&mut self, event: &AgentEvent) {
        for action in self.adapter.adapt_agent_event_with_tools(event, self.tools) {
            (self.dispatch)(action);
        }
    }
}

/// Builds an agent store from the current session records and retry mode.
fn store_for_request(model: &ChatModel, request: &ChatRunRequestModel) -> Result<Store, ChatError> {
    let records = model.records()?;
    let context_records = if request.retry_existing_prompt {
        records_before_latest_user_prompt(&records)
    } else {
        records.as_slice()
    };

    Ok(Store::from(agent_events_from_records(context_records)))
}
