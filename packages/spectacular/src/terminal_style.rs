use anstyle::{RgbColor, Style};

const TEXT: RgbColor = RgbColor(229, 231, 235);
const DIM: RgbColor = RgbColor(148, 163, 184);
const GREEN: RgbColor = RgbColor(34, 197, 94);
const YELLOW: RgbColor = RgbColor(234, 179, 8);
const RED: RgbColor = RgbColor(248, 113, 113);
const MAGENTA: RgbColor = RgbColor(217, 70, 239);
const CYAN: RgbColor = RgbColor(34, 211, 238);
const BLUE: RgbColor = RgbColor(96, 165, 250);
const ORANGE: RgbColor = RgbColor(251, 191, 36);
const SELECTION_BACKGROUND: RgbColor = RgbColor(51, 65, 85);

/// Applies a terminal style and reset sequence around a display string.
pub fn paint(style: Style, value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    format!("{style}{value}{style:#}")
}

/// Returns the default high-contrast style for primary terminal text.
pub fn text_style() -> Style {
    TEXT.on_default()
}

/// Returns the muted style for secondary labels, hints, and metadata.
pub fn dim_style() -> Style {
    DIM.on_default()
}

/// Returns the bold green style for successful status messages.
pub fn success_style() -> Style {
    GREEN.on_default().bold()
}

/// Returns the bold yellow style for warnings and cancellation notices.
pub fn warning_style() -> Style {
    YELLOW.on_default().bold()
}

/// Returns the bold red style for error messages.
pub fn error_style() -> Style {
    RED.on_default().bold()
}

/// Returns the green style for user-authored prompt text.
pub fn user_style() -> Style {
    GREEN.on_default()
}

/// Returns the primary text style for assistant responses.
pub fn assistant_style() -> Style {
    text_style()
}

/// Returns the bold magenta style for tool names and task-like labels.
pub fn tool_style() -> Style {
    MAGENTA.on_default().bold()
}

/// Returns the bold green style for product and section titles.
pub fn title_style() -> Style {
    GREEN.on_default().bold()
}

/// Returns the bold cyan style for provider names.
pub fn provider_style() -> Style {
    CYAN.on_default().bold()
}

/// Returns the bold magenta style for task names.
pub fn task_style() -> Style {
    MAGENTA.on_default().bold()
}

/// Returns the primary text style for model identifiers.
pub fn model_style() -> Style {
    text_style()
}

/// Returns the orange style for masked or secret-adjacent values.
pub fn secret_style() -> Style {
    ORANGE.on_default()
}

/// Returns the blue style for low-effort reasoning indicators.
pub fn low_reasoning_style() -> Style {
    BLUE.on_default()
}

/// Returns the inverse selection style for highlighted prompt text.
pub fn selection_style() -> Style {
    TEXT.on(SELECTION_BACKGROUND)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/terminal_style.rs"
    ));
}
