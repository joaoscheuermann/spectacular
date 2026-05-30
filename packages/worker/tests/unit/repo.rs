use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use worker::error::{WorkerError, WorkerFailureReason, WorkerResult};
use worker::repo::{
    prepare_worker_layout, prepare_worker_repo, GitCommand, GitCommandRunner, GitOutput,
    PrepareRepoRequest, PreparedRepo, WorkerLayout,
};
use worker::state::WorkerId;

#[test]
fn prepare_worker_layout_valid_root_creates_repo_state_artifacts_and_tool_output_directories() {
    let temp = TempRoot::new("layout-creates-directories");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();

    let layout = prepare_worker_layout(&root, worker_id("worker-layout")).unwrap();

    assert_layout(&layout, &root.join("worker-layout"));
    assert!(layout.repo().is_dir());
    assert!(layout.state().is_dir());
    assert!(layout.artifacts().is_dir());
    assert!(layout.tool_output().is_dir());
}

#[test]
fn prepare_worker_layout_valid_root_returns_stable_child_paths() {
    let temp = TempRoot::new("layout-paths");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();

    let layout = prepare_worker_layout(&root, worker_id("worker-paths")).unwrap();

    assert_layout(&layout, &root.join("worker-paths"));
}

#[test]
fn prepare_worker_layout_missing_root_returns_invalid_worker_root() {
    let temp = TempRoot::new("missing-root");
    let missing = temp.path().join("missing-workers");

    let error = prepare_worker_layout(&missing, worker_id("worker-missing-root")).unwrap_err();

    assert!(error.is_invalid_worker_root());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::RootConfiguration
    );
    assert!(!missing.join("worker-missing-root").exists());
}

#[test]
fn prepare_worker_layout_file_root_returns_invalid_worker_root() {
    let temp = TempRoot::new("file-root");
    let file_root = temp.path().join("workers-file");
    fs::write(&file_root, "not a directory").unwrap();

    let error = prepare_worker_layout(&file_root, worker_id("worker-file-root")).unwrap_err();

    assert!(error.is_invalid_worker_root());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::RootConfiguration
    );
    assert!(error.to_string().contains("worker root"));
}

#[test]
fn prepare_worker_layout_path_like_worker_id_returns_invalid_worker_root() {
    let temp = TempRoot::new("path-like-worker-id");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();

    for id in ["../escaped-worker", r"..\escaped-worker"] {
        let escaped = temp.path().join("escaped-worker");
        let error = prepare_worker_layout(&root, worker_id(id)).unwrap_err();

        assert!(error.is_invalid_worker_root());
        assert_eq!(
            error.lifecycle_failure_reason(),
            WorkerFailureReason::RootConfiguration
        );
        assert!(!escaped.exists());
    }
}

#[test]
fn prepare_worker_repo_existing_nonempty_repo_dir_returns_clone_target_conflict() {
    let temp = TempRoot::new("repo-conflict");
    let root = temp.path().join("workers");
    fs::create_dir_all(root.join("worker-conflict").join("repo")).unwrap();
    fs::write(
        root.join("worker-conflict")
            .join("repo")
            .join("sentinel.txt"),
        "existing checkout",
    )
    .unwrap();
    let runner = FakeGitRunner::success();

    let error = prepare_worker_repo(
        request(&root, "worker-conflict", "https://github.com/org/repo.git"),
        &runner,
    )
    .unwrap_err();

    assert!(error.is_clone_target_conflict());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::RepoCloneTargetConflict
    );
    assert!(runner.commands().is_empty());
}

#[test]
fn prepare_worker_repo_existing_empty_repo_dir_uses_injected_runner_and_completes_layout() {
    let temp = TempRoot::new("empty-repo-dir");
    let root = temp.path().join("workers");
    fs::create_dir_all(root.join("worker-empty").join("repo")).unwrap();
    let runner = FakeGitRunner::success();

    let prepared = prepare_worker_repo(
        request(&root, "worker-empty", "https://github.com/org/repo.git"),
        &runner,
    )
    .unwrap();

    assert_prepared_repo(&prepared, &root.join("worker-empty"));
    assert_eq!(runner.commands().len(), 1);
}

