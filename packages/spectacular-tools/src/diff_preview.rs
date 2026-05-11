use similar::{ChangeTag, TextDiff};

const DEFAULT_DIFF_CONTEXT_LINES: usize = 3;

/// Builds a compact unified diff body and line-count metadata for changed text.
pub(crate) fn diff_preview(old: &str, new: &str) -> FileDiffPreview {
    let diff = TextDiff::from_lines(old, new);
    let changes = diff
        .iter_all_changes()
        .map(|change| DiffChange {
            tag: change.tag(),
            old_index: change.old_index(),
            new_index: change.new_index(),
            text: change.value().trim_end_matches('\n').to_owned(),
        })
        .collect::<Vec<_>>();

    let added = changes
        .iter()
        .filter(|change| change.tag == ChangeTag::Insert)
        .count();
    let removed = changes
        .iter()
        .filter(|change| change.tag == ChangeTag::Delete)
        .count();
    let lines = preview_lines(&changes, DEFAULT_DIFF_CONTEXT_LINES);

    FileDiffPreview {
        added,
        removed,
        lines: lines.join("\n"),
    }
}

/// Display-ready diff body and summary counts for a changed file.
pub(crate) struct FileDiffPreview {
    pub(crate) added: usize,
    pub(crate) removed: usize,
    pub(crate) lines: String,
}

struct DiffChange {
    tag: ChangeTag,
    old_index: Option<usize>,
    new_index: Option<usize>,
    text: String,
}

impl DiffChange {
    /// Formats one diff line with old/new line numbers and the original text.
    fn display_line(&self, width: usize) -> String {
        let number = match self.tag {
            ChangeTag::Delete => self.old_index,
            ChangeTag::Equal | ChangeTag::Insert => self.new_index,
        }
        .map(|index| index + 1)
        .unwrap_or_default();
        let marker = match self.tag {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };

        format!("{number:>width$} {marker}{}", self.text)
    }
}

/// Selects changed lines plus nearby context, inserting gaps between distant hunks.
fn preview_lines(changes: &[DiffChange], context_lines: usize) -> Vec<String> {
    let selected = selected_line_indexes(changes, context_lines);
    if selected.is_empty() {
        return Vec::new();
    }

    let width = line_number_width(changes);
    let mut previous_index = None;
    let mut lines = Vec::new();
    for index in selected {
        if previous_index.is_some_and(|previous| index > previous + 1) {
            lines.push("...".to_owned());
        }
        lines.push(changes[index].display_line(width));
        previous_index = Some(index);
    }

    lines
}

/// Returns the ordered line indexes that should appear in the compact diff.
fn selected_line_indexes(changes: &[DiffChange], context_lines: usize) -> Vec<usize> {
    let mut selected = vec![false; changes.len()];
    for (index, change) in changes.iter().enumerate() {
        if change.tag == ChangeTag::Equal {
            continue;
        }

        let start = index.saturating_sub(context_lines);
        let end = (index + context_lines + 1).min(changes.len());
        for item in selected.iter_mut().take(end).skip(start) {
            *item = true;
        }
    }

    selected
        .into_iter()
        .enumerate()
        .filter_map(|(index, is_selected)| is_selected.then_some(index))
        .collect()
}

/// Determines padding width from the largest visible old or new line number.
fn line_number_width(changes: &[DiffChange]) -> usize {
    let max_line = changes
        .iter()
        .flat_map(|change| [change.old_index, change.new_index])
        .flatten()
        .max()
        .map(|index| index + 1)
        .unwrap_or(0);

    max_line.to_string().len().max(1)
}
