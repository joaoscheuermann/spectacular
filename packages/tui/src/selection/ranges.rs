use std::ops::Range;

pub(super) fn selectable_range(start: usize, end: usize) -> Range<usize> {
    let start = start.min(end);
    start..end
}

pub(super) fn split_around_excluded(
    range: Range<usize>,
    excluded: &[Range<usize>],
) -> Vec<Range<usize>> {
    if range.is_empty() {
        return Vec::new();
    }

    let mut segments = vec![range];
    let mut excluded = excluded.to_vec();
    excluded.sort_by_key(|range| (range.start, range.end));
    for excluded in excluded {
        segments = segments
            .into_iter()
            .flat_map(|segment| subtract_range(segment, excluded.clone()))
            .collect();
    }

    segments
        .into_iter()
        .filter(|range| !range.is_empty())
        .collect()
}

pub(super) fn intersect_range(left: Range<usize>, right: Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then_some(start..end)
}

pub(super) fn nearest_selectable_boundary(
    column: usize,
    segments: &[Range<usize>],
) -> Option<usize> {
    let first = segments.first()?;
    if column <= first.start {
        return Some(first.start);
    }

    for pair in segments.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        if column <= left.end {
            return Some(column);
        }
        if column < right.start {
            return Some(nearest_gap_boundary(column, left.end, right.start));
        }
    }

    let last = segments.last()?;
    Some(column.min(last.end))
}

fn subtract_range(range: Range<usize>, excluded: Range<usize>) -> Vec<Range<usize>> {
    let Some(overlap) = intersect_range(range.clone(), excluded) else {
        return vec![range];
    };

    let mut output = Vec::new();
    if range.start < overlap.start {
        output.push(range.start..overlap.start);
    }
    if overlap.end < range.end {
        output.push(overlap.end..range.end);
    }

    output
}

fn nearest_gap_boundary(column: usize, left_end: usize, right_start: usize) -> usize {
    if column.saturating_sub(left_end) <= right_start.saturating_sub(column) {
        return left_end;
    }

    right_start
}
