use tui::{
    reduce, Activity, ChatTuiAction, CommandDescriptor, ContextTokenUsage, DisplayMetadata,
    PromptState, ProviderUsageMetadata, ReasoningLevel, RuntimeSelection, SessionId, State, Status,
    TranscriptItemContent, TranscriptItemId, TurnTokenUsage, ViewAction, WorktreeMetadata,
};

/// Builds a representative runtime selection for reducer tests.
fn runtime(provider: &str, model: &str) -> RuntimeSelection {
    RuntimeSelection::new(
        "openai-compatible",
        provider,
        model,
        ReasoningLevel::Low,
        Some(4096),
    )
}

/// Builds visible display metadata for reducer tests.
fn display(provider: &str, model: &str) -> DisplayMetadata {
    DisplayMetadata::new(provider, model, "low", "/workspace", "session", None)
}

/// Builds an initialized state with stable metadata.
fn state() -> State {
    State::new(
        SessionId::new("session-1"),
        runtime("provider", "model"),
        display("provider", "model"),
    )
}

// Verifies initial state has empty collections, supplied metadata, idle status, first spinner frame, and tail following scroll.

#[path = "reducer/reducer_state_prompt_selection.rs"]
mod reducer_state_prompt_selection;
#[path = "reducer/reducer_transcript_runtime.rs"]
mod reducer_transcript_runtime;
#[path = "reducer/reducer_usage_scroll.rs"]
mod reducer_usage_scroll;
