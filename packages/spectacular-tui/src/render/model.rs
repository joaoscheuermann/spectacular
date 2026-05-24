use crate::metadata::ContextTokenUsage;
use anstyle::Style;
use iocraft::prelude::{Color, MixedTextContent, Weight};

/// Environment variable for overriding IOCraft TUI selected-text foreground color.
pub const TUI_SELECTION_TEXT_COLOR_ENV: &str = "SPECTACULAR_TUI_SELECTION_TEXT_COLOR";
/// Environment variable for overriding IOCraft TUI selected-text background color.
pub const TUI_SELECTION_BACKGROUND_COLOR_ENV: &str = "SPECTACULAR_TUI_SELECTION_BACKGROUND_COLOR";

/// RGB color used by IOCraft TUI rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TuiRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TuiRgb {
    /// Creates an RGB color from component values.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses a hex RGB color in `#RRGGBB` or `RRGGBB` form.
    pub fn from_hex(value: &str) -> Option<Self> {
        let value = value.trim();
        let hex = value.strip_prefix('#').unwrap_or(value);
        if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
            return None;
        }

        Some(Self {
            r: u8::from_str_radix(&hex[0..2], 16).ok()?,
            g: u8::from_str_radix(&hex[2..4], 16).ok()?,
            b: u8::from_str_radix(&hex[4..6], 16).ok()?,
        })
    }

    /// Converts this color into IOCraft's color representation.
    pub fn iocraft_color(self) -> Color {
        Color::Rgb {
            r: self.r,
            g: self.g,
            b: self.b,
        }
    }
}

/// Resolved IOCraft TUI selection colors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TuiSelectionColors {
    pub text: Option<TuiRgb>,
    pub background: TuiRgb,
    pub cursor: TuiRgb,
}

impl TuiSelectionColors {
    /// Creates resolved selection colors from process environment variables.
    pub fn from_env() -> Self {
        let text = std::env::var(TUI_SELECTION_TEXT_COLOR_ENV).ok();
        let background = std::env::var(TUI_SELECTION_BACKGROUND_COLOR_ENV).ok();

        Self::from_env_values(text.as_deref(), background.as_deref())
    }

    /// Creates resolved selection colors from optional env-var values.
    pub fn from_env_values(text: Option<&str>, background: Option<&str>) -> Self {
        let mut colors = Self::default();
        if let Some(text) = text.and_then(TuiRgb::from_hex) {
            colors.text = Some(text);
        }
        if let Some(background) = background.and_then(TuiRgb::from_hex) {
            colors.background = background;
        }

        colors
    }

    /// Returns the selected foreground color after applying explicit overrides.
    pub fn selected_text(self) -> TuiRgb {
        self.text.unwrap_or_else(|| {
            TuiRgb::new(
                255 - self.background.r,
                255 - self.background.g,
                255 - self.background.b,
            )
        })
    }
}

impl Default for TuiSelectionColors {
    fn default() -> Self {
        Self {
            text: None,
            background: TuiRgb::new(179, 179, 179),
            cursor: TuiRgb::new(255, 255, 255),
        }
    }
}

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

/// Extra rendering attributes applied on top of a semantic span style.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderHighlight {
    Selection,
    Cursor,
}

/// One styled text segment in a terminal-flow render line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderSpan {
    pub text: String,
    pub style: RenderStyle,
    pub highlight: Option<RenderHighlight>,
}

impl RenderSpan {
    /// Creates a render span from display text and semantic style.
    pub fn new(text: impl Into<String>, style: RenderStyle) -> Self {
        Self {
            text: text.into(),
            style,
            highlight: None,
        }
    }

    /// Returns this render span with an extra highlight attribute.
    pub fn with_highlight(mut self, highlight: RenderHighlight) -> Self {
        self.highlight = Some(highlight);
        self
    }

    /// Returns this render span marked as selected text.
    pub fn selected(self) -> Self {
        self.with_highlight(RenderHighlight::Selection)
    }

    /// Returns this render span marked as the active cursor cell.
    pub fn cursor(self) -> Self {
        self.with_highlight(RenderHighlight::Cursor)
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
    semantic_iocraft_style_with_selection_colors(style, TuiSelectionColors::default())
}

/// Maps semantic render styles to IOCraft attributes using caller-supplied selection colors.
pub fn semantic_iocraft_style_with_selection_colors(
    style: RenderStyle,
    selection_colors: TuiSelectionColors,
) -> (Option<Color>, Weight) {
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
            Weight::Normal,
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
            Some(selection_colors.selected_text().iocraft_color()),
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
    iocraft_content_with_selection_colors(line, TuiSelectionColors::default())
}

/// Converts a semantic row into IOCraft mixed-text spans with custom selection colors.
pub fn iocraft_content_with_selection_colors(
    line: &RenderLine,
    selection_colors: TuiSelectionColors,
) -> Vec<MixedTextContent> {
    line.spans
        .iter()
        .map(|span| {
            let (color, weight) =
                semantic_iocraft_style_with_selection_colors(span.style, selection_colors);
            let mut content = MixedTextContent::new(&span.text).weight(weight);
            if let Some(color) = color {
                content = content.color(color);
            }
            match span.highlight {
                Some(RenderHighlight::Selection) => {
                    content = content.color(selection_colors.selected_text().iocraft_color());
                    content = content.background_color(selection_colors.background.iocraft_color());
                }
                Some(RenderHighlight::Cursor) => {
                    content = content.background_color(selection_colors.cursor.iocraft_color());
                }
                None if span.style == RenderStyle::Selection => {
                    content = content.background_color(selection_colors.background.iocraft_color());
                }
                None => {}
            }
            content
        })
        .collect()
}

/// Returns the semantic severity for optional context pressure.
pub fn context_pressure_style(usage: Option<ContextTokenUsage>) -> RenderStyle {
    let Some(usage) = usage else {
        return RenderStyle::Dim;
    };
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

/// Returns the semantic severity for a context token usage segment.
pub fn context_usage_style(usage: ContextTokenUsage) -> RenderStyle {
    context_pressure_style(Some(usage))
}
