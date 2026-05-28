use super::measure::transcript_item_row_count_for_width;
use super::model::TranscriptItem;
use crate::ids::SessionId;
use crate::state::State;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

const MAX_TRANSCRIPT_CHANGE_LOG: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TranscriptChange {
    revision: u64,
    start_index: usize,
}

/// Row-aware transcript layout used by virtualized transcript rendering.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TranscriptLayout {
    pub(crate) total_rows: usize,
    pub(crate) items: Vec<TranscriptItemLayout>,
}

impl TranscriptLayout {
    /// Builds cumulative row metadata for transcript items at a known content width.
    pub(crate) fn for_state(state: &State, width: usize) -> Self {
        let mut next_start_row = 0usize;
        let items = state
            .session
            .transcript
            .iter()
            .enumerate()
            .map(|(item_index, item)| {
                let row_count = transcript_item_row_count_for_width(item, width);
                let layout = TranscriptItemLayout {
                    item_index,
                    start_row: next_start_row,
                    row_count,
                };
                next_start_row = next_start_row.saturating_add(row_count);
                layout
            })
            .collect();

        Self {
            total_rows: next_start_row,
            items,
        }
    }

    /// Returns the item indices intersecting a half-open virtual row window.
    pub(crate) fn item_range(&self, rows: Range<usize>) -> Range<usize> {
        if rows.start >= rows.end || self.items.is_empty() {
            return 0..0;
        }

        let start = self
            .items
            .partition_point(|item| item.end_row() <= rows.start);
        let end = self.items.partition_point(|item| item.start_row < rows.end);

        start..end.max(start)
    }

    /// Returns the virtual row where an item starts, or zero for an empty range.
    pub(crate) fn item_start_row(&self, item_index: usize) -> usize {
        self.items
            .get(item_index)
            .map(|item| item.start_row)
            .unwrap_or_default()
    }
}

/// Incremental transcript layout cache keyed by terminal content width and view-local mutations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TranscriptLayoutCache {
    snapshot: Option<TranscriptLayoutSnapshot>,
    revision: u64,
    changes: Vec<TranscriptChange>,
}

impl TranscriptLayoutCache {
    /// Records the earliest transcript index affected by one semantic mutation.
    pub(crate) fn note_change(&mut self, start_index: usize) {
        self.revision = self.revision.saturating_add(1);
        self.changes.push(TranscriptChange {
            revision: self.revision,
            start_index,
        });
        if self.changes.len() > MAX_TRANSCRIPT_CHANGE_LOG {
            let excess = self.changes.len().saturating_sub(MAX_TRANSCRIPT_CHANGE_LOG);
            self.changes.drain(0..excess);
        }
    }

    /// Clears cached layout metadata after a session replacement.
    pub(crate) fn reset_for_session(&mut self) {
        self.snapshot = None;
        self.revision = self.revision.saturating_add(1);
        self.changes.clear();
    }

    /// Returns shared row layout metadata, recomputing only the changed suffix when possible.
    pub(crate) fn snapshot_for_state(
        &mut self,
        state: &State,
        width: usize,
    ) -> TranscriptLayoutSnapshot {
        if let Some(snapshot) = self.reusable_snapshot(state, width) {
            return snapshot;
        }

        let snapshot = match self.changed_suffix_start(state, width) {
            Some(start_index) => self.recompute_suffix(state, width, start_index),
            None => Self::rebuilt_snapshot(state, width, self.revision),
        };
        self.snapshot = Some(snapshot.clone());
        snapshot
    }

    fn reusable_snapshot(&self, state: &State, width: usize) -> Option<TranscriptLayoutSnapshot> {
        let snapshot = self.snapshot.as_ref()?;
        if snapshot.session_id != state.session.id
            || snapshot.width != width
            || snapshot.item_count != state.session.transcript.len()
        {
            return None;
        }

        if snapshot.revision != self.revision {
            return None;
        }

        if snapshot.tail_fingerprint()
            != state
                .session
                .transcript
                .last()
                .map(|item| TranscriptItemFingerprint::for_item(item, width))
        {
            return None;
        }

        Some(snapshot.clone())
    }

    fn changed_suffix_start(&self, state: &State, width: usize) -> Option<usize> {
        let snapshot = self.snapshot.as_ref()?;
        if snapshot.session_id != state.session.id || snapshot.width != width {
            return None;
        }

        if snapshot.revision < self.revision {
            return self
                .change_start_since(snapshot.revision)
                .map(|start| start.min(snapshot.layout.items.len()));
        }

        self.untracked_change_start(state, snapshot, width)
    }

    fn change_start_since(&self, revision: u64) -> Option<usize> {
        if revision == self.revision {
            return Some(usize::MAX);
        }

        let first = self.changes.first()?;
        if first.revision > revision.saturating_add(1) {
            return None;
        }

        self.changes
            .iter()
            .filter(|change| change.revision > revision)
            .map(|change| change.start_index)
            .min()
    }

