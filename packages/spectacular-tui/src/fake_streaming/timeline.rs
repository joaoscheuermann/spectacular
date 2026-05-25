use super::plan::FakeStreamingPlan;
use crate::action::ChatTuiAction;
use crate::ids::SessionId;
use crate::metadata::{DisplayMetadata, ReasoningLevel, RuntimeSelection};
use crate::reducer::reduce;
use crate::render::render_state_to_string;
use crate::runtime::SPINNER_TICK_INTERVAL;
use crate::session::PromptState;
use crate::state::State;
use crate::status::Status;
use crate::transcript::{
    CommandItem, CommandStatus, ToolCallItem, ToolStatus, TranscriptItemContent,
};
use std::time::Duration;

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
            next_spinner_tick: SPINNER_TICK_INTERVAL,
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
        self.next_spinner_tick += SPINNER_TICK_INTERVAL;
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
