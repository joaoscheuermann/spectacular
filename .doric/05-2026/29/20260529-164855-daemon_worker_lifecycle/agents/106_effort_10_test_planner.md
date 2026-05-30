# Agent Receipt: effort 10 test planner

## Spawn proof
- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7709-82e7-7c43-95f4-5838f2ea1ebd
- Spawn result: spawned
- Required agents row: `development | efforts/10_worker_tooling_registration.md | test planner | worker | agents/106_effort_10_test_planner.md`

## Role

Development test-planner sub-agent for effort 10. This role translates the accepted worker tooling registration criteria into concrete tests, focused red/green commands, and review assertions. This receipt is planning-only and intentionally does not edit production source or test files.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.agents/skills/coding-conventions/references/monodon-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/10_worker_tooling_registration.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `packages/worker/src/repo.rs`
- `packages/worker/src/lib.rs`
- `packages/worker/Cargo.toml`
- `packages/tools/src/lib.rs`
- `packages/tools/src/path.rs`
- `packages/tools/src/write.rs`
- `packages/tools/src/terminal.rs`
- `packages/cli/src/chat/runner.rs`
- `packages/worker/tests/unit.rs`
- `packages/worker/tests/unit/repo.rs`
- `packages/worker/tests/unit/provider.rs`
- `packages/tools/tests/unit/lib.rs`
- `packages/tools/tests/unit/path.rs`
- `packages/tools/tests/unit/write.rs`
- `packages/tools/tests/unit/terminal/mod.rs`
- `packages/tools/tests/unit/terminal/execution_contracts.rs`
- `packages/cli/tests/unit/chat/runner.rs`

## Read ownership

- Effort state and design context:
  - `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
  - `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/10_worker_tooling_registration.md`
  - `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- Worker package context:
  - `packages/worker/Cargo.toml`
  - `packages/worker/src/lib.rs`
  - `packages/worker/src/repo.rs`
  - `packages/worker/tests/unit.rs`
  - `packages/worker/tests/unit/repo.rs`
  - `packages/worker/tests/unit/provider.rs`
- Shared tools package context:
  - `packages/tools/src/lib.rs`
  - `packages/tools/src/path.rs`
  - `packages/tools/src/write.rs`
  - `packages/tools/src/terminal.rs`
  - `packages/tools/tests/unit/lib.rs`
  - `packages/tools/tests/unit/path.rs`
  - `packages/tools/tests/unit/write.rs`
  - `packages/tools/tests/unit/terminal/mod.rs`
  - `packages/tools/tests/unit/terminal/execution_contracts.rs`
- Chat read-only compatibility context:
  - `packages/cli/src/chat/runner.rs`
  - `packages/cli/tests/unit/chat/runner.rs`
- Local worktree context:
  - `git status --short`

## Write ownership

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/106_effort_10_test_planner.md` only.

## Coding conventions

- `SKILL.md`: Applied repository invariants for scoped work, behavior-oriented tests, explicit dependency boundaries, no speculative abstractions, and no Rust-only guidance beyond Rust files. For effort 10, the worker should become a second consumer of `packages/tools` rather than moving tool ownership into `worker`.
- `references/implementation-standards.md`: Applied package-root `tests/` placement, no inline production tests except existing include harnesses, Red-Green-Refactor, F.I.R.S.T., Arrange-Act-Assert, public-contract testing, and explicit dependency injection. New worker tests should live under `packages/worker/tests/unit/tooling.rs` and be included by `packages/worker/tests/unit.rs`; narrow shared-tool regressions should remain under `packages/tools/tests/unit/...`.
- `references/sexy-rust.md`: Applied Rust guidance for a small typed worker tooling boundary, flat `Result` propagation from `ToolStorage` registration, natural `Path`/`PathBuf` conversions, and tests that assert behavior through public APIs rather than private fields.
- `references/monodon-rust.md`: Applied Rust/Nx target expectations. Focused validation should use Cargo package gates first, with `worker:test`, `tools:test`, and `cli:test` as Nx equivalents when local Nx tooling is available.

## Prompt summary

Plan tests for effort 10 worker tooling registration. The worker must depend on shared `tools`, register the existing built-ins by calling `tools::built_in_tools_with_trace_dir(repo_dir, tool_output_dir)` using the prepared worker layout's `repo/` as workspace root/default and `tool-output/` as terminal trace storage, preserve current shared path semantics for relative, absolute, and `..` paths, and keep existing chat tool registration unchanged. The plan must not claim sandboxing, containment, path confinement, or scoped tools.

## Planned tests

### Worker test file and harness

- Add `packages/worker/tests/unit/tooling.rs`.
- Update `packages/worker/tests/unit.rs` with `#[path = "unit/tooling.rs"] mod tooling;`.
- Keep helpers local to `tooling.rs` unless a later effort needs them.
- Test through public worker APIs and shared `agent::ToolStorage` manifests/execution. Avoid private-field assertions.