#[test]
fn prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command() {
    let temp = TempRoot::new("clone-command");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let repo_url = "https://user:secret@github.com/org/repo.git";
    let runner = FakeGitRunner::success();

    prepare_worker_repo(request(&root, "worker-command", repo_url), &runner).unwrap();

    let commands = runner.commands();
    assert_eq!(commands.len(), 1);
    let command = &commands[0];
    assert_eq!(command.program(), "git");
    assert_eq!(
        command.args(),
        &[
            "clone",
            "--",
            repo_url,
            root.join("worker-command").join("repo").to_str().unwrap()
        ]
    );
    assert_eq!(command.cwd(), Some(root.join("worker-command").as_path()));
    assert_eq!(command.timeout(), Some(Duration::from_secs(120)));
}

#[test]
fn prepare_worker_repo_fake_runner_success_never_uses_network() {
    let temp = TempRoot::new("fake-runner-success");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let runner = FakeGitRunner::success();

    let prepared = prepare_worker_repo(
        request(
            &root,
            "worker-no-network",
            "https://github.com/org/repo.git",
        ),
        &runner,
    )
    .unwrap();

    assert_prepared_repo(&prepared, &root.join("worker-no-network"));
    assert_eq!(runner.commands().len(), 1);
}

#[test]
fn prepare_worker_repo_missing_git_returns_git_missing_failure() {
    let temp = TempRoot::new("missing-git");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let runner = FakeGitRunner::missing_git();

    let error = prepare_worker_repo(
        request(
            &root,
            "worker-missing-git",
            "https://github.com/org/repo.git",
        ),
        &runner,
    )
    .unwrap_err();

    assert!(error.is_git_missing());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::GitUnavailable
    );
    assert!(!error
        .to_string()
        .contains("https://github.com/org/repo.git"));
}

#[test]
fn prepare_worker_repo_git_nonzero_returns_git_failed_with_redacted_stderr() {
    let temp = TempRoot::new("git-nonzero");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let repo_url = "https://user:pass@github.com/org/private.git";
    let runner = FakeGitRunner::failure(
        128,
        "fatal: token sk-test_secret_1234567890abcdef for https://user:pass@github.com/org/private.git rejected",
    );

    let error =
        prepare_worker_repo(request(&root, "worker-nonzero", repo_url), &runner).unwrap_err();
    let message = error.to_string();

    assert!(error.is_git_failure());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::GitCloneFailed
    );
    assert!(message.contains("[REDACTED]"));
    assert!(message.contains("https://github.com/org/private.git"));
    assert!(!message.contains("sk-test_secret_1234567890abcdef"));
    assert!(!message.contains("user:pass"));
}

#[test]
fn prepare_worker_repo_git_timeout_returns_git_timeout_failure() {
    let temp = TempRoot::new("git-timeout");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let runner = FakeGitRunner::timeout(Duration::from_secs(120));

    let error = prepare_worker_repo(
        request(&root, "worker-timeout", "https://github.com/org/repo.git"),
        &runner,
    )
    .unwrap_err();

    assert!(error.is_git_timeout());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::GitCloneTimedOut
    );
}

#[test]
fn prepare_worker_repo_git_cancellation_returns_git_cancelled_failure() {
    let temp = TempRoot::new("git-cancelled");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let runner = FakeGitRunner::cancelled();

    let error = prepare_worker_repo(
        request(&root, "worker-cancelled", "https://github.com/org/repo.git"),
        &runner,
    )
    .unwrap_err();

    assert!(error.is_git_cancelled());
    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::GitCloneCancelled
    );
}

#[test]
fn prepare_worker_repo_redacts_credential_bearing_repo_url_in_failure() {
    let temp = TempRoot::new("repo-url-redaction");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let runner = FakeGitRunner::failure(128, "authentication failed");

    let error = prepare_worker_repo(
        request(
            &root,
            "worker-url-redaction",
            "https://user:pass@github.com/org/repo.git",
        ),
        &runner,
    )
    .unwrap_err();
    let message = error.to_string();

    assert!(message.contains("https://github.com/org/repo.git"));
    assert!(!message.contains("user"));
    assert!(!message.contains("pass"));
    assert!(!message.contains("@github"));
}

#[test]
fn prepare_worker_repo_redacts_clone_stderr_before_mapping_failure_event() {
    let temp = TempRoot::new("stderr-redaction");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let runner = FakeGitRunner::failure(
        128,
        "remote: API key sk-test_secret_1234567890abcdef denied",
    );

    let error = prepare_worker_repo(
        request(
            &root,
            "worker-stderr-redaction",
            "https://github.com/org/repo.git",
        ),
        &runner,
    )
    .unwrap_err();
    let reason = error.lifecycle_failure_message();

    assert_eq!(
        error.lifecycle_failure_reason(),
        WorkerFailureReason::GitCloneFailed
    );
    assert!(reason.contains("[REDACTED]"));
    assert!(!reason.contains("sk-test_secret_1234567890abcdef"));
}

