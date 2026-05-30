# Agent Receipt: effort 06 test planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7697-0827-71b3-b68c-95a39eb1e412
- Spawn result: completed; wrote this receipt at the assigned path
- Required agents row: `| development | efforts/06_daemon_lifecycle_service.md | test planner | worker | agents/066_effort_06_test_planner.md | 019e7697-0827-71b3-b68c-95a39eb1e412 | accepted |`

## Role

Doric development test planner for effort 06, translating daemon lifecycle service acceptance criteria into focused service and server unit tests.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/06_daemon_lifecycle_service.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/PRD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/05_daemon_registry_root.md`
- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- `packages/lifecycle/src/proto.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/src/root.rs`
- `packages/daemon/src/event.rs`
- `packages/daemon/src/error.rs`
- `packages/daemon/src/lib.rs`
- `packages/daemon/Cargo.toml`
- `packages/daemon/tests/unit/registry.rs`
- `packages/daemon/tests/unit/root.rs`

## Read ownership

- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- `packages/lifecycle/src/proto.rs`
- `packages/daemon/src/{registry.rs,root.rs,event.rs,error.rs,lib.rs}`
- Existing daemon unit tests under `packages/daemon/tests/unit`
- Effort 06 and upstream lifecycle planning artifacts in the run directory

## Write ownership

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/066_effort_06_test_planner.md`

## Coding conventions used

- `.agents/skills/coding-conventions/SKILL.md`: constrained the plan to behavior-first TDD, focused unit tests, public contracts over private internals, explicit dependency injection for daemon worker launch and command delivery seams, and no speculative shared abstractions.
- `references/implementation-standards.md`: constrained test placement to `packages/daemon/tests/unit/*.rs` with a minimal Rust include harness in `src/lib.rs`; tests should be F.I.R.S.T., Arrange-Act-Assert, self-validating, and named with explicit method/scenario/expected behavior meaning adapted to Rust.
- `references/sexy-rust.md`: constrained recommended Rust APIs toward typed boundary values, flat `Result` handling, small semantic structs for service dependencies/configuration, and lifecycle domain types instead of raw string state where practical.

## Prompt summary

Plan tests only for effort 06. Do not edit production or test code. The planned tests must cover the daemon CLI-facing lifecycle service and server construction/binding configuration using fake worker launch, fake command delivery, existing root validation, and the effort 05 registry/event behavior. Recommended Red should fail because `daemon::service` and `daemon::server` are not implemented yet.

## Output

### Current code facts that shape the tests

- `packages/daemon/src/lib.rs` currently exports `error`, `event`, `registry`, and `root`; it does not export `service` or `server`.
- `packages/daemon/Cargo.toml` currently depends on `config` and `lifecycle` only. Service/server implementation will likely need `tokio`, `tonic`, and async stream support dependencies.
- `packages/lifecycle/proto/doric/lifecycle/v1.proto` already defines `LifecycleService` RPCs: `Dispatch`, `ListWorkers`, `StreamWorker`, and `AnswerInput`.
- Effort 05 green validation proves root and registry tests pass, including in-memory list/replay, history truncation, pending input, duplicate answer rejection, root validation, and worker layout creation.
- Existing daemon unit tests live in `packages/daemon/tests/unit/{root.rs,registry.rs}` and are included from `src/lib.rs` under nested modules.

### Test files to create or update

- Create `packages/daemon/tests/unit/service.rs`.
- Create `packages/daemon/tests/unit/server.rs`.
- Update `packages/daemon/src/lib.rs` to export `pub mod service;` and `pub mod server;` and include test modules:

```rust
#[cfg(test)]
mod tests {
    mod service {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/service.rs"));
    }

    mod server {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/server.rs"));
    }
}
```

Keep production and test implementation changes for later agents. This receipt only plans the tests.

### API shape recommendations

