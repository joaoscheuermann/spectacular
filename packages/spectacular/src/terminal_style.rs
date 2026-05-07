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

pub fn paint(style: Style, value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    format!("{style}{value}{style:#}")
}

pub fn text_style() -> Style {
    TEXT.on_default()
}

pub fn dim_style() -> Style {
    DIM.on_default()
}

pub fn success_style() -> Style {
    GREEN.on_default().bold()
}

pub fn warning_style() -> Style {
    YELLOW.on_default().bold()
}

pub fn error_style() -> Style {
    RED.on_default().bold()
}

pub fn user_style() -> Style {
    GREEN.on_default()
}

pub fn assistant_style() -> Style {
    text_style()
}

pub fn tool_style() -> Style {
    MAGENTA.on_default().bold()
}

pub fn title_style() -> Style {
    GREEN.on_default().bold()
}

pub fn provider_style() -> Style {
    CYAN.on_default().bold()
}

pub fn task_style() -> Style {
    MAGENTA.on_default().bold()
}

pub fn model_style() -> Style {
    text_style()
}

pub fn secret_style() -> Style {
    ORANGE.on_default()
}

pub fn low_reasoning_style() -> Style {
    BLUE.on_default()
}

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
