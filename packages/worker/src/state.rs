use std::path::{Path, PathBuf};

use lifecycle::identity::RequestId;

pub use lifecycle::identity::WorkerId;

/// Worker directory layout used by the runtime after daemon `StartJob`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeLayout {
    worker_root: PathBuf,
    repo: PathBuf,
    state: PathBuf,
    artifacts: PathBuf,
    tool_output: PathBuf,
}

impl RuntimeLayout {
    pub fn new(
        worker_root: PathBuf,
        repo: PathBuf,
        state: PathBuf,
        artifacts: PathBuf,
        tool_output: PathBuf,
    ) -> Self {
        Self {
            worker_root,
            repo,
            state,
            artifacts,
            tool_output,
        }
    }

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

/// Repo preparation request built from daemon-controlled job data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRepoRequest {
    worker_id: WorkerId,
    repo_url: String,
    layout: RuntimeLayout,
}

impl RuntimeRepoRequest {
    pub fn new(worker_id: WorkerId, repo_url: impl Into<String>, layout: RuntimeLayout) -> Self {
        Self {
            worker_id,
            repo_url: repo_url.into(),
            layout,
        }
    }

    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub fn repo_url(&self) -> &str {
        &self.repo_url
    }

    pub fn layout(&self) -> &RuntimeLayout {
        &self.layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRuntimeRepo {
    layout: RuntimeLayout,
}

impl PreparedRuntimeRepo {
    pub fn new(layout: RuntimeLayout) -> Self {
        Self { layout }
    }

    pub fn layout(&self) -> &RuntimeLayout {
        &self.layout
    }
}

/// Prompt/requirements job built only after repo preparation succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePromptJob {
    worker_id: WorkerId,
    layout: RuntimeLayout,
    prompt: String,
}

impl RuntimePromptJob {
    pub fn new(worker_id: WorkerId, layout: RuntimeLayout, prompt: impl Into<String>) -> Self {
        Self {
            worker_id,
            layout,
            prompt: prompt.into(),
        }
    }

    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub fn layout(&self) -> &RuntimeLayout {
        &self.layout
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAnswer {
    request_id: RequestId,
    text: String,
}

impl RuntimeAnswer {
    pub fn new(request_id: RequestId, text: impl Into<String>) -> Self {
        Self {
            request_id,
            text: text.into(),
        }
    }

    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}
