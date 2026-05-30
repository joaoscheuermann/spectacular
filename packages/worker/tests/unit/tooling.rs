use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent::Cancellation;
use llms::ProviderToolCall;
use worker::repo::{prepare_worker_layout, WorkerLayout};
use worker::state::WorkerId;

#[test]
fn worker_tool_storage_prepared_layout_registers_shared_built_ins() {
    let temp = TempRoot::new("tooling-registers-built-ins");
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let layout = prepare_worker_layout(&root, worker_id("worker-tools")).unwrap();

    let storage = worker::tooling::worker_tool_storage(&layout).unwrap();

    assert_eq!(
        storage
            .manifests()
            .into_iter()
            .map(|manifest| manifest.name)
            .collect::<Vec<_>>(),
        vec!["edit", "find", "grep", "terminal", "tree", "web", "write"]
    );
    assert_worker_manifest_depends_on_shared_tools();
}

#[tokio::test]
async fn worker_tool_storage_relative_write_defaults_to_prepared_repo_root() {
    let (temp, layout) = prepared_layout("tooling-write-default");
    let storage = worker::tooling::worker_tool_storage(&layout).unwrap();

    let output = execute_tool(
        &storage,
        "write",
        r#"{"path":"notes/worker.txt","content":"worker-root"}"#,
    )
    .await;

    assert!(
        output.contains(r#""success":true"#),
        "write output should report success: {output}"
    );
    assert_eq!(
        fs::read_to_string(layout.repo().join("notes").join("worker.txt")).unwrap(),
        "worker-root"
    );
    assert!(!layout.state().join("notes").join("worker.txt").exists());
    assert!(!layout.artifacts().join("notes").join("worker.txt").exists());
    assert!(!layout
        .tool_output()
        .join("notes")
        .join("worker.txt")
        .exists());

    drop(temp);
}

#[tokio::test]
async fn worker_tool_storage_terminal_missing_working_directory_defaults_to_prepared_repo_root_and_traces_to_tool_output(
) {
    let (temp, layout) = prepared_layout("tooling-terminal-default");
    let storage = worker::tooling::worker_tool_storage(&layout).unwrap();

    let output = execute_tool(
        &storage,
        "terminal",
        r#"{"command":"echo worker-terminal > terminal-root.txt"}"#,
    )
    .await;
    let raw_output_ref = raw_output_ref(&output)
        .unwrap_or_else(|| panic!("terminal output should include raw_output_ref: {output}"));
    let trace_path = PathBuf::from(&raw_output_ref);
    let trace = fs::read_to_string(&trace_path).unwrap();

    assert!(
        output.contains(r#""exit_code":0"#),
        "terminal output should report success: {output}"
    );
    assert!(layout.repo().join("terminal-root.txt").is_file());
    assert!(is_child_path(&trace_path, &layout.tool_output()));
    assert!(
        trace.contains("worker-terminal"),
        "trace should contain raw command output: {trace}"
    );

    drop(temp);
}

#[tokio::test]
async fn worker_tool_storage_terminal_relative_working_directory_resolves_under_prepared_repo_root()
{
    let (temp, layout) = prepared_layout("tooling-terminal-relative");
    fs::create_dir_all(layout.repo().join("nested")).unwrap();
    let storage = worker::tooling::worker_tool_storage(&layout).unwrap();

    let output = execute_tool(
        &storage,
        "terminal",
        r#"{"command":"echo nested-terminal > nested-created.txt","working_directory":"nested"}"#,
    )
    .await;

    assert!(
        output.contains(r#""exit_code":0"#),
        "terminal output should report success: {output}"
    );
    assert!(layout
        .repo()
        .join("nested")
        .join("nested-created.txt")
        .is_file());
    assert!(!layout.repo().join("nested-created.txt").exists());

    drop(temp);
}

fn prepared_layout(name: &str) -> (TempRoot, WorkerLayout) {
    let temp = TempRoot::new(name);
    let root = temp.path().join("workers");
    fs::create_dir_all(&root).unwrap();
    let layout = prepare_worker_layout(&root, worker_id(name)).unwrap();
    (temp, layout)
}

async fn execute_tool(storage: &agent::ToolStorage, name: &str, arguments: &str) -> String {
    storage
        .execute(
            &ProviderToolCall::new("call-id", name, arguments),
            Cancellation::default(),
        )
        .await
}

fn raw_output_ref(output: &str) -> Option<String> {
    let field = r#""raw_output_ref":"#;
    let start = output.find(field)? + field.len();
    let start = start + output[start..].find('"')? + 1;
    let mut value = String::new();
    let mut escaped = false;

    for character in output[start..].chars() {
        if escaped {
            value.push(match character {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}

fn is_child_path(path: &Path, root: &Path) -> bool {
    let path = path.canonicalize().unwrap();
    let root = root.canonicalize().unwrap();
    path.starts_with(root)
}

fn assert_worker_manifest_depends_on_shared_tools() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).unwrap();

    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == r#"tools = { path = "../tools" }"#),
        "worker must depend on the shared tools package instead of copying built-ins"
    );
}

fn worker_id(value: &str) -> WorkerId {
    value.parse().unwrap()
}

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("doric-worker-tooling-{name}-{}", suffix()));
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
