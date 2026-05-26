use anstyle::{RgbColor, Style};

const TEXT: RgbColor = RgbColor(229, 231, 235);
const DIM: RgbColor = RgbColor(148, 163, 184);
const RED: RgbColor = RgbColor(248, 113, 113);
const MAGENTA: RgbColor = RgbColor(217, 70, 239);

pub(crate) fn paint(style: Style, value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    format!("{style}{value}{style:#}")
}

#[cfg(test)]
pub(crate) fn text_style() -> Style {
    TEXT.on_default()
}

#[cfg(test)]
pub(crate) fn dim_style() -> Style {
    DIM.on_default()
}

pub(crate) fn error_style() -> Style {
    RED.on_default().bold()
}

pub(crate) fn tool_style() -> Style {
    MAGENTA.on_default().bold()
}

#[cfg(test)]
pub(crate) fn tool_name_style() -> Style {
    tool_style()
}

pub(crate) fn tool_line(label: &str, input: &str, metadata: Option<&str>) -> String {
    let mut output = Vec::new();
    push_styled_if_visible(&mut output, tool_style(), label);
    push_styled_if_visible(&mut output, TEXT.on_default(), input);
    if let Some(metadata) = metadata {
        push_styled_if_visible(&mut output, DIM.on_default(), metadata);
    }
    output.join(" ")
}

pub(crate) fn tool_arg_line(label: &str, input: &str) -> String {
    tool_line(label, input, None)
}

pub(crate) fn tool_arg_tool_arg_line(
    label: &str,
    first_argument: &str,
    second_label: &str,
    second_argument: &str,
) -> String {
    let mut output = Vec::new();
    push_styled_if_visible(&mut output, tool_style(), label);
    push_styled_if_visible(&mut output, TEXT.on_default(), first_argument);
    push_styled_if_visible(&mut output, tool_style(), second_label);
    push_styled_if_visible(&mut output, TEXT.on_default(), second_argument);
    output.join(" ")
}

fn push_styled_if_visible(output: &mut Vec<String>, style: Style, value: &str) {
    if value.trim().is_empty() {
        return;
    }

    output.push(paint(style, value));
}