- `service::LifecycleService` or `service::DaemonLifecycleService` should be constructed from explicit dependencies:
  - `Arc<Mutex<Registry>>` or a small registry handle.
  - Validated worker root path or `RootConfig` that validates before record creation.
  - `WorkerLauncher` trait with a fake implementation that records launch requests and can fail deterministically.
  - `WorkerCommandSender` or `CommandRouter` trait with a fake implementation that records answer forwarding.
  - Optional `IdGenerator` trait so dispatch tests can assert stable ids without parsing random values.
- `WorkerLauncher::launch` should receive a typed launch request containing worker id, mode, raw prompt, raw repo, redacted repo identity, and prepared layout paths. It must not run a process in service unit tests.
- `WorkerCommandSender::answer` should receive `(WorkerId, RequestId, text)` after daemon pending-input validation succeeds. It should not be called for unknown worker, stale request, duplicate answer, terminal worker, or non-waiting worker.
- Dispatch validation should happen before `Registry::insert` and before launcher invocation. Validate non-empty prompt, non-empty repo, known mode, and worker-root accessibility/layout creation.
- Service methods should expose plain async Rust methods for unit tests, then adapt to tonic generated traits. Example: `dispatch(DispatchRequest) -> DaemonResult<DispatchResponse>`, `list(ListWorkersRequest)`, `stream(StreamWorkerRequest)`, and `answer_input(AnswerInputRequest)`.
- `server::ServerConfig` should hold bind address and worker root. `server::build_router(config, service)` or `server::service_definition(config, deps)` should be testable without binding a real listener.
- Error mapping should be concise and redacted. Unit tests can assert `tonic::Status` code or daemon error variant, but should avoid depending on private error formatting beyond required unknown/root/invalid wording.

### Service tests mapped to acceptance criteria

`packages/daemon/tests/unit/service.rs`

- `dispatch_empty_prompt_rejects_without_record_or_launch`
  - Covers AC-3 and effort criterion "validating prompt before record creation."
  - Arrange fake registry/service with valid root and launcher. Act with empty prompt and valid repo/mode. Assert error names prompt, registry list remains empty, and fake launcher saw no launch.

- `dispatch_empty_repo_rejects_without_record_or_launch`
  - Covers AC-3 and effort criterion "validating repo before record creation."
  - Assert no registry row and no launch when repo is blank or whitespace.

- `dispatch_unspecified_mode_rejects_without_record_or_launch`
  - Covers FR-2/FR-3 and mode validation.
  - Use `JobMode::Unspecified` from the generated contract. Assert invalid mode error, no row, and no launch.

- `dispatch_invalid_root_rejects_before_record_creation`
  - Covers AC-13.
  - Arrange root dependency or root path that fails validation. Assert root-configuration error/status, registry is empty, and launcher was not called.

- `dispatch_feature_valid_request_returns_identity_status_and_launches_once`
  - Covers AC-1, AC-12, AC-15.
  - Arrange deterministic id `worker-feature-1`, valid root, fake launcher success, prompt, and credential-bearing repo URL. Assert response includes id, mode `FEATURE`, redacted repo identity without credentials, and `ACCEPTED` or `STARTING` status. Assert registry has one row and launcher received feature mode plus raw prompt/repo and prepared layout.

- `dispatch_debug_valid_request_preserves_requested_mode_label`
  - Covers AC-2 and AC-15.
  - Same as feature test but with debug mode. Assert response/list/launch preserve debug mode while not claiming separate workflow behavior.

- `dispatch_launcher_failure_marks_or_rejects_without_secret_leak`
  - Covers AC-9 and AC-12.
  - If design inserts before launch, assert worker is marked failed with redacted reason. If design launches before final accept, assert request fails and no row is created. Pick one implementation contract and make the test pin it. Recommended: insert accepted, attempt launch, then mark failed so list/stream can expose failure.

- `list_empty_registry_returns_empty_response`
  - Covers AC-5.
  - Assert service list succeeds with `workers.is_empty()` and no fake worker access.

