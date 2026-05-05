use spectacular_agent::{AgentEvent, Store};
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
