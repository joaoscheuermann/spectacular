use crate::event::AgentEvent;

/// In-memory durable event log for one agent conversation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Store {
    events: Vec<AgentEvent>,
}

impl Store {
    /// Builds a store from already persisted events.
    pub fn from_events(events: Vec<AgentEvent>) -> Self {
        Self { events }
    }

    /// Appends one event to the end of the log.
    pub fn append(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    /// Returns all stored events in replay order.
    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }

    /// Returns an offset that can later be used to roll back speculative events.
    pub fn checkpoint(&self) -> usize {
        self.events.len()
    }

    /// Truncates the log back to a previous checkpoint.
    pub fn rollback(&mut self, checkpoint: usize) {
        self.events.truncate(checkpoint);
    }
}

impl From<Vec<AgentEvent>> for Store {
    fn from(events: Vec<AgentEvent>) -> Self {
        Self::from_events(events)
    }
}
