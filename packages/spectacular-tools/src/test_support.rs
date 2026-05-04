use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) async fn temp_workspace(test_name: &str) -> PathBuf {
    let unique_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "spectacular_tools_{test_name}_{}_{}",
        std::process::id(),
        unique_id
    ));
    tokio::fs::create_dir_all(&path).await.unwrap();
    path
}

pub(crate) async fn write_file(root: &Path, relative_path: &str, content: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    tokio::fs::write(path, content).await.unwrap();
}

pub(crate) async fn remove_workspace(path: PathBuf) {
    let _ = tokio::fs::remove_dir_all(path).await;
}
