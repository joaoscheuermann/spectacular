use super::directory::format_directory;
use super::style::{dim_style, paint};
use super::token_usage::format_context_token_usage;
use crate::chat::model::ChatPromptFooterModel;
use spectacular_agent::ContextTokenUsage;

/// Display-ready values for the footer rendered below newly submitted prompts.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct UserPromptFooterView {
    pub(super) directory: String,
    pub(super) model: String,
    pub(super) reasoning: String,
    pub(super) token_usage: Option<ContextTokenUsage>,
}

impl UserPromptFooterView {
    /// Builds a terminal footer view from prompt footer request data.
    pub(super) fn from_model(model: &ChatPromptFooterModel) -> Self {
        Self {
            directory: format_directory(&model.directory),
            model: model.model.clone(),
            reasoning: model.reasoning.to_string(),
            token_usage: model.token_usage,
        }
    }
}

/// Formats prompt footer values as one compact terminal line.
pub(super) fn format_user_prompt_footer(view: &UserPromptFooterView) -> String {
    let prefix = paint(
        dim_style(),
        format!("{} · {} ({})", view.directory, view.model, view.reasoning),
    );
    match view.token_usage {
        Some(usage) => format!(
            "{}{}{}",
            prefix,
            paint(dim_style(), " · "),
            format_context_token_usage(usage)
        ),
        None => prefix,
    }
}