#[test]
fn worker_error_failure_variants_map_to_lifecycle_failure_reasons() {
    let cases = [
        (
            WorkerError::invalid_worker_root(PathBuf::from("missing")),
            WorkerFailureReason::RootConfiguration,
        ),
        (
            WorkerError::clone_target_conflict(PathBuf::from("repo")),
            WorkerFailureReason::RepoCloneTargetConflict,
        ),
        (
            WorkerError::git_missing("git"),
            WorkerFailureReason::GitUnavailable,
        ),
        (
            WorkerError::git_failed(
                128,
                "fatal: key sk-test_secret_1234567890abcdef failed",
                "https://user:pass@github.com/org/repo.git",
            ),
            WorkerFailureReason::GitCloneFailed,
        ),
        (
            WorkerError::git_timeout(Duration::from_secs(120)),
            WorkerFailureReason::GitCloneTimedOut,
        ),
        (
            WorkerError::git_cancelled(),
            WorkerFailureReason::GitCloneCancelled,
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.lifecycle_failure_reason(), expected);
        assert!(!error.lifecycle_failure_message().contains("sk-test_secret"));
        assert!(!error.lifecycle_failure_message().contains("user:pass"));
    }
}

fn request(root: &Path, id: &str, repo_url: &str) -> PrepareRepoRequest {
    PrepareRepoRequest::new(root.to_path_buf(), worker_id(id), repo_url.to_owned())
        .with_timeout(Duration::from_secs(120))
}

fn worker_id(value: &str) -> WorkerId {
    value.parse().unwrap()
}

fn assert_layout(layout: &WorkerLayout, worker_root: &Path) {
    assert_eq!(layout.worker_root(), worker_root);
    assert_eq!(layout.repo(), worker_root.join("repo"));
    assert_eq!(layout.state(), worker_root.join("state"));
    assert_eq!(layout.artifacts(), worker_root.join("artifacts"));
    assert_eq!(layout.tool_output(), worker_root.join("tool-output"));
}

fn assert_prepared_repo(prepared: &PreparedRepo, worker_root: &Path) {
    assert_layout(prepared.layout(), worker_root);
    assert_eq!(prepared.repo_dir(), worker_root.join("repo"));
}

#[derive(Clone)]
enum GitOutcome {
    Success,
    Failure { status: i32, stderr: String },
    MissingGit,
    Timeout(Duration),
    Cancelled,
}

struct FakeGitRunner {
    outcome: GitOutcome,
    commands: Mutex<Vec<GitCommand>>,
}

impl FakeGitRunner {
    fn success() -> Self {
        Self::new(GitOutcome::Success)
    }

    fn failure(status: i32, stderr: &str) -> Self {
        Self::new(GitOutcome::Failure {
            status,
            stderr: stderr.to_owned(),
        })
    }

    fn missing_git() -> Self {
        Self::new(GitOutcome::MissingGit)
    }

    fn timeout(duration: Duration) -> Self {
        Self::new(GitOutcome::Timeout(duration))
    }

    fn cancelled() -> Self {
        Self::new(GitOutcome::Cancelled)
    }

    fn new(outcome: GitOutcome) -> Self {
        Self {
            outcome,
            commands: Mutex::new(Vec::new()),
        }
    }

    fn commands(&self) -> Vec<GitCommand> {
        self.commands.lock().unwrap().clone()
    }
}

impl GitCommandRunner for FakeGitRunner {
    fn run(&self, command: GitCommand) -> WorkerResult<GitOutput> {
        self.commands.lock().unwrap().push(command);

        match &self.outcome {
            GitOutcome::Success => Ok(GitOutput::success()),
            GitOutcome::Failure { status, stderr } => Ok(GitOutput::failure(*status, stderr)),
            GitOutcome::MissingGit => Err(WorkerError::git_missing("git")),
            GitOutcome::Timeout(duration) => Err(WorkerError::git_timeout(*duration)),
            GitOutcome::Cancelled => Err(WorkerError::git_cancelled()),
        }
    }
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-worker-repo-{name}-{}", suffix()));
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
