use crate::scroll::TranscriptScrollState;
use std::ops::Range;

const DEFAULT_TRANSCRIPT_WINDOW_ROWS: usize = 200;

/// Returns the semantic transcript item range to materialize for the current viewport.
pub(crate) fn visible_transcript_range(
    transcript_len: usize,
    scroll: &TranscriptScrollState,
) -> Range<usize> {
    let visible_rows = visible_transcript_row_count(scroll);
    if visible_rows >= transcript_len {
        return 0..transcript_len;
    }

    let max_offset = transcript_len.saturating_sub(visible_rows);
    let offset = (scroll.offset as usize).min(max_offset);
    let end = transcript_len.saturating_sub(offset);
    let start = end.saturating_sub(visible_rows);
    start..end
}

/// Returns the known viewport height or the bounded pre-resize default window.
fn visible_transcript_row_count(scroll: &TranscriptScrollState) -> usize {
    if scroll.visible_rows > 0 {
        return scroll.visible_rows as usize;
    }

    DEFAULT_TRANSCRIPT_WINDOW_ROWS
}
