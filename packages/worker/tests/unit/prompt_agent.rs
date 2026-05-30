use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent::Store;
use config::ReasoningLevel;
use lifecycle::identity::RequestId;
use lifecycle::status::WorkerStatus;
use llms::OPENROUTER_PROVIDER_ID;
use worker::agents::prompt::{
    prompt_agent_for_worker, write_prompt_artifact, PromptAgentEvent, PromptAgentOrchestrator,
    PromptAgentReport, PromptAgentRunner, PromptJob,
};
use worker::error::WorkerResult;
use worker::provider::{
    agent_config_for_runtime, provider_for_runtime, WorkerProviderAuth, WorkerProviderDeps,
    WorkerRuntimeSelection,
};
use worker::repo::{prepare_worker_layout, WorkerLayout};
use worker::state::WorkerId;

#[test]
fn run_prompt_agent_fake_runner_started_emits_prompt_agent_started() {
    let (_temp, layout) = prepared_layout("prompt-agent-started");
    let worker_id = worker_id("worker-prompt-started");
    let runner = FakePromptRunner::new([PromptAgentEvent::started(
        "Prompt requirements agent started",
    )]);

    let report = run_prompt_job(&runner, worker_id, layout);

    let event = &report.events()[0];
    assert_eq!(event.name(), "prompt_agent_started");
    assert_eq!(event.status(), WorkerStatus::Running);
    assert_prompt_only(event.message());
}

#[test]
fn run_prompt_agent_fake_runner_artifact_written_emits_prompt_artifact_written() {
    let (_temp, layout) = prepared_layout("prompt-agent-artifact-event");
    let worker_id = worker_id("worker-prompt-artifact-event");
    let runner = FakePromptRunner::new([PromptAgentEvent::artifact_written(
        "artifacts/PROMPT.md",
        "Prompt artifact written to artifacts/PROMPT.md",
    )]);

    let report = run_prompt_job(&runner, worker_id, layout);

    let event = &report.events()[0];
    assert_eq!(event.name(), "prompt_artifact_written");
    assert_eq!(event.status(), WorkerStatus::Running);
    assert!(event.message().contains("artifacts/PROMPT.md"));
    assert_prompt_only(event.message());
}

#[test]
fn run_prompt_agent_fake_runner_input_requested_includes_request_text_and_correlation_handle() {
    let (_temp, layout) = prepared_layout("prompt-agent-input");
    let worker_id = worker_id("worker-prompt-input");
    let request_id = request_id("request-repository-constraints");
    let request_text = "Which repository constraints should the prompt include?";
    let runner = FakePromptRunner::new([PromptAgentEvent::input_requested(
        request_id.clone(),
        request_text,
    )]);

    let report = run_prompt_job(&runner, worker_id, layout);

    let event = &report.events()[0];
    assert_eq!(event.name(), "waiting_for_input");
    assert_eq!(event.status(), WorkerStatus::WaitingForInput);
    assert_eq!(event.request_id(), Some(&request_id));
    assert!(event.message().contains(request_text));
}

#[test]
fn run_prompt_agent_fake_runner_answer_consumed_emits_answer_consumed() {
    let (_temp, layout) = prepared_layout("prompt-agent-answer");
    let worker_id = worker_id("worker-prompt-answer");
    let request_id = request_id("request-answer-consumed");
    let runner = FakePromptRunner::new([PromptAgentEvent::answer_consumed(
        request_id.clone(),
        "Prompt answer consumed",
    )]);
    let job = PromptJob::new(
        worker_id,
        layout,
        "Gather product and architecture requirements",
    )
    .with_answer(request_id.clone(), "Include repo-local coding conventions.");

    let report = PromptAgentOrchestrator::new(&runner).run(job).unwrap();

    let event = &report.events()[0];
    assert_eq!(event.name(), "prompt_answer_consumed");
    assert_eq!(event.status(), WorkerStatus::Running);
    assert_eq!(event.request_id(), Some(&request_id));
    assert_prompt_only(event.message());
}

