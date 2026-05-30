# Validation: worker repo preparation

## Red evidence

- Command: `cargo test -p worker --no-fail-fast prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command`
- Exit code: `1`
- Failure summary:
  - `error[E0432]: unresolved import worker::error`
  - `error[E0432]: unresolved import worker::repo`
  - `error[E0432]: unresolved import worker::state`
- Acceptance mapping: the worker crate is still a skeleton, so the focused red fails for the expected missing public API required by effort 08 rather than a harness, syntax, network, or fixture failure.

## Green evidence

- Command: `cargo test -p worker --no-fail-fast prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command`
  - Exit code: `0`
  - Summary: focused command-construction test passed; 1 integration test passed with 15 filtered out after implementation and repair.
- Command: `cargo test -p worker --no-fail-fast`
  - Exit code: `0`
  - Summary: full worker package tests passed; 16 integration tests passed plus empty lib/main/doc test targets.
- Command: `cargo test -p lifecycle --no-fail-fast`
  - Exit code: `0`
  - Summary: lifecycle regression passed; 18 unit tests passed plus empty doc tests.
- Command: `cargo clippy -p worker --all-targets -- -D warnings`
  - Exit code: `0`
  - Summary: worker lib, bin, and test targets passed clippy with warnings denied.
- Command: `npx nx run worker:test`
  - Exit code: `0`
  - Summary: Nx worker test target passed; Cargo reported 16 worker integration tests passed. Nx emitted a Node experimental warning from npm debug/supports-color, but the target succeeded.
- Command: `cargo fmt --all -- --check`
  - Exit code: `0`
  - Summary: workspace Rust formatting check passed.

## Focused commands

- `cargo test -p worker --no-fail-fast prepare_worker_repo_uses_injected_git_runner_constructs_external_clone_command`
- `cargo test -p worker --no-fail-fast prepare_worker_layout_path_like_worker_id_returns_invalid_worker_root`

## Regression commands

- `cargo test -p worker --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`
- `npx nx run worker:test`
- `cargo fmt --all -- --check`

## Unavailable tooling

None.

## Refactors applied

No validator/refactor changes were needed. A code-review repair added root-escape validation before validator/refactor.

## Reviewer decision

Approved in `agents/095_effort_08_reviewer.md`; no blocking issues found.
