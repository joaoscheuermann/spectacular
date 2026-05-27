use spectacular_tui::TranscriptItemId;

/// Converts an agent transcript item identifier into the TUI identifier type.
pub(crate) fn transcript_item_id(id: &str) -> TranscriptItemId {
    TranscriptItemId::new(id)
}
