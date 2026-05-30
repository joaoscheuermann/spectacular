# Agent Receipt: Effort 13 Daemon Stream List Repair

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7788-6491-7e70-b012-a646d7fc45ce
- Spawn result: spawned
- Required agents row: `development | efforts/13_cli_daemon_client_output.md | daemon/stream/list repair writer | worker | agents/139_effort_13_daemon_stream_list_repair.md | 019e7788-6491-7e70-b012-a646d7fc45ce | spawned`

## Role

Repair the independent review blockers for effort 13 with the minimum focused code changes.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/138_effort_13_reviewer.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/13_cli_daemon_client_output.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/PRD.md`
- `.agents/skills/doric/SKILL.md`
- `.agents/skills/coding-conventions/SKILL.md`

## Read ownership

- Current changed CLI, lifecycle, daemon files and tests.
- Rejected review receipt and effort/PRD/TDD context.

## Write ownership

- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- `packages/lifecycle/tests/unit/proto_contract.rs`
- `packages/daemon/Cargo.toml`
- `packages/daemon/src/main.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/src/server.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/tests/unit/server.rs`
- `packages/daemon/tests/unit/service.rs`
- `packages/cli/Cargo.toml`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main/lifecycle_client.rs`
- `packages/cli/src/main/lifecycle_output.rs`
- `packages/cli/src/main/output.rs`
- `packages/cli/tests/unit/lifecycle_output.rs`
- `Cargo.lock`
- This receipt.

## Coding conventions

Loaded `.agents/skills/coding-conventions/SKILL.md` and its Rust reference. Kept changes behavior-focused, used existing package test include layout, avoided unrelated refactors, and preserved unrelated staged/user changes.

## Prompt summary

Repair effort 13 rejection blockers: real foreground daemon serving, generated-gRPC `list` integration proof, live streaming CLI output, and production list summary fields for activity, terminal reason, and waiting request correlation.

## Output

### Blocker 1 repaired: daemon foreground server

- Added tonic serving runtime for the generated `LifecycleService` in `packages/daemon/src/server.rs:292`, `packages/daemon/src/server.rs:328`, and `packages/daemon/src/server.rs:356`.
- Updated `doric-daemon` to run the server instead of only building the service bundle in `packages/daemon/src/main.rs:1`.
- Updated `doric daemon` routing to parse the requested address/root and run the daemon foreground server in `packages/cli/src/main/entry.rs:162` and `packages/cli/src/main/entry.rs:192`.
- Added daemon runtime dependencies in `packages/daemon/Cargo.toml` and CLI daemon dependency in `packages/cli/Cargo.toml`.

### Blocker 2 repaired: live worker stream output

- Changed the CLI lifecycle client stream contract from collecting a `Vec` to invoking a sink as each gRPC event arrives in `packages/cli/src/main/lifecycle.rs:30` and `packages/cli/src/main/lifecycle_client.rs:29`.
- Updated worker command handling to write stream output to a writer during stream execution in `packages/cli/src/main/lifecycle.rs:190` and `packages/cli/src/main/lifecycle.rs:243`.
- Split stream formatting into header/item helpers for incremental rendering in `packages/cli/src/main/output.rs:494` and `packages/cli/src/main/output.rs:505`.
- Added `handle_lifecycle_worker_stream_writes_first_event_before_stream_closes` proving first event output is visible before the fake stream closes in `packages/cli/tests/unit/lifecycle_output.rs:182`.

### High finding repaired: list summary contract

- Extended `WorkerSummary` proto with `activity`, `terminal_reason`, and `pending_request_id` in `packages/lifecycle/proto/doric/lifecycle/v1.proto:73`.
- Mapped registry activity, terminal reason, and pending request id into production gRPC list responses in `packages/daemon/src/service.rs:186`.
- Mapped those fields into CLI DTOs in `packages/cli/src/main/lifecycle_client.rs:177`.
- Rendered terminal reason and waiting request correlation in list output in `packages/cli/src/main/output.rs:482`.
- Preserved existing redaction via lifecycle field rendering.

## Tests added/updated

- `packages/daemon/tests/unit/server.rs:106`: in-process tonic server with loopback listener proves generated `LifecycleServiceClient::list_workers` reaches the served `LifecycleService` path.
- `packages/daemon/tests/unit/service.rs:160`: list service mapping now asserts activity, pending request id, and terminal reason.
- `packages/lifecycle/tests/unit/proto_contract.rs:19`: proto contract asserts new generated summary fields.
- `packages/cli/tests/unit/lifecycle_output.rs:126`: list output asserts request correlation.
- `packages/cli/tests/unit/lifecycle_output.rs:182`: stream sink test asserts first event is written before stream closure.

## Validation evidence

- `cargo fmt --all -- --check`: pass.
- `cargo test -p lifecycle --no-fail-fast`: pass, 19 tests.
- `cargo test -p daemon --no-fail-fast`: pass, 59 tests.
- `cargo test -p cli --no-fail-fast`: pass, 181 unit tests plus 2 debug-log startup tests.
- `cargo build -p cli --bin doric`: pass.
- `cargo build -p daemon --bin doric-daemon`: pass.
- `cargo clippy -p lifecycle -p daemon -p cli --all-targets -- -D warnings`: pass.

## Failed commands during repair

- `cargo test -p daemon --no-fail-fast` and `cargo test -p cli --no-fail-fast` initially failed after the server alias rename because `server.rs` still referenced `LifecycleService::new`; repaired to `CoreLifecycleService::new`.
- `cargo fmt --all -- --check` initially failed with formatting diffs; ran `cargo fmt --all` and reran successfully.
- `cargo clippy -p lifecycle -p daemon -p cli --all-targets -- -D warnings` initially failed on a test helper with too many arguments; simplified the helper and reran successfully.

## Remaining risks / scope gaps

- The daemon now serves the CLI-facing generated lifecycle gRPC service. The generated worker-session service is still not exposed as a full live command stream; worker-session serving remains follow-up integration scope for complete worker attach/answer delivery.
- `doric daemon` uses the existing worker-root validation behavior. A missing default worker root still fails startup instead of creating the directory.
- Stream tailing closes on terminal events emitted through the lifecycle stream. Non-terminal streams remain open by design.

## Files changed

- `Cargo.lock`
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- `packages/lifecycle/tests/unit/proto_contract.rs`
- `packages/daemon/Cargo.toml`
- `packages/daemon/src/main.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/src/server.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/tests/unit/server.rs`
- `packages/daemon/tests/unit/service.rs`
- `packages/cli/Cargo.toml`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main/lifecycle_client.rs`
- `packages/cli/src/main/lifecycle_output.rs`
- `packages/cli/src/main/output.rs`
- `packages/cli/tests/unit/lifecycle_output.rs`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/139_effort_13_daemon_stream_list_repair.md`

Coordinator follow-up:

- Split lifecycle formatting out of `packages/cli/src/main/output.rs` into `packages/cli/src/main/lifecycle_output.rs` after reviewing this receipt, keeping `output.rs` below the repository 500-line hard threshold.
- Verified `cargo fmt --all -- --check`, `cargo test -p cli lifecycle_output --no-fail-fast`, and line counts `output.rs: 493`, `lifecycle_output.rs: 177`.

## Blocking questions

- None.

## Coordinator decision

Coordinator decision: accepted