### Worker public API shape to drive from tests

The first test should drive a small worker-owned module rather than tool reimplementation:

- `packages/worker/src/lib.rs` should expose `pub mod tooling;`.
- `packages/worker/src/tooling.rs` should expose a public function such as:

```rust
pub fn worker_tool_storage(layout: &worker::repo::WorkerLayout) -> Result<agent::ToolStorage, agent::ToolRegistrationError>
```

or an equivalent function accepting `repo_dir` and `tool_output_dir` explicitly. The key contract is that production code passes `layout.repo()` and `layout.tool_output()` to `tools::built_in_tools_with_trace_dir`.

### Worker tooling tests

- `worker_tool_storage_prepared_layout_registers_shared_built_ins`
  - Arrange a temp worker root and call `prepare_worker_layout`.
  - Act by creating worker tool storage through the new worker tooling API.
  - Assert provider-visible manifest order matches the shared built-ins: `edit`, `find`, `grep`, `terminal`, `tree`, `web_search`, `write`.
  - Assert `packages/worker/Cargo.toml` has a `tools = { path = "../tools" }` dependency and does not include copied tool modules.

- `worker_tool_storage_relative_write_defaults_to_prepared_repo_root`
  - Arrange a prepared layout and worker tool storage.
  - Act through the registered `write` tool with `{"path":"notes/worker.txt","content":"worker-root"}`.
  - Assert the file is written at `<worker-root>/<id>/repo/notes/worker.txt`.
  - Assert no same relative file is created under `<worker-root>/<id>/state`, `artifacts`, or `tool-output`.

- `worker_tool_storage_terminal_missing_working_directory_defaults_to_prepared_repo_root_and_traces_to_tool_output`
  - Arrange prepared layout and worker tool storage.
  - Act through the registered `terminal` tool with a command that creates `terminal-root.txt` and no `working_directory`.
  - Assert the file is created under the prepared `repo/` directory.
  - Assert the compact terminal output includes a `raw_output_ref`.
  - Assert the referenced trace file is under the prepared `tool-output/` directory and contains the raw command output.

- `worker_tool_storage_terminal_relative_working_directory_resolves_under_prepared_repo_root`
  - Arrange `repo/nested/`.
  - Act through the terminal tool with `working_directory = "nested"` and a command that creates a file.
  - Assert the file lands under `repo/nested/`, proving the workspace root default came from the worker repo root.

- `worker_tool_storage_uses_shared_tools_without_sandbox_claims`
  - Assert the worker tooling module and test names describe repo defaults and trace storage, not confinement.
  - Prefer a simple source-text assertion only if needed to prevent accidental terms in worker tooling user-visible strings. Do not add user-visible text that claims sandboxing, containment, path confinement, or scoped tools.

### Shared tools regression tests

Add only narrow tests that preserve current semantics in `packages/tools/tests/unit/...`; do not change shared tools behavior.

