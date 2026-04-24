mod keys;
mod navigation;
pub mod screens;
mod terminal;
pub mod widgets;

pub use screens::api_key::{
    render_api_key_screen, run_api_key_screen, submit_api_key_input, ApiKeyInputError,
};
pub use screens::model_assignment::{
    render_model_assignment_screen, run_model_assignment_screen, ModelAssignmentError, ModelOption,
    TaskModelSelection, TASK_MODEL_SLOT_COUNT,
};
pub use screens::provider_selection::{
    mask_api_key, render_provider_selection, run_provider_selection_screen, select_provider_id,
    ProviderOption, ProviderSelectionError,
};