    fn untracked_change_start(
        &self,
        state: &State,
        snapshot: &TranscriptLayoutSnapshot,
        width: usize,
    ) -> Option<usize> {
        let current_count = state.session.transcript.len();
        if current_count < snapshot.item_count {
            return None;
        }

        if current_count > snapshot.item_count {
            return appended_suffix_start(state, snapshot, width);
        }

        let last_index = current_count.checked_sub(1)?;
        let current =
            TranscriptItemFingerprint::for_item(&state.session.transcript[last_index], width);
        (snapshot.fingerprints.get(last_index).copied() != Some(current)).then_some(last_index)
    }

    fn recompute_suffix(
        &self,
        state: &State,
        width: usize,
        start_index: usize,
    ) -> TranscriptLayoutSnapshot {
        let Some(previous) = self.snapshot.as_ref() else {
            return Self::rebuilt_snapshot(state, width, self.revision);
        };
        if start_index == usize::MAX {
            return previous.clone();
        }

        let prefix = start_index.min(previous.layout.items.len());
        let mut items = previous.layout.items[..prefix].to_vec();
        let mut fingerprints = previous.fingerprints[..prefix].to_vec();
        let mut next_start_row = items
            .last()
            .map(TranscriptItemLayout::end_row)
            .unwrap_or_default();

        for (item_index, item) in state.session.transcript.iter().enumerate().skip(prefix) {
            let row_count = transcript_item_row_count_for_width(item, width);
            items.push(TranscriptItemLayout {
                item_index,
                start_row: next_start_row,
                row_count,
            });
            fingerprints.push(TranscriptItemFingerprint::from_row_count(item, row_count));
            next_start_row = next_start_row.saturating_add(row_count);
        }

        TranscriptLayoutSnapshot {
            session_id: state.session.id.clone(),
            width,
            revision: self.revision,
            item_count: state.session.transcript.len(),
            layout: Arc::new(TranscriptLayout {
                total_rows: next_start_row,
                items,
            }),
            fingerprints: Arc::new(fingerprints),
        }
    }

    fn rebuilt_snapshot(state: &State, width: usize, revision: u64) -> TranscriptLayoutSnapshot {
        let layout = TranscriptLayout::for_state(state, width);
        let fingerprints = state
            .session
            .transcript
            .iter()
            .zip(layout.items.iter())
            .map(|(item, layout)| TranscriptItemFingerprint::from_row_count(item, layout.row_count))
            .collect();

        TranscriptLayoutSnapshot {
            session_id: state.session.id.clone(),
            width,
            revision,
            item_count: state.session.transcript.len(),
            layout: Arc::new(layout),
            fingerprints: Arc::new(fingerprints),
        }
    }
}

/// Shared transcript row metadata snapshot for one state revision and content width.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptLayoutSnapshot {
    pub(crate) session_id: SessionId,
    pub(crate) width: usize,
    pub(crate) revision: u64,
    pub(crate) item_count: usize,
    pub(crate) layout: Arc<TranscriptLayout>,
    fingerprints: Arc<Vec<TranscriptItemFingerprint>>,
}

impl TranscriptLayoutSnapshot {
    fn tail_fingerprint(&self) -> Option<TranscriptItemFingerprint> {
        self.fingerprints.last().copied()
    }
}

/// Cumulative row metadata for one semantic transcript item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptItemLayout {
    pub(crate) item_index: usize,
    pub(crate) start_row: usize,
    pub(crate) row_count: usize,
}

impl TranscriptItemLayout {
    /// Returns the first row after this item in the virtual transcript coordinate space.
    fn end_row(&self) -> usize {
        self.start_row.saturating_add(self.row_count)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TranscriptItemFingerprint {
    id_hash: u64,
    row_count: usize,
}

impl TranscriptItemFingerprint {
    fn for_item(item: &TranscriptItem, width: usize) -> Self {
        Self::from_row_count(item, transcript_item_row_count_for_width(item, width))
    }

    fn from_row_count(item: &TranscriptItem, row_count: usize) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        item.id.as_str().hash(&mut hasher);
        Self {
            id_hash: hasher.finish(),
            row_count,
        }
    }
}

fn appended_suffix_start(
    state: &State,
    snapshot: &TranscriptLayoutSnapshot,
    width: usize,
) -> Option<usize> {
    if snapshot.item_count == 0 {
        return Some(0);
    }

    let previous_tail = snapshot.tail_fingerprint()?;
    let current_previous_tail = TranscriptItemFingerprint::for_item(
        &state.session.transcript[snapshot.item_count - 1],
        width,
    );
    (previous_tail == current_previous_tail).then_some(snapshot.item_count)
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/transcript_layout_cache.rs"
    ));
}
