use crate::prompt::grapheme::clamp_boundary;
use crate::session::{PromptHistoryEntry, PromptState};

impl PromptState {
    /// Restores the previous prompt content snapshot when available.
    pub fn undo(&mut self) {
        self.record_history_boundary();
        let Some(entry) = self.undo_stack.pop() else {
            return;
        };

        self.redo_stack.push(self.history_entry());
        self.restore_history_entry(entry);
    }

    /// Restores the next prompt content snapshot after an undo.
    pub fn redo(&mut self) {
        self.record_history_boundary();
        let Some(entry) = self.redo_stack.pop() else {
            return;
        };

        self.undo_stack.push(self.history_entry());
        self.restore_history_entry(entry);
    }

    /// Captures undo state before an edit, grouping adjacent plain character insertions.
    pub(super) fn prepare_edit(&mut self, grouped: bool) {
        if grouped {
            if self.history_group.is_none() {
                self.history_group = Some(self.history_entry());
            }
        } else {
            self.record_history_boundary();
            self.undo_stack.push(self.history_entry());
        }
        self.redo_stack.clear();
    }

    /// Commits active grouped insertion history to the undo stack.
    pub(super) fn record_history_boundary(&mut self) {
        if let Some(entry) = self.history_group.take() {
            self.undo_stack.push(entry);
        }
    }

    /// Captures the content and cursor state used by history stacks.
    fn history_entry(&self) -> PromptHistoryEntry {
        PromptHistoryEntry {
            lines: self.lines.clone(),
            cursor: self.cursor,
        }
    }

    /// Restores a history snapshot and clears transient selection/navigation state.
    fn restore_history_entry(&mut self, entry: PromptHistoryEntry) {
        let lines = if entry.lines.is_empty() {
            vec![String::new()]
        } else {
            entry.lines
        };
        self.set_lines(lines);
        self.cursor = clamp_boundary(&self.text(), entry.cursor);
        self.preferred_column = None;
        self.selection_anchor = None;
        self.selected_completion = 0;
        self.dismissed_completion = None;
        self.scroll_top_row = 0;
    }
}