#[test]
fn run_prompt_agent_fake_runner_completed_emits_prompt_agent_completed() {
    let (_temp, layout) = prepared_layout("prompt-agent-completed");
    let worker_id = worker_id("worker-prompt-completed");
    let runner = FakePromptRunner::new([PromptAgentEvent::completed(
        "Prompt requirements artifact completed",
    )]);

    let report = run_prompt_job(&runner, worker_id, layout);

    let event = &report.events()[0];
    assert_eq!(event.name(), "prompt_agent_completed");
    assert_eq!(event.status(), WorkerStatus::Succeeded);
    assert_prompt_only(event.message());
    assert_prompt_only(report.output());
}

#[test]
fn run_prompt_agent_fake_runner_failed_emits_prompt_agent_failed_with_redacted_message() {
    let (_temp, layout) = prepared_layout("prompt-agent-failed");
    let worker_id = worker_id("worker-prompt-failed");
    let runner = FakePromptRunner::new([PromptAgentEvent::failed(
        "provider returned sk-test_secret_1234567890abcdef",
    )]);

    let report = run_prompt_job(&runner, worker_id, layout);

    let event = &report.events()[0];
    assert_eq!(event.name(), "prompt_agent_failed");
    assert_eq!(event.status(), WorkerStatus::Failed);
    assert!(event.message().contains("[REDACTED]"));
    assert!(!event.message().contains("sk-test_secret"));
}

#[test]
fn write_prompt_artifact_prepared_layout_writes_prompt_md_under_artifacts() {
    let (_temp, layout) = prepared_layout("prompt-artifact-location");
    let content = "# Product requirements\n\nCapture prompt-only requirements.\n";

    let path = write_prompt_artifact(&layout, content).unwrap();

    assert_eq!(path, layout.artifacts().join("PROMPT.md"));
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
    assert!(!layout.repo().join("PROMPT.md").exists());
    assert!(!layout.state().join("PROMPT.md").exists());
    assert!(!layout.tool_output().join("PROMPT.md").exists());
}

#[test]
fn write_prompt_artifact_existing_prompt_md_replaces_prompt_artifact_deterministically() {
    let (_temp, layout) = prepared_layout("prompt-artifact-replace");
    fs::write(layout.artifacts().join("PROMPT.md"), "stale prompt").unwrap();
    let content = "# Product requirements\n\nFresh prompt requirements.\n";

    write_prompt_artifact(&layout, content).unwrap();

    assert_eq!(
        fs::read_to_string(layout.artifacts().join("PROMPT.md")).unwrap(),
        content
    );
}

#[test]
fn prompt_agent_lifecycle_names_do_not_claim_later_doric_phases() {
    let (_temp, layout) = prepared_layout("prompt-agent-name-scope");
    let worker_id = worker_id("worker-prompt-name-scope");
    let runner = FakePromptRunner::new([
        PromptAgentEvent::started("Prompt requirements agent started"),
        PromptAgentEvent::artifact_written(
            "artifacts/PROMPT.md",
            "Prompt artifact written to artifacts/PROMPT.md",
        ),
        PromptAgentEvent::input_requested(
            request_id("request-scope"),
            "Which repository constraints should the prompt include?",
        ),
        PromptAgentEvent::answer_consumed(request_id("request-scope"), "Prompt answer consumed"),
        PromptAgentEvent::completed("Prompt requirements artifact completed"),
    ]);

    let report = run_prompt_job(&runner, worker_id, layout);

    for event in report.events() {
        assert_prompt_only(event.name());
    }
}

#[test]
fn prompt_agent_messages_do_not_claim_later_doric_phases_completed() {
    let (_temp, layout) = prepared_layout("prompt-agent-message-scope");
    let worker_id = worker_id("worker-prompt-message-scope");
    let runner = FakePromptRunner::new([
        PromptAgentEvent::started("Prompt requirements agent started"),
        PromptAgentEvent::completed("Prompt requirements artifact completed"),
    ]);

    let report = run_prompt_job(&runner, worker_id, layout);

    for event in report.events() {
        assert_prompt_only(event.message());
    }
    assert_prompt_only(report.output());
}

