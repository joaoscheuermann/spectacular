# Agent Receipt: Effort 14 Test Planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7867-1400-7002-b9ee-93751dc47795
- Spawn result: spawned
- Required agents row: `development | efforts/14_lifecycle_integration_smoke.md | test planner | worker | agents/149_effort_14_test_planner.md | 019e7867-1400-7002-b9ee-93751dc47795 | spawned`

## Role

Effort 14 test planner. Planning-only assignment to translate lifecycle integration-smoke acceptance criteria into concrete test files, red-test order, expected current failures, and validation commands.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/architecture-principles.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/14_lifecycle_integration_smoke.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/13_cli_daemon_client_output.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/146_effort_13_reviewer_retry_after_registry_repair.md`

## Read ownership

Read was limited to the required Doric artifacts, coding-conventions Rust/testing and architecture references, existing package test layout, and focused lifecycle source seams:

- `packages/cli/tests/**`
- `packages/daemon/tests/**`
- `packages/worker/tests/**`
- `packages/lifecycle/tests/**`
- `packages/cli/src/main/lifecycle.rs`
- `packages/cli/src/main/lifecycle_client.rs`
- `packages/daemon/src/service.rs`
- `packages/daemon/src/registry.rs`
- `packages/daemon/src/server.rs`
- `packages/daemon/src/worker_session.rs`
- `packages/daemon/src/event.rs`
- `packages/worker/src/runtime.rs`
- `packages/worker/tests/unit/runtime.rs`
- `packages/worker/tests/unit/runtime_support.rs`
- `packages/lifecycle/src/event.rs`
- `packages/lifecycle/src/status.rs`
- `packages/cli/Cargo.toml`
- `packages/daemon/Cargo.toml`
- `packages/worker/Cargo.toml`

## Write ownership

Wrote only this receipt:

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/149_effort_14_test_planner.md`

No implementation files, package tests, `STATE.md`, or validation artifacts were edited.

## Coding conventions

- Loaded `.agents/skills/coding-conventions/SKILL.md` before planning.
- Used `references/implementation-standards.md` for Red-Green-Refactor, F.I.R.S.T., behavior-oriented tests through public contracts, explicit test doubles for external dependencies, and package `tests/` placement.
- Used `references/architecture-principles.md` for dependency direction: `daemon` must not depend on `worker` internals, `worker` must not depend on `daemon`, and lifecycle contracts remain the shared boundary.
- Did not load TypeScript, Python, or Rust polish references because this was a Rust integration-test planning task, not implementation review.
- Placement constraint: Cargo does not automatically execute arbitrary nested `tests/integration/*.rs` files. If the target paths from effort 14 are kept, add Cargo-visible root harnesses such as `packages/daemon/tests/lifecycle_service.rs` that include `integration/lifecycle_service.rs`, or place the integration tests directly at `packages/<crate>/tests/<name>.rs`.

## Prompt summary

Plan the smallest lifecycle integration smoke tests for effort 14. Propose target files and cargo commands, identify first red tests and expected failure mode, preserve scope, and flag any decomposition gap if AC-1 through AC-15 cannot be covered without widening public contracts or package boundaries.

## Test plan

1. `packages/daemon/tests/integration/lifecycle_service.rs`
   - Add a daemon integration smoke through public `daemon` and `lifecycle` APIs only.
   - Use `Arc<Mutex<Registry>>`, `LifecycleService`, `SessionManager`, `SessionCommandSender`, and a `WorkerLauncher` fake that registers a `SessionWorker` from the `LaunchRequest`.
   - Dispatch a feature job, attach the worker with `WorkerHello`, then record worker protocol frames that represent repo preparation, prompt-agent start, artifact write, input wait, answer continuation, prompt-agent completion, and terminal success.
   - Assert:
     - dispatch returns worker id, feature mode, redacted repo, and accepted status;
     - stream replay from sequence `0` includes retained milestones in order;
     - list shows waiting state with request id before answer;
     - `answer_input` forwards the answer through `SessionCommandSender`, emits continuation, clears pending input, and returns to running;
     - terminal success is visible through list and stream;
     - repo credentials and provider-looking secrets do not appear in replay/list text.
   - Keep this as a daemon/session/protocol smoke, not a worker-runtime import. `daemon` should not add a `worker` dependency.

2. `packages/daemon/tests/integration/lifecycle_service.rs`
   - Add a failure smoke beside the success path.
   - Seed or dispatch a worker, then record either a repo failure frame or child/process failure through the existing public session/process seam.
   - Assert list status is `failed`, terminal reason is redacted and actionable, and stream replay includes a failed terminal event.
   - This is the smallest AC-9 coverage without network repo or live worker process.

3. `packages/worker/tests/integration/runtime_session.rs`
   - Add a public-worker runtime smoke using `run_worker_session`, fake `DaemonSession`, fake `RepoPreparer`, and fake `RuntimePromptRunner`.
   - Start a job, have fake repo preparation succeed, emit prompt-agent events including input wait, feed the matching answer, then complete.
   - Assert outbound protocol frames are `Hello`, running status, repo preparation event, prompt milestones, waiting status/input request, answer-consumed continuation, and one succeeded terminal status.
   - This duplicates some unit-test behaviors intentionally at the public API boundary and proves the fakeable seam required by F-12.

4. `packages/worker/tests/integration/runtime_session.rs`
   - Add a repo/provider failure smoke with secret-bearing text.
   - Assert prompt runner is not invoked for repo failure, terminal status is failed, and secret text is redacted from status/event messages.
   - Covers AC-9 and AC-12 at the worker-runtime boundary.

5. `packages/cli/tests/lifecycle_smoke.rs`
   - Add a CLI process smoke against an in-process daemon lifecycle server.
   - Start `daemon::server::serve_bundle_on_listener_with_shutdown` on `127.0.0.1:0` with fake launcher, fake command sender, and fixed id generator.
   - Invoke the `doric` binary via `CARGO_BIN_EXE_doric`:
     - `doric list --addr <addr>` for empty-state AC-5;
     - `doric feature --prompt ... --repo https://token@example.com/org/repo.git --addr <addr>` for AC-1 redaction and output scope;
     - `doric worker seeded-worker --addr <addr>` against a pre-seeded worker for stream replay/truncation/input rendering;
     - `doric answer seeded-worker request-1 --text continue --addr <addr>` for answer output.
   - Do not try to prove real worker runtime through this CLI test unless a Cargo-visible test harness can own a shared registry/session manager. The current CLI crate depends on `daemon` and `lifecycle`, not `worker`, and should not become the cross-package worker-runtime owner.

6. `packages/lifecycle/tests/unit/proto_contract.rs`
   - Add only lightweight protocol contract assertions if the daemon/worker smoke exposes missing public fields.
   - Candidate: assert `WorkerEvent` preserves arbitrary prompt milestone names such as `prompt_artifact_written` and `prompt_agent_completed` through generated frames.
   - Do not change protobuf contracts in effort 14 unless a smoke test proves an existing contract field is unavailable.

## Red-test recommendation

Add the daemon integration milestone-preservation test first:

- File: `packages/daemon/tests/integration/lifecycle_service.rs` plus a Cargo-visible harness if using the nested path.
- Test name: `lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names`.
- Smallest behavior: record worker `WorkerEvent` frames with names `repo_preparation`, `prompt_agent_started`, `prompt_artifact_written`, and `prompt_agent_completed`; then stream from sequence `0` and assert those exact names appear in order.
- Expected current failure: `SessionManager::record_event` currently maps incoming worker events by status through `event_for_status` instead of preserving the event name. Running events will replay as `current_activity`, and a prompt completion frame with succeeded status will replay as `succeeded` instead of `prompt_agent_completed`. That loses lifecycle milestones required by AC-7, AC-10, AC-15, and F-09/F-11.

Add the CLI process smoke second:

- File: `packages/cli/tests/lifecycle_smoke.rs`.
- Test name: `doric_worker_stream_against_in_process_daemon_renders_seeded_replay_and_input_request`.
- Expected current failure depends on whether seeded events use existing service helpers or real worker frames. If seeded with `seed_events_for_test`, it may pass; if driven through real daemon session frames after the first red test, it should fail for the same milestone-collapse reason until daemon recording is fixed.

Add the worker runtime public-boundary smoke third:

- File: `packages/worker/tests/integration/runtime_session.rs` plus a Cargo-visible harness if using the nested path.
- Test name: `run_worker_session_prompt_input_answer_success_emits_single_terminal_success`.
- Expected current result is likely green because package-local unit tests already cover the same runtime seam. Keep it as regression smoke after the daemon red test rather than the first red test.

## Commands

First red command:

```text
cargo test -p daemon lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names --no-fail-fast
```

Focused implementation loop after the first fix:

```text
cargo test -p daemon lifecycle_service --no-fail-fast
cargo test -p daemon worker_session --no-fail-fast
cargo test -p worker runtime_session --no-fail-fast
cargo test -p cli lifecycle_smoke --no-fail-fast
```

Effort 14 regression commands, prioritized from narrow to broad:

```text
cargo test -p lifecycle --no-fail-fast
cargo test -p daemon --no-fail-fast
cargo test -p worker --no-fail-fast
cargo test -p cli --no-fail-fast
cargo build -p cli --bin doric
cargo build -p daemon --bin doric-daemon
cargo build -p worker --bin doric-worker
```

Workspace hygiene after behavior is green:

```text
cargo fmt --all -- --check
cargo clippy -p lifecycle -p daemon -p worker -p cli --all-targets -- -D warnings
git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff --check
```

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/149_effort_14_test_planner.md`

## Blocking questions

- No blocker for the first red daemon smoke.
- Decomposition gap: direct AC-1 through AC-15 end-to-end coverage across CLI, daemon gRPC, generated worker-session service, real `doric-worker`, fake repo, and fake prompt runner is not currently a single legal package-local test. The current public server exposes the CLI-facing `LifecycleService`, while worker attach/session is a plain Rust `SessionManager` seam and no package owns both daemon and worker as dependencies. The smaller legal smoke slice is daemon session/protocol integration, worker runtime public-boundary integration, and CLI process-to-daemon integration. Full generated worker-session service exposure or a workspace-level integration crate should be decomposed separately if required.
- Placement gap: effort 14 target paths under `tests/integration/*.rs` need Cargo-visible harness files or direct `tests/*.rs` placement to actually run.

## Coordinator decision

Coordinator decision: accepted.
