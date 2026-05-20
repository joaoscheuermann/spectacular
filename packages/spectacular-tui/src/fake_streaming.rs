use crate::action::ChatTuiAction;
use crate::event_loop::TUI_SPINNER_TICK_INTERVAL;
use crate::ids::{SessionId, TranscriptItemId};
use crate::metadata::{DisplayMetadata, ReasoningLevel, RuntimeSelection};
use crate::reducer::reduce;
use crate::render::render_state_to_string;
use crate::session::PromptState;
use crate::state::State;
use crate::status::Status;
use crate::transcript::{
    CommandItem, CommandStatus, ToolCallItem, ToolStatus, TranscriptItemContent,
};
use std::time::Duration;

/// Deterministic fake runtime event schedule consumed by the prototype harness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeStreamingPlan {
    scheduled_actions: Vec<ScheduledAction>,
}

/// One reducer action scheduled at a fake elapsed time.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ScheduledAction {
    at: Duration,
    action: ChatTuiAction,
}

/// Describes which source produced the most recent fake streaming step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakeStreamingTickOutcome {
    AgentAction,
    SpinnerTick,
    Finished,
}

/// Deterministic async-compatible fake streaming runner for IOCraft TUI state.
#[derive(Clone, Debug)]
pub struct FakeStreamingTimeline {
    state: State,
    plan: FakeStreamingPlan,
    next_action: usize,
    elapsed: Duration,
    next_spinner_tick: Duration,
    spinner_ticks: usize,
    direct_terminal_writes: usize,
}

impl FakeStreamingPlan {
    /// Creates a fake runtime plan from caller-owned scheduled reducer actions.
    pub fn new(scheduled_actions: Vec<(Duration, ChatTuiAction)>) -> Self {
        let mut scheduled_actions: Vec<_> = scheduled_actions
            .into_iter()
            .map(|(at, action)| ScheduledAction { at, action })
            .collect();
        scheduled_actions.sort_by_key(|action| action.at);
        Self { scheduled_actions }
    }
}

impl FakeStreamingTimeline {
    /// Creates a deterministic fake streaming timeline with default prototype metadata.
    pub fn new(plan: FakeStreamingPlan) -> Self {
        Self {
            state: State::new(
                SessionId::new("fake-session"),
                fake_runtime(),
                fake_display(),
            ),
            plan,
            next_action: 0,
            elapsed: Duration::ZERO,
            next_spinner_tick: TUI_SPINNER_TICK_INTERVAL,
            spinner_ticks: 0,
            direct_terminal_writes: 0,
        }
    }

    /// Advances to the next scheduled agent action or spinner tick without terminal IO.
    pub async fn step(&mut self) -> FakeStreamingTickOutcome {
        let Some(next_action) = self.plan.scheduled_actions.get(self.next_action) else {
            return self.step_remaining_spinner();
        };

        if self.spinner_is_active() && self.next_spinner_tick <= next_action.at {
            return self.apply_spinner_tick();
        }

        self.elapsed = next_action.at;
        reduce(&mut self.state, next_action.action.clone());
        self.next_action += 1;
        FakeStreamingTickOutcome::AgentAction
    }

    /// Runs fake streaming until every scheduled agent action has been reduced.
    pub async fn run_until_finished(&mut self) {
        while self.next_action < self.plan.scheduled_actions.len() {
            self.step().await;
        }
    }

    /// Runs fake streaming through all scheduled events at or before the supplied elapsed time.
    pub async fn run_for(&mut self, elapsed: Duration) {
        while self.has_work_before(elapsed) {
            self.step().await;
        }
        self.elapsed = elapsed;
    }

    /// Applies a local prompt edit while preserving runtime-streamed transcript state.
    pub fn apply_prompt(&mut self, prompt: PromptState) {
        reduce(&mut self.state, ChatTuiAction::PromptChanged(prompt));
    }

    /// Returns immutable access to the current prototype state.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Returns the rendered IOCraft projection for the current prototype state.
    pub fn rendered_output(&self) -> String {
        render_state_to_string(&self.state, Some(120))
    }

    /// Returns how many direct terminal writes the fake source performed.
    pub fn direct_terminal_writes(&self) -> usize {
        self.direct_terminal_writes
    }

    /// Returns how many spinner ticks were reduced independently of agent actions.
    pub fn spinner_tick_count(&self) -> usize {
        self.spinner_ticks
    }

    /// Returns whether the current status is idle.
    pub fn is_idle(&self) -> bool {
        self.state.status == Status::Idle
    }

    /// Returns whether the current status is failed.
    pub fn is_failed(&self) -> bool {
        matches!(self.state.status, Status::Failed { .. })
    }

