use super::directory::format_directory;
use crate::chat::model::ChatPromptFooterModel;

/// Display-ready values for the footer rendered below newly submitted prompts.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct UserPromptFooterView {
    pub(super) directory: String,
    pub(super) model: String,
    pub(super) reasoning: String,
}

impl UserPromptFooterView {
    /// Builds a terminal footer view from prompt footer request data.
    pub(super) fn from_model(model: &ChatPromptFooterModel) -> Self {
        Self {
            directory: format_directory(&model.directory),
            model: model.model.clone(),
            reasoning: model.reasoning.to_string(),
        }
    }
}

/// Formats prompt footer values as one compact terminal line.
pub(super) fn format_user_prompt_footer(view: &UserPromptFooterView) -> String {
    format!("{} · {} ({})", view.directory, view.model, view.reasoning)
}
