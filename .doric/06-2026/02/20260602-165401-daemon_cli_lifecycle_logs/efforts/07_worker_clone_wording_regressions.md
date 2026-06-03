# Effort: Worker clone wording and regressions

Status: done

## Requirement links

- FEATURES.md: F7, F8.
- PRD.md: FR8, FR10, FR11, FR12, FR13, FR20, FR21, FR23; User Value Hops 4, 7, 8, and 9.
- TDD.md: Proposed design components 7 and 8; Dependency Hops 9 and 12; worker tests; integration/regression testing strategy.

## Goal

Update worker repo-preparation lifecycle wording to `cloning repo: <redacted-repo-url>` while preserving raw clone execution and existing worker lifecycle semantics, then run focused cross-package regressions for the full feature chain.

## Sequence

- Position: 07 of 07.
- Previous effort: 06_cli_stream_output.md.
- Next effort: none.
- Dependency reason: this final slice depends on lifecycle repo display helpers, daemon event forwarding/timestamps/logging, and CLI stream rendering so the corrected worker message appears consistently across daemon logs and CLI streams.

## Target files

- `packages/worker/src/runtime.rs`: emit `repo_preparation` as `cloning repo: <redacted-repo-url>` before repo preparer execution.
- `packages/worker/src/event.rs`: reuse lifecycle redaction/safe rendering as needed for worker-emitted event text.
- `packages/worker/src/repo.rs`: preserve raw repository URL as the `git clone -- <repo> <repo-dir>` argument.
- `packages/worker/src/state.rs`: update only if a typed repo display/raw view is needed in runtime request models.
- `packages/worker/tests/unit/runtime.rs`: assert clone-start wording, redaction, event order before repo preparation, and lifecycle semantics preservation.
- `packages/worker/tests/unit/repo.rs`: assert raw repo URL remains the clone command argument.
- `packages/worker/tests/unit/event.rs`: update event redaction/sanitization tests if shared safe rendering is used.
- `packages/daemon/tests/integration/lifecycle_service.rs` and `packages/cli/tests/unit/lifecycle_output.rs`: update fixtures only where cross-package replay/stream expectations need the corrected clone wording.

Graph impact: worker runtime emits the source clone event, daemon session maps it to registry events, daemon service streams it with timestamps, CLI stream renderer displays it as one safe lifecycle line.

## Coupled files

- `packages/lifecycle/src/repo.rs` and `packages/lifecycle/src/terminal.rs`: shared display/raw repo and safe rendering contracts.
- `packages/daemon/src/worker_session.rs`: maps worker `repo_preparation` events and logs clone-start lifecycle lines.
- `packages/daemon/src/service.rs`: streams clone-start events with occurred-at timestamps.
- `packages/cli/src/main/lifecycle_output.rs`: displays clone activity line.

Graph impact: raw repo URL must travel only on the clone execution edge from daemon start job to worker repo preparer. Redacted repo display travels on the event/log/stream edges.

## Ownership

- Intended worker write scope: worker runtime/event/repo/state files as needed, worker unit tests, and narrowly scoped daemon/CLI fixture updates for clone wording regressions.
- Read-only context: lifecycle repo/terminal modules, daemon service/session, CLI stream output.
- Known conflict risks: do not change worker lifecycle states, prompt-agent flow, event capacity, input-answer behavior, or gRPC method names.

## Tests to add or update

- Runtime emits a `repo_preparation` event before repo preparer execution with message `cloning repo: <redacted-repo-url>`.
- Credential-bearing repo URLs are redacted in the clone-start event.
- Repo preparer still receives the raw URL, and `prepare_worker_repo` still builds `git clone -- <raw-url> <repo-dir>`.
- Repo preparation failure and prompt failure still redact secrets and send exactly one terminal status.
- Integration replay preserves milestone names and includes clone-start wording.
- CLI stream fixture shows clone activity as one timestamped `cloning repo: <repo-url>` line.
- Existing answer-input validation and worker terminal behavior continue to pass.

## Regression suites

- `cargo test -p worker runtime`
- `cargo test -p worker repo`
- `cargo test -p worker event`
- `cargo test -p daemon --test lifecycle_service`
- `cargo test -p daemon worker_session`
- `cargo test -p cli lifecycle_output`
- `cargo test -p lifecycle`

Graph impact: worker suites validate source event emission and raw clone execution, daemon suites validate forwarding/logging, CLI suite validates final display, lifecycle suite protects shared redaction/rendering.

## Acceptance criteria

- Worker clone-start lifecycle event text is `cloning repo: <redacted-repo-url>`.
- Raw repo URL remains scoped to clone execution and is not displayed in daemon logs, CLI streams, tables, or lifecycle lines when it contains credentials.
- Worker runtime still emits clone-start before repo preparer execution.
- Existing worker lifecycle semantics remain unchanged except for clone-start wording and safe display behavior.
- Repo preparation failures, prompt failures, success, stop, and input flows still send one terminal status where existing tests require it.
- Cross-package regression coverage proves the full path from worker event to daemon stream to CLI output remains line-oriented and safe.

## Notes

- This effort is the final regression slice. It should not add new CLI commands, daemon APIs, protobuf fields, durable logs, or local-path repository support.
