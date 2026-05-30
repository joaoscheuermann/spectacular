# Validation: Effort 14 Lifecycle Integration Smoke

## Red evidence

- Command: `cargo test -p daemon lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names --no-fail-fast`
- Exit code: `1`
- Result: failed in `packages/daemon/tests/lifecycle_service.rs`.
- Failure summary:

```text
assertion `left == right` failed
  left: []
 right: ["repo_preparation", "prompt_agent_started", "prompt_artifact_written", "prompt_agent_completed"]
error: test failed, to rerun pass `-p daemon --test lifecycle_service`
error: 1 target failed:
    `-p daemon --test lifecycle_service`
```

## Acceptance mapping

The red test drives the daemon lifecycle service dispatch seam and the daemon worker session seam without importing worker internals. It records worker `WorkerEvent` frames named `repo_preparation`, `prompt_agent_started`, `prompt_artifact_written`, and `prompt_agent_completed`, then replays the lifecycle stream from sequence `0`.

The failure maps to effort 14 acceptance criteria because replay currently does not preserve those worker milestone names. The missing replayed names block retained lifecycle milestone evidence for stream replay from sequence `0` and the prompt lifecycle visibility expected by AC-7, AC-10, AC-15, F-09, and F-11.

## Smallest needed production change

The code writer should preserve incoming `WorkerEvent.name` values when daemon `SessionManager::record_event` appends registry events. The current behavior collapses events through status-derived registry names such as running or terminal status labels, so prompt milestone names are lost before lifecycle replay.

## Green evidence

- Command: `cargo test -p daemon lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names --no-fail-fast`
- Exit code: `0`
- Result: focused daemon lifecycle replay smoke passed; replay preserved `repo_preparation`, `prompt_agent_started`, `prompt_artifact_written`, and `prompt_agent_completed`.
- Command: `cargo test -p daemon --no-fail-fast`
- Exit code: `0`
- Result: 59 daemon unit tests, 1 lifecycle integration test, and daemon doctests passed.
- Command: `cargo fmt --all -- --check`
- Exit code: `0`
- Result: workspace formatting check passed.
- Command: `git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff --check`
- Exit code: `0`
- Result: whitespace check passed; Git emitted line-ending warnings for existing touched files.

## Validator/refactor evidence

- Command: `cargo test -p daemon lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names --no-fail-fast`
- Exit code: `0`
- Result: focused daemon lifecycle replay smoke passed; 1 integration test passed.
- Command: `cargo test -p daemon --no-fail-fast`
- Exit code: `0`
- Result: 59 daemon unit tests, 1 lifecycle integration test, and daemon doctests passed.
- Command: `cargo test -p lifecycle --no-fail-fast`
- Exit code: `0`
- Result: 19 lifecycle tests and lifecycle doctests passed.
- Command: `cargo fmt --all -- --check`
- Exit code: `0`
- Result: workspace formatting check passed.
- Command: `cargo clippy -p daemon -p lifecycle --all-targets -- -D warnings`
- Exit code: `0`
- Result: daemon and lifecycle clippy checks passed with warnings denied.
- Command: `git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff --check`
- Exit code: `0`
- Result: whitespace check passed; Git emitted line-ending warnings for existing working-copy files.

Validator decision: current effort 14 smoke slice is green. The planner's full end-to-end decomposition gap remains a non-blocking residual risk for this slice because the implemented test validates the legal daemon/session/protocol boundary without adding cross-package dependencies.

## Coordinator regression evidence

- Command: `cargo test -p worker --no-fail-fast`
- Exit code: `0`
- Result: 59 worker unit tests passed; worker lib/bin empty test targets and doctests passed.
- Command: `cargo test -p cli --no-fail-fast`
- Exit code: `0`
- Result: 181 CLI unit tests and 2 process tests passed.
- Command: `cargo build -p cli --bin doric`
- Exit code: `0`
- Result: `doric` binary build passed.
- Command: `cargo build -p daemon --bin doric-daemon`
- Exit code: `0`
- Result: `doric-daemon` binary build passed.
- Command: `cargo build -p worker --bin doric-worker`
- Exit code: `0`
- Result: `doric-worker` binary build passed.
