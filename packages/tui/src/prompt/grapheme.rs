use unicode_segmentation::UnicodeSegmentation;

/// Returns the nearest valid grapheme boundary at or before an offset.
pub(crate) fn clamp_boundary(value: &str, offset: usize) -> usize {
    let offset = offset.min(value.len());
    if offset == value.len() {
        return value.len();
    }

    value
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .take_while(|index| *index <= offset)
        .last()
        .unwrap_or(0)
}

/// Returns the previous grapheme boundary from the supplied cursor.
pub(crate) fn previous_boundary(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    if cursor == 0 {
        return 0;
    }

    value[..cursor]
        .grapheme_indices(true)
        .next_back()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

/// Returns the next grapheme boundary from the supplied cursor.
pub(crate) fn next_boundary(value: &str, cursor: usize) -> usize {
    let cursor = clamp_boundary(value, cursor);
    if cursor >= value.len() {
        return value.len();
    }

    value[cursor..]
        .grapheme_indices(true)
        .nth(1)
        .map(|(index, _)| cursor + index)
        .unwrap_or(value.len())
}

/// Returns the grapheme cluster that starts at the supplied boundary.
pub(crate) fn grapheme_at(value: &str, cursor: usize) -> Option<&str> {
    let cursor = clamp_boundary(value, cursor);
    value[cursor..].graphemes(true).next()
}

/// Iterates over grapheme start offsets and clusters in a range.
pub(crate) fn graphemes_in(value: &str) -> impl Iterator<Item = (usize, &str)> {
    value.grapheme_indices(true)
}