- `list_active_worker_returns_required_summary_fields`
  - Covers AC-6.
  - Seed registry with running worker and event. Assert summary includes id, mode, redacted repo identity, status, current activity or latest sequence, and no raw credentials.

- `list_waiting_worker_returns_waiting_status_and_request_context`
  - Covers AC-6 and AC-11.
  - Seed running worker and call registry input request. Assert summary status is waiting and exposes enough correlation for CLI rendering. If proto `WorkerSummary` lacks request id/activity fields, this test should fail until the service maps them through an added DTO/proto extension or accepted alternative.

- `list_failed_worker_returns_failure_reason`
  - Covers AC-6 and AC-9.
  - Seed failed worker with concise reason. Assert summary/status includes failed state and reason if the service response supports it. If current proto lacks reason, this is a contract gap for implementation to resolve.

- `list_terminal_workers_returns_succeeded_failed_and_stopped_rows`
  - Covers AC-6, AC-9, and AC-14.
  - Seed succeeded, failed, and stopped records. Assert all terminal rows remain listed and statuses are preserved.

- `stream_unknown_worker_returns_unknown_worker_error_without_hanging`
  - Covers AC-8 and AC-14.
  - Call stream for an unknown id. Assert immediate unknown/untracked error and no live subscription is opened.

- `stream_known_worker_replays_retained_events_in_order`
  - Covers AC-7.
  - Seed accepted, starting, repo preparation, prompt agent, activity, and succeeded events. Assert stream returns replayed events in sequence order from `from_sequence = 0`.

- `stream_from_middle_replays_requested_suffix`
  - Covers replay behavior.
  - Seed several events and request from sequence `2`. Assert first emitted worker event has sequence `>= 2` and earlier events are absent.

- `stream_truncated_history_emits_history_truncated_before_retained_events`
  - Covers AC-7 and TDD replay decision.
  - Use a registry capacity small enough to evict early events. Assert stream starts with a history-truncated event or tonic-compatible equivalent before retained worker events.

- `stream_follow_receives_live_event_after_replay`
  - Covers daemon-mediated live tail if practical.
  - If service exposes a fakeable subscriber/broadcast seam, start stream after replay, append a live event, and assert the stream yields it without polling private registry internals. If the live-tail seam is not ready, keep this test planned but allow the test writer to defer it behind a practical service subscription API.

- `answer_input_pending_request_accepts_forwards_and_emits_continuation`
  - Covers AC-11.
  - Seed waiting worker with request id. Call service answer with text. Assert response accepted, command sender recorded exactly one answer with raw answer text, registry status returns running, and replay includes `answer_provided` continuation without exposing the answer body.

- `answer_input_unknown_worker_rejects_without_forwarding`
  - Covers AC-8 and AC-11.
  - Assert unknown worker error and fake command sender call count remains zero.

- `answer_input_unknown_request_rejects_without_forwarding`
  - Covers AC-11.
  - Seed waiting worker with request `r1`, answer `missing-r2`. Assert stale/unknown request error and no command forwarding.

- `answer_input_duplicate_answer_rejects_without_second_forward`
  - Covers AC-11.
  - First answer succeeds and forwards once. Second answer with same request fails as duplicate/stale/no-pending and does not forward again.

- `answer_input_non_waiting_worker_rejects_without_forwarding`
  - Covers AC-11.
  - Seed running worker with no pending input. Assert non-waiting/no-pending error and no command forwarding.

### Server tests mapped to acceptance criteria

`packages/daemon/tests/unit/server.rs`

- `server_config_default_loopback_uses_lifecycle_daemon_port`
  - Covers TDD daemon binding default.
  - Assert default config address is `127.0.0.1:47821`.

- `server_config_custom_addr_preserves_requested_bind_addr`
  - Covers CLI/server configuration contract.
  - Assert custom bind address is parsed and stored without binding a listener.