    /// Returns the visible assistant text for a transcript item ID.
    pub fn assistant_text(&self, id: &str) -> Option<String> {
        self.find_content(id).and_then(|content| match content {
            TranscriptItemContent::AssistantMessage(message) => Some(message.text.clone()),
            _ => None,
        })
    }

    /// Returns the count of assistant transcript items in current state.
    pub fn assistant_item_count(&self) -> usize {
        self.state
            .session
            .transcript
            .iter()
            .filter(|item| matches!(item.content, TranscriptItemContent::AssistantMessage(_)))
            .count()
    }

    /// Returns reasoning text for a transcript item ID.
    pub fn reasoning_text(&self, id: &str) -> Option<&str> {
        self.find_content(id).and_then(|content| match content {
            TranscriptItemContent::Reasoning(reasoning) => Some(reasoning.text.as_str()),
            _ => None,
        })
    }

    /// Returns the count of reasoning transcript items in current state.
    pub fn reasoning_item_count(&self) -> usize {
        self.state
            .session
            .transcript
            .iter()
            .filter(|item| matches!(item.content, TranscriptItemContent::Reasoning(_)))
            .count()
    }

    /// Returns a tool item by transcript item ID.
    pub fn tool(&self, id: &str) -> Option<&ToolCallItem> {
        self.find_content(id).and_then(|content| match content {
            TranscriptItemContent::ToolCall(tool) => Some(tool),
            _ => None,
        })
    }

    /// Returns whether a tool item has reached finished status.
    pub fn is_tool_finished(&self, id: &str) -> bool {
        matches!(self.tool(id), Some(tool) if tool.status == ToolStatus::Finished)
    }

    /// Returns a command item by transcript item ID.
    pub fn command(&self, id: &str) -> Option<&CommandItem> {
        self.find_content(id).and_then(|content| match content {
            TranscriptItemContent::Command(command) => Some(command),
            _ => None,
        })
    }

    /// Returns whether a command item has reached finished status.
    pub fn is_command_finished(&self, id: &str) -> bool {
        matches!(self.command(id), Some(command) if command.status == CommandStatus::Finished)
    }

    /// Applies a spinner tick and advances the deterministic ticker schedule.
    fn apply_spinner_tick(&mut self) -> FakeStreamingTickOutcome {
        self.elapsed = self.next_spinner_tick;
        reduce(&mut self.state, ChatTuiAction::SpinnerTick);
        self.next_spinner_tick += TUI_SPINNER_TICK_INTERVAL;
        self.spinner_ticks += 1;
        FakeStreamingTickOutcome::SpinnerTick
    }

    /// Emits one pending timer tick after all agent actions when runtime status is still active.
    fn step_remaining_spinner(&mut self) -> FakeStreamingTickOutcome {
        if !self.spinner_is_active() {
            return FakeStreamingTickOutcome::Finished;
        }

        self.apply_spinner_tick()
    }

    /// Returns whether a scheduled source has work at or before the target elapsed time.
    fn has_work_before(&self, elapsed: Duration) -> bool {
        let next_action_due = self
            .plan
            .scheduled_actions
            .get(self.next_action)
            .is_some_and(|action| action.at <= elapsed);
        next_action_due || self.spinner_is_active() && self.next_spinner_tick <= elapsed
    }

    /// Returns whether the status line should continue receiving spinner ticks.
    fn spinner_is_active(&self) -> bool {
        matches!(
            self.state.status,
            Status::Running { .. } | Status::Cancelling
        )
    }

    /// Finds transcript content by string item ID.
    fn find_content(&self, id: &str) -> Option<&TranscriptItemContent> {
        self.state
            .session
            .transcript
            .iter()
            .find(|item| item.id.as_str() == id)
            .map(|item| &item.content)
    }
}

