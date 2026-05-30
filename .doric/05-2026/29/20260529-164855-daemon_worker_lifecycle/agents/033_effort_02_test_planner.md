# Agent Receipt: effort 02 test planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e764b-192c-7b23-933a-6126db89abd9
- Spawn result: completed
- Close result: completed status returned before shutdown
- Required agents row: `| development | efforts/02_lifecycle_proto_codegen.md | test planner | worker | agents/033_effort_02_test_planner.md | 019e764b-192c-7b23-933a-6126db89abd9 | accepted |`

## Role

Test planner for effort 02 lifecycle proto/codegen.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/02_lifecycle_proto_codegen.md`
- `.agents/skills/doric/references/05-development.md`
- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/current-architecture/SKILL.md`
- Existing lifecycle skeleton files.

## Read ownership

- `packages/lifecycle/Cargo.toml`
- `packages/lifecycle/src/lib.rs`
- Existing package test patterns.
- TDD protocol and codegen sections.

## Write ownership

- None.

## Coding conventions

Used:

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/monodon-rust.md`
- `.agents/skills/current-architecture/SKILL.md`

Constraints applied:

- Place behavior/contract tests under the package `tests/` directory.
- Use the repo-local `#[cfg(test)] include!` harness pattern when a nested `tests/unit/*.rs` file needs to run in the library test target.
- Keep tests focused on public generated contract reachability and avoid hand-written DTO/redaction behavior in this effort.

## Prompt summary

Translate effort 02 acceptance criteria into concrete red tests and validation commands for the lifecycle generated proto contract.

## Output

Proposed test file:

- `packages/lifecycle/tests/unit/proto_contract.rs`

Proposed harness:

- Add a minimal `#[cfg(test)]` include block in `packages/lifecycle/src/lib.rs` so `cargo test -p lifecycle --lib --no-fail-fast` runs the nested unit test file.

Proposed tests:

- `proto_namespace_exposes_cli_facing_messages`: compile-time reachability for `DispatchRequest`, `DispatchResponse`, `ListWorkersRequest`, `ListWorkersResponse`, `StreamWorkerRequest`, `AnswerInputRequest`, `AnswerInputResponse`, `WorkerSummary`, `WorkerEvent`, `InputRequest`, `JobMode`, and `WorkerStatus` from `lifecycle::proto::doric::lifecycle::v1`.
- `proto_namespace_exposes_worker_session_messages`: compile-time reachability for `WorkerFrame`, `DaemonFrame`, `WorkerHello`, `WorkerStatusUpdate`, `StartJob`, `AnswerInputCommand`, and `ShutdownWorker`.
- `proto_namespace_exposes_generated_service_modules`: compile-time reachability for `lifecycle_service_client::LifecycleServiceClient`, `lifecycle_service_server::{LifecycleService, LifecycleServiceServer}`, `worker_session_service_client::WorkerSessionServiceClient`, and `worker_session_service_server::{WorkerSessionService, WorkerSessionServiceServer}`.
- `proto_frames_expose_expected_oneof_variants`: compile-time reachability for `worker_frame::Frame::{Hello, Status, Event}` and `daemon_frame::Frame::{StartJob, Answer, Shutdown}`.

Red commands:

- `cargo test -p lifecycle --lib --no-fail-fast`
- `cargo build -p lifecycle`

Expected red reason: missing `lifecycle::proto::doric::lifecycle::v1` module or missing generated message/service symbols before proto/codegen implementation.

Focused green commands:

- `cargo build -p lifecycle`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo fmt --all -- --check`

Regression commands:

- `npx nx run lifecycle:build`
- `npx nx run lifecycle:test`

## Files changed

- None by sub-agent.

## Blocking questions

- None.

## Coordinator decision

accepted
