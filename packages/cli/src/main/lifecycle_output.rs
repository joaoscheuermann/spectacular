use super::{
    lifecycle::{
        redact_text, LifecycleAnswerResponse, LifecycleDispatchMode, LifecycleDispatchResponse,
        LifecycleListResponse, LifecycleStreamItem, LifecycleWorkerEvent,
    },
    terminal_style,
};
use anstyle::Style;

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
    if response.workers.is_empty() {
        return format!(
            "{} {}",
            paint(missing_style(), "[empty]"),
            "No lifecycle workers are tracked"
        );
    }

    response
        .workers
        .iter()
        .map(|worker| {
            [
                format!(
                    "{} {}",
                    paint(success_style(), "[worker]"),
                    paint(title_style(), &worker.worker_id)
                ),
                lifecycle_field("Mode", mode_label(worker.mode)),
                lifecycle_field("Repo", &worker.repo),
                lifecycle_field("Status", &worker.status),
                lifecycle_field("Sequence", &worker.latest_sequence.to_string()),
                lifecycle_field("Activity", &worker.activity),
                lifecycle_optional_field("Reason", worker.terminal_reason.as_deref()),
                lifecycle_optional_field("Request", worker.pending_request_id.as_deref()),
            ]
            .into_iter()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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

fn lifecycle_optional_field(label: &str, value: Option<&str>) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| lifecycle_field(label, value))
        .unwrap_or_default()
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

fn missing_style() -> Style {
    terminal_style::warning_style()
}
