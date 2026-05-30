use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::root::{
    prepare_worker_layout, resolve_worker_root_with_default, validate_worker_root, WorkerLayout,
};

/// Verifies that explicit worker-root overrides resolve to the canonical directory.
#[test]
fn resolve_worker_root_explicit_directory_returns_canonical_root() {
    let temp = TempRoot::new("explicit-directory");
    let explicit = temp.path().join("workers");
    let default = temp.path().join("default").join("workers");
    fs::create_dir_all(&explicit).unwrap();
    fs::create_dir_all(&default).unwrap();

    let root =
        resolve_worker_root_with_default(Some(explicit.clone()), || Ok(default.clone())).unwrap();

    assert_eq!(root, explicit.canonicalize().unwrap());
}

/// Verifies that absent worker-root uses the injected config-dir workers default.
#[test]
fn resolve_worker_root_without_override_uses_config_dir_workers() {
    let temp = TempRoot::new("default-directory");
    let config_workers = temp.path().join("config").join("workers");
    fs::create_dir_all(&config_workers).unwrap();

    let root = resolve_worker_root_with_default(None, || Ok(config_workers.clone())).unwrap();

    assert_eq!(root, config_workers.canonicalize().unwrap());
}

/// Verifies that missing roots fail as root configuration before execution.
#[test]
fn validate_worker_root_missing_directory_returns_root_configuration_error() {
    let temp = TempRoot::new("missing-directory");
    let missing = temp.path().join("missing-workers");

    let error = validate_worker_root(&missing).unwrap_err();

    assert!(error.is_root_configuration());
    assert_eq!(error.path(), Some(missing.as_path()));
    assert!(error.to_string().contains("worker root"));
}

/// Verifies that file paths are rejected as invalid worker roots.
#[test]
fn validate_worker_root_file_instead_of_directory_returns_root_configuration_error() {
    let temp = TempRoot::new("file-root");
    let file_root = temp.path().join("workers-file");
    fs::write(&file_root, "not a directory").unwrap();

    let error = validate_worker_root(&file_root).unwrap_err();

    assert!(error.is_root_configuration());
    assert_eq!(error.path(), Some(file_root.as_path()));
    assert!(error.to_string().contains("directory"));
}

/// Verifies that permission failures are reported as root configuration errors.
#[test]
fn resolve_worker_root_default_provider_error_returns_root_configuration_error() {
    let error = resolve_worker_root_with_default(None, || {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "cannot read config directory",
        )
        .into())
    })
    .unwrap_err();

    assert!(error.is_root_configuration());
    assert!(error.to_string().contains("worker root"));
}

/// Verifies that worker layout stays under the validated worker root.
#[test]
fn prepare_worker_layout_valid_root_creates_expected_child_directories() {
    let temp = TempRoot::new("worker-layout");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();

    let layout = prepare_worker_layout(&root, "worker-123".parse().unwrap()).unwrap();

    assert_layout(&layout, &root.join("worker-123"));
    assert!(layout.repo().is_dir());
    assert!(layout.state().is_dir());
    assert!(layout.artifacts().is_dir());
    assert!(layout.tool_output().is_dir());
}

fn assert_layout(layout: &WorkerLayout, worker_root: &Path) {
    assert_eq!(layout.worker_root(), worker_root);
    assert_eq!(layout.repo(), worker_root.join("repo"));
    assert_eq!(layout.state(), worker_root.join("state"));
    assert_eq!(layout.artifacts(), worker_root.join("artifacts"));
    assert_eq!(layout.tool_output(), worker_root.join("tool-output"));
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-daemon-root-{name}-{}", suffix()));
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
