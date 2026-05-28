use crate::action::ChatTuiAction;
use std::time::Duration;

/// Deterministic fake runtime event schedule consumed by the prototype harness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FakeStreamingPlan {
    pub(super) scheduled_actions: Vec<ScheduledAction>,
}

/// One reducer action scheduled at a fake elapsed time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScheduledAction {
    pub(super) at: Duration,
    pub(super) action: ChatTuiAction,
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
