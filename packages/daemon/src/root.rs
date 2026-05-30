use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle::identity::WorkerId;

use crate::error::{DaemonError, DaemonResult};

/// Resolves and validates the daemon worker root, defaulting to config dir workers.
pub fn resolve_worker_root(explicit: Option<PathBuf>) -> DaemonResult<PathBuf> {
    resolve_worker_root_with_default(explicit, || Ok(config::config_dir()?.join("workers")))
}

/// Resolves and validates the daemon worker root with an injectable default path.
pub fn resolve_worker_root_with_default(
    explicit: Option<PathBuf>,
    default: impl FnOnce() -> DaemonResult<PathBuf>,
) -> DaemonResult<PathBuf> {
    let root = match explicit {
        Some(root) => root,
        None => default().map_err(|error| {
            DaemonError::root_configuration(
                error.path().map(Path::to_path_buf),
                "could not resolve default worker root",
                Some(Box::new(error)),
            )
        })?,
    };

    validate_worker_root(&root)
}

/// Validates that a worker root exists, is a directory, and accepts child directories.
pub fn validate_worker_root(path: &Path) -> DaemonResult<PathBuf> {
    let metadata = fs::metadata(path).map_err(|source| {
        DaemonError::root_configuration(
            Some(path.to_path_buf()),
            "worker root must exist and be readable",
            Some(Box::new(source)),
        )
    })?;

    if !metadata.is_dir() {
        return Err(DaemonError::root_configuration(
            Some(path.to_path_buf()),
            "worker root must be a directory",
            None,
        ));
    }

    let canonical = path.canonicalize().map_err(|source| {
        DaemonError::root_configuration(
            Some(path.to_path_buf()),
            "worker root could not be canonicalized",
            Some(Box::new(source)),
        )
    })?;

    ensure_worker_root_writable(&canonical)?;

    Ok(canonical)
}

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

/// Creates the per-worker daemon layout under `<root>/<worker-id>/`.
pub fn prepare_worker_layout(root: &Path, worker_id: WorkerId) -> DaemonResult<WorkerLayout> {
    validate_worker_root(root)?;

    let worker_root = root.join(worker_id.as_str());
    let layout = WorkerLayout {
        repo: worker_root.join("repo"),
        state: worker_root.join("state"),
        artifacts: worker_root.join("artifacts"),
        tool_output: worker_root.join("tool-output"),
        worker_root,
    };

    for path in [
        &layout.worker_root,
        &layout.repo,
        &layout.state,
        &layout.artifacts,
        &layout.tool_output,
    ] {
        fs::create_dir_all(path).map_err(|source| {
            DaemonError::root_configuration(
                Some(path.clone()),
                "could not create worker layout directory",
                Some(Box::new(source)),
            )
        })?;
    }

    Ok(layout)
}

fn ensure_worker_root_writable(path: &Path) -> DaemonResult<()> {
    let probe = path.join(format!(
        ".doric-worker-root-write-check-{}-{}",
        std::process::id(),
        timestamp()
    ));

    fs::create_dir(&probe).map_err(|source| {
        DaemonError::root_configuration(
            Some(path.to_path_buf()),
            "worker root must be writable",
            Some(Box::new(source)),
        )
    })?;

    fs::remove_dir(&probe).map_err(|source| {
        DaemonError::root_configuration(
            Some(probe),
            "could not remove worker root writability probe",
            Some(Box::new(source)),
        )
    })
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
