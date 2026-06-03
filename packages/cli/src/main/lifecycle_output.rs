use super::{
    lifecycle::{
        redact_text, LifecycleAnswerResponse, LifecycleDispatchMode, LifecycleDispatchResponse,
        LifecycleListResponse, LifecycleStreamItem, LifecycleWorkerEvent, LifecycleWorkerSummary,
    },
    terminal_style,
};
use ::lifecycle::terminal::safe_message;
use anstyle::Style;

pub(super) fn format_connecting_line() -> &'static str {
    "connecting to daemon"
}

pub(super) fn format_connected_line() -> &'static str {
    "connected to daemon"
}

pub(super) fn format_creating_worker_line() -> &'static str {
    "creating worker"
}

pub(super) fn format_created_worker_line(worker_id: &str) -> String {
    format!("created worker: {}", safe_cell(worker_id))
}

pub(super) fn format_dispatch_output(response: &LifecycleDispatchResponse) -> String {
    [
        format!(
            "{} {}",
            paint(success_style(), "[accepted]"),
            paint(title_style(), "Lifecycle worker")
        ),
        lifecycle_field("Worker", &response.worker_id),
        lifecycle_field("Mode", mode_label(response.mode)),
        lifecycle_field("Repo", &response.repo),
        lifecycle_field("Status", &response.status),
        lifecycle_field("Scope", "v1 prompt/requirements workflow"),
    ]
    .join("\n")
}

pub(super) fn format_list_output(response: &LifecycleListResponse) -> String {
    let rows = list_rows(response);
    let status_width = column_width(&rows, |row| &row.status);
    let worker_id_width = column_width(&rows, |row| &row.worker_id);

    rows.into_iter()
        .map(|row| {
            format!(
                "{:<status_width$}  {:<worker_id_width$}  {}",
                row.status, row.worker_id, row.repo
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn format_worker_output_header(worker_id: &str) -> String {
    format!(
        "{} {}",
        paint(success_style(), "[stream]"),
        paint(
            title_style(),
            format!("Lifecycle worker {}", redact_text(worker_id))
        )
    )
}

pub(super) fn format_worker_output_item(
    worker_id: &str,
    item: &LifecycleStreamItem,
) -> Vec<String> {
    let mut lines = Vec::new();

    match item {
        LifecycleStreamItem::HistoryTruncated {
            requested_from_sequence,
            first_available_sequence,
        } => lines.push(format!(
            "  history was truncated: requested sequence {requested_from_sequence}, first available sequence {first_available_sequence}"
        )),
        LifecycleStreamItem::Event(event) => append_worker_event(&mut lines, worker_id, event),
    }

    lines
}

pub(super) fn format_answer_output(response: &LifecycleAnswerResponse) -> String {
    let status = if response.accepted {
        "answer accepted and forwarded"
    } else {
        "answer was not accepted"
    };

    [
        format!(
            "{} {}",
            paint(success_style(), "[answered]"),
            paint(title_style(), "Lifecycle request")
        ),
        lifecycle_field("Worker", &response.worker_id),
        lifecycle_field("Request", &response.request_id),
        lifecycle_field("Status", status),
    ]
    .join("\n")
}

fn lifecycle_field(label: &str, value: &str) -> String {
    format!(
        "  {} {}",
        paint(label_style(), format!("{label}:")),
        paint(provider_style(), redact_text(value))
    )
}

#[derive(Debug, Eq, PartialEq)]
struct ListRow {
    status: String,
    worker_id: String,
    repo: String,
}

impl ListRow {
    fn new(status: &str, worker_id: &str, repo: &str) -> Self {
        Self {
            status: safe_cell(status),
            worker_id: safe_cell(worker_id),
            repo: safe_cell(repo),
        }
    }
}

fn list_rows(response: &LifecycleListResponse) -> Vec<ListRow> {
    std::iter::once(ListRow::new("status", "worker id", "git repo"))
        .chain(response.workers.iter().map(worker_row))
        .collect()
}

fn worker_row(worker: &LifecycleWorkerSummary) -> ListRow {
    ListRow::new(&worker.status, &worker.worker_id, &worker.repo)
}

fn column_width(rows: &[ListRow], cell: impl Fn(&ListRow) -> &str) -> usize {
    rows.iter()
        .map(|row| cell(row).chars().count())
        .max()
        .unwrap_or_default()
}

fn safe_cell(value: &str) -> String {
    safe_message(value)
}

fn mode_label(mode: LifecycleDispatchMode) -> &'static str {
    match mode {
        LifecycleDispatchMode::Feature => "feature",
        LifecycleDispatchMode::Debug => "debug",
    }
}

fn append_worker_event(lines: &mut Vec<String>, worker_id: &str, event: &LifecycleWorkerEvent) {
    lines.push(format!(
        "  {} {} {} - {}",
        event.sequence,
        redact_text(&event.name),
        redact_text(&event.status),
        redact_text(&event.message)
    ));

    if let Some(request_id) = &event.request_id {
        lines.push(lifecycle_field("Status", "waiting for input"));
        lines.push(lifecycle_field("Request", request_id));
        lines.push(format!(
            "  {} doric answer {} {} --text <answer>",
            paint(label_style(), "Answer:"),
            redact_text(worker_id),
            redact_text(request_id)
        ));
    }
}

fn paint(style: Style, value: impl AsRef<str>) -> String {
    terminal_style::paint(style, value)
}

fn title_style() -> Style {
    terminal_style::title_style()
}

fn label_style() -> Style {
    terminal_style::dim_style()
}

fn success_style() -> Style {
    terminal_style::success_style()
}

fn provider_style() -> Style {
    terminal_style::provider_style()
}
