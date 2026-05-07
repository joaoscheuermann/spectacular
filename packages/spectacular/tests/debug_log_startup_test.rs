use spectacular_llms::DEBUG_LOG_FILE_NAME;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn spectacular_process_replaces_debug_log_on_start() {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_spectacular"));
    let log_path = executable
        .parent()
        .expect("binary should have a parent directory")
        .join(DEBUG_LOG_FILE_NAME);

    std::fs::write(&log_path, "stale content").unwrap();
    run_config_command(&executable, "first");
    assert_eq!(std::fs::read_to_string(&log_path).unwrap(), "");

    std::fs::write(&log_path, "second stale content").unwrap();
    run_config_command(&executable, "second");
    assert_eq!(std::fs::read_to_string(&log_path).unwrap(), "");
}

fn run_config_command(executable: &Path, name: &str) {
    let config_home = temp_config_home(name);
    let output = Command::new(executable)
        .arg("config")
        .env("APPDATA", &config_home)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", &config_home)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "config command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_config_home(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("spectacular-debug-startup-{name}-{suffix}"))
}
