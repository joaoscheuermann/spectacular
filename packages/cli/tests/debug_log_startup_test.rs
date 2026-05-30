use ::llms::DEBUG_LOG_FILE_NAME;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static DEBUG_LOG_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn doric_process_config_command_preserves_stale_debug_log_content() {
    let _guard = DEBUG_LOG_LOCK.lock().unwrap();
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_doric"));
    let log_path = executable
        .parent()
        .expect("binary should have a parent directory")
        .join(DEBUG_LOG_FILE_NAME);

    std::fs::write(&log_path, "stale content").unwrap();
    run_config_command(&executable, "first");
    assert_eq!(std::fs::read_to_string(&log_path).unwrap(), "stale content");

    std::fs::write(&log_path, "second stale content").unwrap();
    run_config_command(&executable, "second");
    assert_eq!(
        std::fs::read_to_string(&log_path).unwrap(),
        "second stale content"
    );
}

#[test]
fn doric_process_lifecycle_commands_preserve_stale_debug_log_content() {
    let _guard = DEBUG_LOG_LOCK.lock().unwrap();
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_doric"));
    let log_path = executable
        .parent()
        .expect("binary should have a parent directory")
        .join(DEBUG_LOG_FILE_NAME);

    std::fs::write(&log_path, "stale lifecycle content").unwrap();
    run_lifecycle_command(&executable, "lifecycle");
    assert_eq!(
        std::fs::read_to_string(&log_path).unwrap(),
        "stale lifecycle content"
    );
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

fn run_lifecycle_command(executable: &Path, name: &str) {
    let config_home = temp_config_home(name);
    let output = Command::new(executable)
        .arg("list")
        .env("APPDATA", &config_home)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("HOME", &config_home)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "lifecycle command should fail when daemon is unavailable: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("daemon unavailable"),
        "lifecycle command should report daemon unavailable: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_config_home(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    std::env::temp_dir().join(format!("doric-debug-startup-{name}-{suffix}"))
}
