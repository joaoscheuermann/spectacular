# Effort: worker repo preparation

Status: todo

## Requirement links

- Features: F-06, F-08, F-11, F-12
- PRD: FR-8, FR-11, FR-12, FR-13; AC-7, AC-9, AC-12, AC-13
- TDD: root/repo handling, per-worker root layout, external Git repo preparation, repo clone mechanism Dependency Hop

## Goal

Implement worker-local root layout creation and repository preparation using an injected external `git` command runner.

## Sequence

- Position: 08 of 15
- Previous effort: 07_daemon_process_worker_session.md
- Enables: worker runtime can prepare the target repo before prompt-agent execution without live network dependencies in automated tests.

## Target files

- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/repo.rs`
- `packages/worker/src/state.rs`
- `packages/worker/src/error.rs`
- `packages/worker/tests/unit/repo.rs`
- `Cargo.lock`

## Coupled files

- `packages/lifecycle/src/redaction.rs` is used for redacted repo failure text.
- `packages/daemon/src/root.rs` owns daemon-side root validation, read-only here.
- `packages/tools/src/path.rs` and terminal tooling are read-only context for later worker tool root defaults.

## Ownership

- Intended worker write scope: worker repo/state/error modules, worker repo tests, and worker manifest updates.
- Read-only context: lifecycle redaction, daemon root convention, and TDD repo/root layout.
- Known conflict risks: worker root layout names are consumed by tooling, prompt artifacts, and integration tests. Keep `repo/`, `state/`, `artifacts/`, and `tool-output/` stable.

## Tests to add or update

- Repo layout tests for per-worker `repo/`, `state/`, `artifacts/`, and `tool-output/` directory creation.
- External Git command construction tests using a fake `GitCommandRunner`.
- Missing Git, nonzero exit, timeout, cancellation, invalid root, and clone target conflict tests.
- Redaction tests for clone stderr and credential-bearing repo URLs.

## Regression suites

- `cargo test -p worker --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`
- `npx nx run worker:test`

## Acceptance criteria

- Worker repo preparation uses the external `git` CLI through an injected command runner, not `git2` or another Rust Git library.
- Automated tests do not require a network repo.
- Failure variants are redacted and mappable to lifecycle failure events.
- The worker writes under the assigned worker root and never mutates the user's source checkout for test fixtures.
- The per-worker directory layout matches the TDD exactly.

## Notes

- Preserve the repaired TDD decision: external Git is the v1 implementation boundary.
- This effort does not register tools or run the prompt agent.