/// Builds the main deterministic fake streaming lifecycle used by tests and the example.
pub fn fake_streaming_plan() -> FakeStreamingPlan {
    FakeStreamingPlan::new(vec![
        (Duration::ZERO, ChatTuiAction::AgentStarted),
        (
            Duration::from_millis(10),
            ChatTuiAction::ReasoningStarted {
                id: item_id("reasoning-1"),
            },
        ),
        (
            Duration::from_millis(20),
            ChatTuiAction::ReasoningDelta {
                id: item_id("reasoning-1"),
                text: "Plan the response, ".to_owned(),
            },
        ),
        (
            Duration::from_millis(35),
            ChatTuiAction::ReasoningDelta {
                id: item_id("reasoning-1"),
                text: "then stream it.".to_owned(),
            },
        ),
        (
            Duration::from_millis(45),
            ChatTuiAction::ReasoningFinished {
                id: item_id("reasoning-1"),
            },
        ),
        (
            Duration::from_millis(55),
            ChatTuiAction::MessageStarted {
                id: item_id("assistant-1"),
            },
        ),
        (
            Duration::from_millis(65),
            ChatTuiAction::MessageDelta {
                id: item_id("assistant-1"),
                text: "Hello ".to_owned(),
            },
        ),
        (
            Duration::from_millis(110),
            ChatTuiAction::MessageDelta {
                id: item_id("assistant-1"),
                text: "from the fake ".to_owned(),
            },
        ),
        (
            Duration::from_millis(150),
            ChatTuiAction::MessageDelta {
                id: item_id("assistant-1"),
                text: "async stream.".to_owned(),
            },
        ),
        (
            Duration::from_millis(160),
            ChatTuiAction::MessageFinished {
                id: item_id("assistant-1"),
            },
        ),
        (
            Duration::from_millis(170),
            ChatTuiAction::ToolCallStarted {
                id: item_id("tool-item-1"),
                tool_call_id: "tool-call-1".to_owned(),
                name: "grep".to_owned(),
                arguments: "pattern: fake".to_owned(),
            },
        ),
        (
            Duration::from_millis(200),
            ChatTuiAction::ToolCallDelta {
                tool_call_id: "tool-call-1".to_owned(),
                text: "found match".to_owned(),
            },
        ),
        (
            Duration::from_millis(230),
            ChatTuiAction::ToolCallFinished {
                tool_call_id: "tool-call-1".to_owned(),
                name: "grep".to_owned(),
                output: "found match in src/lib.rs".to_owned(),
            },
        ),
        (
            Duration::from_millis(240),
            ChatTuiAction::CommandStarted {
                id: item_id("command-item-1"),
                command_id: "command-1".to_owned(),
                command: "cargo check -p fake".to_owned(),
            },
        ),
        (
            Duration::from_millis(260),
            ChatTuiAction::CommandOutput {
                command_id: "command-1".to_owned(),
                text: "checking fake workspace\n".to_owned(),
            },
        ),
        (
            Duration::from_millis(300),
            ChatTuiAction::CommandOutput {
                command_id: "command-1".to_owned(),
                text: "finished\n".to_owned(),
            },
        ),
        (
            Duration::from_millis(320),
            ChatTuiAction::CommandFinished {
                command_id: "command-1".to_owned(),
                exit_code: Some(0),
            },
        ),
        (
            Duration::from_millis(330),
            ChatTuiAction::ProviderUsageReported(crate::metadata::ProviderUsageMetadata::new(
                Some(21),
                Some(21),
                Some(42),
            )),
        ),
        (Duration::from_millis(340), ChatTuiAction::AgentFinished),
    ])
}

/// Builds a deterministic fake cancellation lifecycle for terminal-state validation.
pub fn fake_cancellation_plan() -> FakeStreamingPlan {
    FakeStreamingPlan::new(vec![
        (Duration::ZERO, ChatTuiAction::AgentStarted),
        (
            Duration::from_millis(10),
            ChatTuiAction::MessageStarted {
                id: item_id("cancel-message"),
            },
        ),
        (
            Duration::from_millis(20),
            ChatTuiAction::AgentCancelled {
                reason: "cancelled by fake runtime".to_owned(),
            },
        ),
    ])
}

/// Builds a deterministic fake failure lifecycle for terminal-state validation.
pub fn fake_failure_plan() -> FakeStreamingPlan {
    FakeStreamingPlan::new(vec![
        (Duration::ZERO, ChatTuiAction::AgentStarted),
        (
            Duration::from_millis(10),
            ChatTuiAction::AgentFailed {
                message: "fake runtime failure".to_owned(),
            },
        ),
    ])
}

/// Returns the async runtime finding captured for the real runtime integration effort.
pub fn fake_streaming_runtime_finding() -> &'static str {
    "Tokio can drive the fake agent stream and a fixed 90ms spinner ticker while IOCraft rendering remains a pure state projection; keep real runtime work behind an isolated prototype executor until the production IOCraft event loop owns the Tokio select bridge."
}

/// Builds the prototype runtime selection shown by fake streaming state.
fn fake_runtime() -> RuntimeSelection {
    RuntimeSelection::new(
        "fake",
        "FakeProvider",
        "fake-model",
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds the prototype display metadata shown by fake streaming state.
fn fake_display() -> DisplayMetadata {
    DisplayMetadata::new(
        "FakeProvider",
        "fake-model",
        "low",
        "/fake",
        "fake-session",
        None,
    )
}

/// Creates a transcript item ID for fake runtime actions.
fn item_id(value: &str) -> TranscriptItemId {
    TranscriptItemId::new(value)
}
