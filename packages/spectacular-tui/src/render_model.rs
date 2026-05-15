use crate::metadata::ContextTokenUsage;
use anstyle::Style;
use iocraft::prelude::{Color, MixedTextContent, Weight};

/// Semantic style categories used by active TUI render lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderStyle {
    Text,
    Dim,
    Title,
    Provider,
    Model,
    Task,
    User,
    Assistant,
    Reasoning,
    Tool,
    Command,
    CommandOutput,
    Success,
    Warning,
    Error,
    Selection,
    DiffAdded,
    DiffRemoved,
    Secret,
}

/// One styled text segment in a terminal-flow render line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSpan {
    pub text: String,
    pub style: RenderStyle,
}

impl RenderSpan {
    /// Creates a render span from display text and semantic style.
    pub fn new(text: impl Into<String>, style: RenderStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// One terminal-flow row made from ordered semantic text spans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderLine {
    pub spans: Vec<RenderSpan>,
}

impl RenderLine {
    /// Creates a render line from prebuilt semantic spans.
    pub fn from_spans(spans: Vec<RenderSpan>) -> Self {
        Self { spans }
    }

    /// Creates a single-style render line from display text.
    pub fn styled(text: impl Into<String>, style: RenderStyle) -> Self {
        Self::from_spans(vec![RenderSpan::new(text, style)])
    }

    /// Creates a primary-text render line from display text.
    pub fn text(text: impl Into<String>) -> Self {
        Self::styled(text, RenderStyle::Text)
    }

    /// Flattens styled spans to visible text for snapshots and plain IOCraft output.
    pub fn plain_text(&self) -> String {
        self.spans
            .iter()
            .map(|span| span.text.as_str())
            .collect::<String>()
    }
}

/// Maps semantic render styles to existing terminal ANSI styles for compatibility output.
pub fn semantic_ansi_style(style: RenderStyle) -> Style {
    match style {
        RenderStyle::Text | RenderStyle::Assistant | RenderStyle::Model => crate::text_style(),
        RenderStyle::Dim | RenderStyle::Reasoning => crate::dim_style(),
        RenderStyle::Title => crate::title_style(),
        RenderStyle::Provider => crate::provider_style(),
        RenderStyle::Task => crate::task_style(),
        RenderStyle::User => crate::user_style(),
        RenderStyle::Tool => crate::tool_style(),
        RenderStyle::Command => crate::command_style(),
        RenderStyle::CommandOutput => crate::command_output_style(),
        RenderStyle::Success => crate::success_style(),
        RenderStyle::Warning => crate::warning_style(),
        RenderStyle::Error => crate::error_style(),
        RenderStyle::Selection => crate::selection_style(),
        RenderStyle::DiffAdded => crate::diff_added_style(),
        RenderStyle::DiffRemoved => crate::diff_removed_style(),
        RenderStyle::Secret => crate::secret_style(),
    }
}

/// Maps semantic render styles to IOCraft color and weight attributes.
pub fn semantic_iocraft_style(style: RenderStyle) -> (Option<Color>, Weight) {
    match style {
        RenderStyle::Text | RenderStyle::Assistant | RenderStyle::Model => (
            Some(Color::Rgb {
                r: 229,
                g: 231,
                b: 235,
            }),
            Weight::Normal,
        ),
        RenderStyle::Dim | RenderStyle::Reasoning => (
            Some(Color::Rgb {
                r: 148,
                g: 163,
                b: 184,
            }),
            Weight::Light,
        ),
        RenderStyle::Title => (
            Some(Color::Rgb {
                r: 34,
                g: 197,
                b: 94,
            }),
            Weight::Bold,
        ),
        RenderStyle::Provider => (
            Some(Color::Rgb {
                r: 34,
                g: 211,
                b: 238,
            }),
            Weight::Bold,
        ),
        RenderStyle::Task | RenderStyle::Tool => (
            Some(Color::Rgb {
                r: 217,
                g: 70,
                b: 239,
            }),
            Weight::Bold,
        ),
        RenderStyle::User | RenderStyle::DiffAdded => (
            Some(Color::Rgb {
                r: 34,
                g: 197,
                b: 94,
            }),
            Weight::Normal,
        ),
        RenderStyle::Success => (
            Some(Color::Rgb {
                r: 34,
                g: 197,
                b: 94,
            }),
            Weight::Bold,
        ),
        RenderStyle::Command => (
            Some(Color::Rgb {
                r: 96,
                g: 165,
                b: 250,
            }),
            Weight::Bold,
        ),
        RenderStyle::CommandOutput => (
            Some(Color::Rgb {
                r: 107,
                g: 114,
                b: 128,
            }),
            Weight::Normal,
        ),
        RenderStyle::Warning => (
            Some(Color::Rgb {
                r: 234,
                g: 179,
                b: 8,
            }),
            Weight::Bold,
        ),
        RenderStyle::Error | RenderStyle::DiffRemoved => (
            Some(Color::Rgb {
                r: 248,
                g: 113,
                b: 113,
            }),
            Weight::Bold,
        ),
        RenderStyle::Selection => (
            Some(Color::Rgb {
                r: 229,
                g: 231,
                b: 235,
            }),
            Weight::Bold,
        ),
        RenderStyle::Secret => (
            Some(Color::Rgb {
                r: 251,
                g: 191,
                b: 36,
            }),
            Weight::Normal,
        ),
    }
}

/// Converts a semantic row into IOCraft mixed-text spans.
pub fn iocraft_content(line: &RenderLine) -> Vec<MixedTextContent> {
    line.spans
        .iter()
        .map(|span| {
            let (color, weight) = semantic_iocraft_style(span.style);
            let mut content = MixedTextContent::new(&span.text).weight(weight);
            if let Some(color) = color {
                content = content.color(color);
            }
            content
        })
        .collect()
}

/// Returns the semantic severity for a context token usage segment.
pub fn context_usage_style(usage: ContextTokenUsage) -> RenderStyle {
    let Some(window) = usage.context_window_tokens else {
        return RenderStyle::Dim;
    };
    if window == 0 {
        return RenderStyle::Dim;
    }

    let ratio_percent = usage.input_tokens.saturating_mul(100) / window;
    if ratio_percent >= 90 {
        return RenderStyle::Error;
    }
    if ratio_percent >= 80 {
        return RenderStyle::Warning;
    }

    RenderStyle::Dim
}
