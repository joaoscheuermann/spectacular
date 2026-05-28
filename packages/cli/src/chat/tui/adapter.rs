#[cfg(test)]
mod actions;
mod commands;
mod events;
mod ids;

#[cfg(test)]
pub(crate) use crate::chat::tui::state::{display_metadata_action, runtime_selection_action};
#[cfg(test)]
pub(crate) use actions::{agent_started_action, session_changed_action, submit_prompt_action};
#[cfg(test)]
pub(crate) use commands::commands_loaded_action;
pub(crate) use commands::commands_loaded_with_completions_action;
pub(crate) use events::TuiEventAdapter;
pub(crate) use ids::transcript_item_id;

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/tui_adapter.rs"
    ));
}
