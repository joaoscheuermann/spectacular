use anstyle::{RgbColor, Style};

const GREEN: RgbColor = RgbColor(34, 197, 94);
const RED: RgbColor = RgbColor(248, 113, 113);
#[cfg(test)]
const CYAN: RgbColor = RgbColor(34, 211, 238);

pub(crate) fn paint(style: Style, value: impl AsRef<str>) -> String {
    format!("{style}{}{style:#}", value.as_ref())
}

pub(crate) fn success_style() -> Style {
    GREEN.on_default().bold()
}

pub(crate) fn error_style() -> Style {
    RED.on_default().bold()
}

#[cfg(test)]
pub(crate) fn tool_name_style() -> Style {
    CYAN.on_default().bold()
}
