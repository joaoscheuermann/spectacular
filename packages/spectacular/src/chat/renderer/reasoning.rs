use super::style::{dim_style, paint};

/// Formats a complete model reasoning block for replay and standalone rendering.
pub(super) fn format_reasoning_text(content: &str) -> Option<String> {
    if !has_visible_reasoning_text(content) {
        return None;
    }

    Some(paint(dim_style(), content))
}

/// Reports whether a reasoning delta contains visible non-whitespace text.
pub(crate) fn has_visible_reasoning_text(content: &str) -> bool {
    !content.trim().is_empty()
}
