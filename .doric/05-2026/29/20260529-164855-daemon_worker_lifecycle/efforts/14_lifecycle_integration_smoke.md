# Effort: lifecycle integration smoke

Status: todo

## Requirement links

- Features: F-01 through F-11, with validation support from F-12
- PRD: AC-1 through AC-15
- TDD: integration tests, end-to-end smoke, validation commands, fake daemon/worker/repo/prompt seams

## Goal

Add focused integration coverage that proves the daemon, worker, lifecycle contract, and CLI client work together through fakeable seams without live LLM or network repo dependencies.

## Sequence

- Position: 14 of 15
- Previous effort: 13_cli_daemon_client_output.md
- Enables: final documentation and workspace validation can rely on behavior evidence rather than package-local assumptions.

## Target files

- `packages/cli/tests/lifecycle_smoke.rs`
- `packages/daemon/tests/integration/lifecycle_service.rs`
- `packages/worker/tests/integration/runtime_session.rs`
- `packages/lifecycle/tests/unit/proto_contract.rs`
- `packages/daemon/src/service.rs`
- `packages/worker/src/runtime.rs`
- `packages/cli/src/main/lifecycle.rs`

## Coupled files

- Package-local unit tests from efforts 02 through 13 are regression context.
- `packages/daemon/src/process.rs` and `worker_session.rs` are coupled through worker attach behavior.
- `packages/worker/src/repo.rs`, `provider.rs`, `tooling.rs`, and `agents/prompt.rs` are coupled through fake runtime seams.

## Ownership

- Intended worker write scope: integration tests and only the small product-code fixes required to make those tests pass.
- Read-only context: package-local unit tests and source modules from earlier efforts.
- Known conflict risks: this effort can expose earlier design gaps across package boundaries. If a fix requires changing public lifecycle contracts, stop and report a decomposition gap rather than quietly expanding scope.

## Tests to add or update

- In-process daemon service with fake worker session proves feature dispatch, list, stream replay, input request, answer, continuation, and terminal success.
- CLI client smoke against an in-process daemon or test server proves output and error mapping.
- Worker runtime smoke with fake repo preparer and fake prompt runner proves attach/start/input/terminal event flow.
- Failure smoke for repo failure or worker crash visible in list and stream.
- Restart/untracked smoke proving stale worker ids are not reported live after daemon registry reset.

## Regression suites

- `cargo test -p lifecycle --no-fail-fast`
- `cargo test -p daemon --no-fail-fast`
- `cargo test -p worker --no-fail-fast`
- `cargo test -p cli --no-fail-fast`
- `cargo build -p cli --bin doric`
- `cargo build -p daemon --bin doric-daemon`
- `cargo build -p worker --bin doric-worker`

## Acceptance criteria

- AC-1 through AC-15 have direct package-local or integration-test evidence.
- Integration tests use fake repo, fake prompt/provider, and fake worker seams where needed; no live network repo or live LLM is required.
- Stream replay begins at sequence `0`, includes retained lifecycle milestones, and reports truncation when appropriate.
- Human input can be observed, answered through daemon mediation, and resumed.
- Failure states are visible through both list and stream.

## Notes

- This is a test-and-glue slice, not a second implementation pass.
- If integration exposes a need for large rewrites in earlier packages, stop at the failing boundary and request re-decomposition.
