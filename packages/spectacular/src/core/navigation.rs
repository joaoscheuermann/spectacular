pub(crate) fn previous_index(current: usize, item_count: usize) -> usize {
    if item_count == 0 {
        return 0;
    }

    current.checked_sub(1).unwrap_or(item_count - 1)
}

pub(crate) fn next_index(current: usize, item_count: usize) -> usize {
    if item_count == 0 {
        return 0;
    }

    (current + 1) % item_count
}