#[test]
fn prompt_agent_for_worker_provider_runtime_layout_builds_agent_with_config_and_tools_offline() {
    let (_temp, layout) = prepared_layout("prompt-production-agent");
    let runtime = runtime_selection(
        OPENROUTER_PROVIDER_ID,
        "openrouter-main",
        "openai/gpt-5.1-codex",
        ReasoningLevel::High,
        WorkerProviderAuth::ApiKey {
            api_key: "sk-test_openrouter".to_owned(),
        },
    )
    .with_context_window_tokens(Some(200_000));
    let provider = provider_for_runtime(&runtime, WorkerProviderDeps::offline()).unwrap();

    let prompt_agent = prompt_agent_for_worker(
        provider,
        &runtime,
        &layout,
        Store::default(),
        "system prompt",
    )
    .unwrap();

    assert_eq!(
        prompt_agent.config(),
        &agent_config_for_runtime(&runtime, "system prompt")
    );
    assert_eq!(
        prompt_agent
            .agent()
            .tool_manifests()
            .into_iter()
            .map(|manifest| manifest.name)
            .collect::<Vec<_>>(),
        vec!["edit", "find", "grep", "terminal", "tree", "web", "write"]
    );
    assert_worker_manifest_does_not_depend_on_cli();
}

fn run_prompt_job(
    runner: &FakePromptRunner,
    worker_id: WorkerId,
    layout: WorkerLayout,
) -> PromptAgentReport {
    let job = PromptJob::new(
        worker_id,
        layout,
        "Gather product and architecture requirements",
    );

    PromptAgentOrchestrator::new(runner).run(job).unwrap()
}

fn prepared_layout(name: &str) -> (TempRoot, WorkerLayout) {
    let temp = TempRoot::new(name);
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let layout = prepare_worker_layout(&root, worker_id(name)).unwrap();
    (temp, layout)
}

fn worker_id(value: &str) -> WorkerId {
    value.parse().unwrap()
}

fn request_id(value: &str) -> RequestId {
    value.parse().unwrap()
}

fn runtime_selection(
    provider_type: &str,
    provider_name: &str,
    model: &str,
    reasoning: ReasoningLevel,
    provider_auth: WorkerProviderAuth,
) -> WorkerRuntimeSelection {
    WorkerRuntimeSelection {
        provider_type: provider_type.to_owned(),
        provider_name: provider_name.to_owned(),
        model_key: format!("{provider_name}/{model}"),
        model: model.to_owned(),
        reasoning,
        provider_auth,
        context_window_tokens: None,
    }
}

fn assert_prompt_only(value: &str) {
    for forbidden in [
        "prd",
        "tdd",
        "technical_design",
        "technical design",
        "decomposition",
        "implementation",
        "tests",
        "validation",
        "handover",
    ] {
        assert!(
            !value.to_ascii_lowercase().contains(forbidden),
            "`{value}` should not claim later Doric phase `{forbidden}`"
        );
    }
}

fn assert_worker_manifest_does_not_depend_on_cli() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).unwrap();

    assert!(
        !manifest
            .lines()
            .any(|line| line.trim().starts_with("cli =")),
        "worker prompt-agent composition must not depend on the cli package"
    );
}

trait RuntimeSelectionExt {
    fn with_context_window_tokens(self, context_window_tokens: Option<usize>) -> Self;
}

impl RuntimeSelectionExt for WorkerRuntimeSelection {
    fn with_context_window_tokens(mut self, context_window_tokens: Option<usize>) -> Self {
        self.context_window_tokens = context_window_tokens;
        self
    }
}

struct FakePromptRunner {
    events: Vec<PromptAgentEvent>,
}

impl FakePromptRunner {
    fn new(events: impl IntoIterator<Item = PromptAgentEvent>) -> Self {
        Self {
            events: events.into_iter().collect(),
        }
    }
}

impl PromptAgentRunner for FakePromptRunner {
    fn run(&self, _job: &PromptJob) -> WorkerResult<PromptAgentReport> {
        Ok(PromptAgentReport::new(
            self.events.clone(),
            "# Product requirements\n\n# Architecture requirements\n",
        ))
    }
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-worker-prompt-{name}-{}", suffix()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
