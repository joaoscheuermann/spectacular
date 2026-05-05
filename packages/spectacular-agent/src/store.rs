use crate::event::AgentEvent;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Store {
    events: Vec<AgentEvent>,
}

impl Store {
    pub fn from_events(events: Vec<AgentEvent>) -> Self {
        Self { events }
    }

    pub fn append(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }

    pub fn checkpoint(&self) -> usize {
        self.events.len()
    }

    pub fn rollback(&mut self, checkpoint: usize) {
        self.events.truncate(checkpoint);
    }
}

impl From<Vec<AgentEvent>> for Store {
    fn from(events: Vec<AgentEvent>) -> Self {
        Self::from_events(events)
    }
}