- `server_config_invalid_addr_returns_configuration_error`
  - Covers clear setup failure.
  - Assert invalid address returns an error before service construction.

- `build_service_valid_config_constructs_lifecycle_and_worker_session_services`
  - Covers server module construction without real network listener.
  - Use fake launcher/command dependencies and temp root. Assert returned server/router descriptor contains the CLI-facing `LifecycleService` and does not spawn or bind.

- `build_service_invalid_worker_root_fails_before_binding`
  - Covers AC-13 at server startup composition.
  - Arrange missing/file root. Assert root-configuration error and no listener/launcher call.

- `server_run_with_injected_listener_uses_supplied_listener_without_opening_real_socket`
  - Covers binding seam if practical.
  - If implementation exposes a `serve_with_incoming` or injected listener seam, pass an empty in-memory incoming stream and assert construction succeeds/can be cancelled. If not practical in unit tests, prefer the router/config tests above and leave real listener coverage for integration.

### Focused Red commands and expected failure mode

Run these after the test writer adds only the planned test files and minimal test harness:

```text
cargo test -p daemon --no-fail-fast service::
cargo test -p daemon --no-fail-fast server::
```

Expected Red:

- Compile failure is the expected first Red because `daemon::service` and `daemon::server` do not exist yet and `packages/daemon/src/lib.rs` does not export those modules.
- Representative errors should include unresolved imports such as `could not find service in crate`, `could not find server in crate`, missing `WorkerLauncher`/`WorkerCommandSender` traits, missing lifecycle service struct, or missing server config/builder types.
- A secondary expected compile failure may be missing dependencies for async/gRPC service implementation, such as `tokio`, `tonic`, `tokio-stream`, or similar.
- If the modules compile but tests fail, acceptable Red failures are assertion-level contract failures around validation before record creation, repo credential redaction, missing waiting/failure fields in list responses, missing history-truncated replay, missing live-tail seam, or answer forwarding before daemon validation.

Do not count unrelated Cargo/Nx metadata failures as valid Red. The failure must point at the missing effort 06 service/server APIs or the planned public contract.

### Regression commands after implementation

Focused effort gates:

```text
cargo fmt --all -- --check
cargo test -p daemon --no-fail-fast service::
cargo test -p daemon --no-fail-fast server::
cargo test -p daemon --no-fail-fast
cargo test -p lifecycle --no-fail-fast
cargo clippy -p daemon --all-targets -- -D warnings
npx nx run daemon:test
```

Broader confidence before handoff:

```text
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p daemon --bin doric-daemon
```

Windows file-lock fallback:

```text
$env:CARGO_TARGET_DIR='dist\target\effort-06-verify'; cargo test -p daemon --no-fail-fast
$env:CARGO_TARGET_DIR='dist\target\effort-06-verify'; cargo clippy -p daemon --all-targets -- -D warnings
```

### Testability risks

- Current proto `WorkerSummary` has no activity, terminal reason, or pending request fields even though PRD AC-6 and AC-11 require them in list output. Service tests should expose this as a contract gap unless implementation provides a documented alternative mapping.
- Current proto `WorkerEvent` has no explicit `history_truncated` oneof; registry uses `lifecycle::event::StreamEvent` for truncation. Stream service tests should force an explicit service-level representation rather than silently dropping truncation.
- Live-tail testing can become flaky if implemented through real timers or sockets. Prefer injected broadcast/subscriber seams and deterministic stream polling.
- Server tests should not open real listeners. A testable builder/configuration layer is enough for effort 06; process binding smoke can happen later.
- Fake launcher failure ordering must be decided. The recommended contract is to create the accepted record, attempt launch, and mark failed on launcher error so list/stream can report the failure.

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/066_effort_06_test_planner.md`

## Blocking questions

- None blocking. There are contract gaps for list waiting/failure details and history truncation representation in the current proto, but they can be surfaced by Red tests and resolved by the effort 06 implementation or a coordinated lifecycle contract update.

## Coordinator decision

accepted
