use serde_json::Value;

/// Builds the `(+added -removed)` suffix from a serialized edit output diff.
pub(crate) fn diff_summary(parsed_output: Option<&Value>) -> String {
    let Some(diff) = parsed_output
        .and_then(|output| output.get("diff"))
        .and_then(Value::as_str)
    else {
        return String::new();
    };

    let added = diff.lines().filter(|line| is_added_diff_line(line)).count();
    let removed = diff
        .lines()
        .filter(|line| is_removed_diff_line(line))
        .count();
    if added == 0 && removed == 0 {
        return String::new();
    }

    format!(" (+{added} -{removed})")
}

/// Reports whether a formatted diff line represents an insertion.
fn is_added_diff_line(line: &str) -> bool {
    line.split_whitespace().nth(1) == Some("+")
}

/// Reports whether a formatted diff line represents a deletion.
fn is_removed_diff_line(line: &str) -> bool {
    line.split_whitespace().nth(1) == Some("-")
}
