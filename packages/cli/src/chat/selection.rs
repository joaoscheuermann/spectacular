/// Request data for rendering a command-side option selection prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPromptRequest {
    pub title: String,
    pub description: String,
    pub options: Vec<String>,
    pub allow_custom: bool,
    pub allow_comment: bool,
}

impl SelectionPromptRequest {
    /// Creates a selection prompt request with static options and no optional text fields.
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            options,
            allow_custom: false,
            allow_comment: false,
        }
    }

    /// Configures optional custom text input and comment entry.
    pub fn with_inputs(mut self, allow_custom: bool, allow_comment: bool) -> Self {
        self.allow_custom = allow_custom;
        self.allow_comment = allow_comment;
        self
    }
}

/// User-provided answer returned by the command-side selection prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionPromptAnswer {
    pub choice: SelectionPromptChoice,
    pub comment: Option<String>,
}

/// Selected predefined option or custom free-text value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionPromptChoice {
    Option { index: usize, label: String },
    Custom(String),
}
