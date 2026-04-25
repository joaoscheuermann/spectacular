use crate::event::AgentEvent;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Store {
    events: Vec<AgentEvent>,
}

impl Store {
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

#[cfg(test)]
mod tests {
    use super::*;
    use spectacular_llms::FinishReason;

    #[test]
    fn append_preserves_order() {
        let mut store = Store::default();

        store.append(AgentEvent::user_prompt("first"));
        store.append(AgentEvent::Finished {
            finish_reason: FinishReason::Stop,
        });

        assert_eq!(
            store.events(),
            &[
                AgentEvent::user_prompt("first"),
                AgentEvent::Finished {
                    finish_reason: FinishReason::Stop
                },
            ]
        );
    }

    #[test]
    fn rollback_discards_events_after_checkpoint() {
        let mut store = Store::default();
        store.append(AgentEvent::user_prompt("kept"));
        let checkpoint = store.checkpoint();
        store.append(AgentEvent::user_prompt("discarded"));

        store.rollback(checkpoint);

        assert_eq!(store.events(), &[AgentEvent::user_prompt("kept")]);
    }
}
