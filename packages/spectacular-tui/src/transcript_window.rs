use crate::scroll::TranscriptScrollState;

const DEFAULT_TRANSCRIPT_WINDOW_ROWS: usize = 200;

/// Returns the known viewport height or the bounded pre-resize default window.
pub(crate) fn visible_transcript_row_count(scroll: &TranscriptScrollState) -> usize {
    if scroll.visible_rows > 0 {
        return scroll.visible_rows as usize;
    }

    DEFAULT_TRANSCRIPT_WINDOW_ROWS
}