- `resolve_workspace_path_parent_traversal_can_escape_workspace_root`
  - In `packages/tools/tests/unit/path.rs`, assert `resolve_workspace_path("/workspace/repo", "../outside.txt")` resolves lexically to `/workspace/outside.txt` on Unix-style paths, with a Windows-specific equivalent if needed.
  - This locks the TDD decision that `..` traversal remains allowed intentionally.

- `write_tool_absolute_path_writes_to_absolute_path`
  - In `packages/tools/tests/unit/write.rs`, create a temp directory and pass an absolute path inside that temp directory.
  - Assert `WriteTool::new(workspace_root)` writes to the absolute path, not under `workspace_root`.
  - Keep the absolute path inside the temp root so the test is deterministic and does not touch the user's checkout.

- `write_tool_parent_traversal_writes_outside_workspace_root`
  - In `packages/tools/tests/unit/write.rs`, arrange a temp parent with `workspace/`.
  - Act with `path = "../outside-write.txt"`.
  - Assert the file is created next to `workspace/`, not inside it.

- `terminal_tool_parent_working_directory_can_escape_workspace_root`
  - In `packages/tools/tests/unit/terminal/execution_contracts.rs`, arrange a temp parent with `workspace/` and `outside/`.
  - Act with `working_directory = "../outside"` and a command that creates a file.
  - Assert the file is created in `outside/`.
  - This should be a focused regression that current terminal resolution remains lexical/defaulting behavior, not confinement.

### Chat registration assertion

- Do not edit `packages/cli/src/chat/runner.rs` for effort 10.
- Use the existing `main_chat_tool_storage(workspace_root, trace_dir)` path as the compatibility proof: it already delegates to `tools::built_in_tools_with_trace_dir(workspace_root, trace_dir)`.
- Run the existing CLI test `main_chat_agent_gets_built_in_tools` as a review/test assertion that chat still sees the same built-in manifest order.
- Reviewer should compare the pre/post diff for `packages/cli/src/chat/runner.rs`; expected diff is no source change.

## Commands

Focused red command before implementation:

```text
cargo test -p worker tooling --no-fail-fast
```

Expected initial failure mode:

- Before tests exist, the test writer should first add `packages/worker/tests/unit/tooling.rs` and include it from `packages/worker/tests/unit.rs`.
- With the first test in place, the red should fail at compile time because `worker::tooling` and the worker `tools` dependency do not exist yet, or fail an assertion proving worker storage is not yet registered through the prepared repo/tool-output layout.
- The red must not fail because of a live network call, live provider call, environment credential lookup, or mutation of the user's checkout.

Focused worker green commands:

```text
cargo test -p worker tooling --no-fail-fast
cargo test -p worker --no-fail-fast
cargo clippy -p worker --all-targets -- -D warnings
```

Shared tools regression commands from the effort:

```text
cargo test -p tools --no-fail-fast
cargo clippy -p tools --all-targets -- -D warnings
```

Chat compatibility command from the effort:

```text
cargo test -p cli main_chat_agent_gets_built_in_tools --no-fail-fast
cargo test -p cli --no-fail-fast
```

Workspace hygiene commands:

```text
cargo fmt --all -- --check
cargo metadata --format-version 1 --no-deps
```

Nx equivalents when local Nx dependencies are available:

```text
npx nx run worker:test
npx nx run worker:lint
npx nx run tools:test
npx nx run cli:test
```

Review assertions:

```text
git diff -- packages/cli/src/chat/runner.rs
git diff -- packages/tools/src packages/worker/src packages/worker/tests packages/tools/tests
```

Expected review results:

- `packages/cli/src/chat/runner.rs` has no diff.
- Worker code adds a `tools` dependency and a worker-local registration seam.
- Shared tools diffs are limited to regression tests unless the test writer discovers a blocking public-helper need.
- No source or tests claim sandboxing, containment, path confinement, or scoped tools.

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/106_effort_10_test_planner.md`

## Blocking questions

- None for test planning.

## Coordinator decision

accepted
