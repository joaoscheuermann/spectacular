use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::error::{WorkerError, WorkerResult};
use crate::state::WorkerId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerLayout {
    worker_root: PathBuf,
    repo: PathBuf,
    state: PathBuf,
    artifacts: PathBuf,
    tool_output: PathBuf,
}

impl WorkerLayout {
    pub fn worker_root(&self) -> &Path {
        &self.worker_root
    }

    pub fn repo(&self) -> PathBuf {
        self.repo.clone()
    }

    pub fn state(&self) -> PathBuf {
        self.state.clone()
    }

    pub fn artifacts(&self) -> PathBuf {
        self.artifacts.clone()
    }

    pub fn tool_output(&self) -> PathBuf {
        self.tool_output.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareRepoRequest {
    root: PathBuf,
    worker_id: WorkerId,
    repo_url: String,
    timeout: Option<Duration>,
}

impl PrepareRepoRequest {
    pub fn new(root: PathBuf, worker_id: WorkerId, repo_url: String) -> Self {
        Self {
            root,
            worker_id,
            repo_url,
            timeout: None,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRepo {
    layout: WorkerLayout,
}

impl PreparedRepo {
    pub fn layout(&self) -> &WorkerLayout {
        &self.layout
    }

    pub fn repo_dir(&self) -> PathBuf {
        self.layout.repo()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitCommand {
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
    timeout: Option<Duration>,
}

impl GitCommand {
    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn cwd(&self) -> Option<&Path> {
        Some(&self.cwd)
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitOutput {
    status: i32,
    stderr: String,
}

impl GitOutput {
    pub fn success() -> Self {
        Self {
            status: 0,
            stderr: String::new(),
        }
    }

    pub fn failure(status: i32, stderr: &str) -> Self {
        Self {
            status,
            stderr: stderr.to_owned(),
        }
    }

    pub fn status(&self) -> i32 {
        self.status
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }
}

pub trait GitCommandRunner {
    fn run(&self, command: GitCommand) -> WorkerResult<GitOutput>;
}

pub fn prepare_worker_layout(
    root: impl AsRef<Path>,
    worker_id: WorkerId,
) -> WorkerResult<WorkerLayout> {
    let root = root.as_ref();

    if !root.is_dir() {
        return Err(WorkerError::invalid_worker_root(root.to_path_buf()));
    }

    validate_worker_path_component(root, &worker_id)?;

    let worker_root = root.join(worker_id.as_str());
    let layout = WorkerLayout {
        repo: worker_root.join("repo"),
        state: worker_root.join("state"),
        artifacts: worker_root.join("artifacts"),
        tool_output: worker_root.join("tool-output"),
        worker_root,
    };

    for path in [
        layout.worker_root(),
        layout.repo.as_path(),
        layout.state.as_path(),
        layout.artifacts.as_path(),
        layout.tool_output.as_path(),
    ] {
        fs::create_dir_all(path)
            .map_err(|_| WorkerError::invalid_worker_root(path.to_path_buf()))?;
    }

    Ok(layout)
}

pub fn prepare_worker_repo(
    request: PrepareRepoRequest,
    runner: &dyn GitCommandRunner,
) -> WorkerResult<PreparedRepo> {
    let layout = prepare_worker_layout(&request.root, request.worker_id)?;

    if repo_dir_is_non_empty(&layout.repo)? {
        return Err(WorkerError::clone_target_conflict(layout.repo()));
    }

    let repo_dir = layout.repo().to_string_lossy().into_owned();
    let command = GitCommand {
        program: "git".to_owned(),
        args: vec![
            "clone".to_owned(),
            "--".to_owned(),
            request.repo_url.clone(),
            repo_dir,
        ],
        cwd: layout.worker_root().to_path_buf(),
        timeout: request.timeout,
    };
    let output = runner.run(command)?;

    if output.status() != 0 {
        return Err(WorkerError::git_failed(
            output.status(),
            output.stderr(),
            &request.repo_url,
        ));
    }

    Ok(PreparedRepo { layout })
}

fn validate_worker_path_component(root: &Path, worker_id: &WorkerId) -> WorkerResult<()> {
    let value = worker_id.as_str();
    let path = Path::new(value);
    let is_single_normal_component = matches!(
        path.components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(component)] if *component == value
    );

    if is_single_normal_component && !value.contains('/') && !value.contains('\\') {
        Ok(())
    } else {
        Err(WorkerError::invalid_worker_root(root.join(value)))
    }
}

fn repo_dir_is_non_empty(path: &Path) -> WorkerResult<bool> {
    let mut entries =
        fs::read_dir(path).map_err(|_| WorkerError::invalid_worker_root(path.to_path_buf()))?;
    Ok(entries
        .next()
        .transpose()
        .map_err(|_| WorkerError::invalid_worker_root(path.to_path_buf()))?
        .is_some())
}
