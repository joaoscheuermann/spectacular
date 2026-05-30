# Effort: worker tooling registration

Status: todo

## Requirement links

- Features: F-08, F-11, F-12
- PRD: FR-10, FR-11, FR-12; AC-10, AC-12, AC-13
- TDD: tool ownership graph, shared tools registration, security/privacy, rollout step 10

## Goal

Register the existing shared `packages/tools` built-ins for worker execution with the cloned repo as default workspace root and `tool-output/` as trace storage, while preserving current chat tool behavior.

## Sequence

- Position: 10 of 15
- Previous effort: 09_worker_provider_runtime.md
- Enables: prompt-agent runner can use the existing tool set under the worker root without moving tools into `worker`.

## Target files

- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/tooling.rs`
- `packages/worker/tests/unit/tooling.rs`
- `packages/tools/tests/unit/path.rs`
- `packages/tools/tests/unit/write.rs`
- `packages/tools/tests/unit/terminal/mod.rs`
- `Cargo.lock`

## Coupled files

- `packages/tools/src/lib.rs`, `path.rs`, `write.rs`, and `terminal.rs` are shared tool implementation context.
- `packages/cli/src/chat/runner.rs` is read-only context proving chat still registers shared tools.
- `packages/worker/src/repo.rs` supplies `repo/` and `tool-output/` paths.

## Ownership

- Intended worker write scope: worker tooling module/tests and narrow tools tests that preserve existing semantics.
- Read-only context: current chat runner registration and tools implementation files unless a test requires a minimal public helper.
- Known conflict risks: changing `packages/tools` semantics can break chat. Prefer adding worker-side registration and regression tests over changing tool behavior.

## Tests to add or update

- Worker tooling tests prove `tools::built_in_tools_with_trace_dir(repo_dir, tool_output_dir)` is used.
- Tests prove relative paths default to the worker repo root.
- Tools regression tests preserve current absolute path and `..` traversal behavior.
- Chat registration remains unchanged in tests or review assertions.

## Regression suites

- `cargo test -p worker --no-fail-fast`
- `cargo test -p tools --no-fail-fast`
- `cargo test -p cli --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`
- `cargo clippy -p tools --all-targets -- -D warnings`

## Acceptance criteria

- Worker depends on shared `tools` and does not move existing tool modules into `worker`.
- Worker tools use the cloned repo as workspace root/default and `tool-output/` as terminal trace storage.
- Existing chat tool registration remains untouched.
- No test or user-visible text claims sandboxing, containment, path confinement, or scoped tools.
- Current shared-tool path semantics remain preserved.

## Notes

- Preserve the repaired TDD decision: shared tools are not confined, and `packages/tools` remains the canonical owner.
- Docker or future scoped tooling is out of scope for this effort.
