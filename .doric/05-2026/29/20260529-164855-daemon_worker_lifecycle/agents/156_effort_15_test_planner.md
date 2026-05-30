# Agent Receipt: Effort 15 Test Planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7878-e409-77d1-9572-86a50dec8e64
- Spawn result: spawned
- Required agents row: `development | efforts/15_architecture_docs_validation.md | test planner | worker | agents/156_effort_15_test_planner.md | 019e7878-e409-77d1-9572-86a50dec8e64 | spawned`

## Role

Effort 15 test planner. This receipt plans documentation validation and the final command matrix only. It does not edit architecture docs, validation records, source, or manifests.

## Input artifacts

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/architecture-principles.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/current-architecture/SKILL.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/15_architecture_docs_validation.md`
- `docs/architecture-and-packages.md`
- Earlier validation records read for context: `validation/13_cli_daemon_client_output.md` and `validation/14_lifecycle_integration_smoke.md`

## Read ownership

Read evidence included:

- Workspace manifests: `Cargo.toml`, `package.json`, `packages/*/Cargo.toml`, `packages/*/project.json`
- Lifecycle source boundary: `packages/lifecycle/src/*`, `packages/lifecycle/build.rs`
- Daemon source boundary: `packages/daemon/src/*`
- Worker source boundary: `packages/worker/src/*`, including `agents/prompt.rs`, `provider.rs`, `repo.rs`, `runtime.rs`, and `tooling.rs`
- CLI lifecycle/chat boundary: `packages/cli/src/main/entry.rs`, `cli_types.rs`, `lifecycle.rs`, `lifecycle_client.rs`, and `lifecycle_output.rs`
- Shared dependency boundaries: `packages/tools/src/lib.rs`, plus relevant `config`, `llms`, and `agent` manifest/source references

## Write ownership

Only this receipt was written:

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/156_effort_15_test_planner.md`

## Coding conventions

Loaded `.agents/skills/coding-conventions/SKILL.md`.

References used:

- `references/architecture-principles.md`: dependency direction, SRP, deep modules, reuse before abstraction.
- `references/implementation-standards.md`: validation and test-location expectations, especially that no new behavior tests are expected for a documentation-only update.

Applied conventions:

- Treat this as architecture/documentation validation, not Rust implementation.
- Keep validation evidence tied to observed package manifests and public module boundaries.
- Do not add speculative shared abstractions or claims beyond the implemented graph.

## Current architecture skill

Loaded `.agents/skills/current-architecture/SKILL.md`.

Applied rules:

- `docs/architecture-and-packages.md` is the repo-level architecture map.
- Code and manifests are authoritative when they disagree with docs.
- Architecture docs must describe observed implementation in current tense.
- The doc writer should edit the smallest affected sections and avoid future-intent wording.
- Documentation must preserve package responsibilities and dependency direction.

## Prompt summary

Plan the documentation validation and final command matrix for effort 15. Identify what evidence the documentation writer must inspect before editing, whether red evidence should be a baseline doc gap audit, what final validation commands to run, likely local tool risks, and focused pre/post commands for the doc writer.

## Test and validation plan

No new behavior tests are expected for effort 15. The validation plan should be documentation-first:

1. Establish baseline doc gaps against the current implementation graph.
2. Update `docs/architecture-and-packages.md` from observed manifests and source boundaries.
3. Record validation results in `validation/15_architecture_docs_validation.md`.
4. Run focused documentation checks first, then the full effort command matrix.
5. Distinguish product failures from local tooling failures, especially Nx availability or target execution issues.

The documentation writer must inspect these facts before editing docs:

- `Cargo.toml` workspace members now include `lifecycle`, `daemon`, and `worker`.
- `package.json` uses `packages/*` workspaces and Nx with `@monodon/rust`.
- `packages/lifecycle/Cargo.toml` is a library with `build.rs`; it uses `prost`, `prost-types`, `tonic`, `tonic-prost`, `tonic-prost-build`, and `protoc-bin-vendored`.
- `packages/daemon/Cargo.toml` defines `doric-daemon` and depends on `config`, `lifecycle`, `tokio`, `tokio-stream`, and `tonic`.
- `packages/worker/Cargo.toml` defines `doric-worker` and depends on `agent`, `config`, `lifecycle`, `llms`, and `tools`.
- `packages/cli/Cargo.toml` now depends on `daemon` and `lifecycle` in addition to the existing chat packages.
- `packages/lifecycle/src/lib.rs` exports `event`, `identity`, `proto`, `redaction`, `repo`, and `status`.
- `packages/daemon/src/lib.rs` exports `error`, `event`, `process`, `registry`, `root`, `server`, `service`, and `worker_session`.
- `packages/worker/src/lib.rs` exports `agents`, `error`, `event`, `provider`, `repo`, `runtime`, `state`, and `tooling`.
- `packages/cli/src/main/entry.rs` routes lifecycle/config commands before creating `LlmDebugLogger`; bare chat still creates the debug logger in the chat branch.
- `packages/cli/src/main/cli_types.rs` defines `daemon`, `feature`, `debug`, `list`, `worker`, and `answer`.
- `packages/cli/src/main/lifecycle_client.rs` uses the generated lifecycle gRPC client for dispatch, list, stream, and answer.
- `packages/tools/src/lib.rs` remains the shared built-in tool owner. `packages/worker/src/tooling.rs` calls `tools::built_in_tools_with_trace_dir(layout.repo(), layout.tool_output())`. This is repo-root defaulting and trace storage, not confinement.
- `packages/worker/src/provider.rs` owns worker-local provider/runtime composition from `config`, `llms`, and `agent`. Chat provider/runtime composition remains under `packages/cli/src/chat/*`.
- `packages/worker/src/repo.rs` uses an injected external `git` command runner and prepares `repo/`, `state/`, `artifacts/`, and `tool-output/`.
- `packages/daemon/src/root.rs` and `packages/worker/src/repo.rs` both expose worker-root layout evidence that docs should describe consistently.
- `packages/daemon/src/server.rs`, `service.rs`, and `worker_session.rs` show daemon-mediated lifecycle service and worker-session flow.
- Earlier validation records show effort 13 and 14 already exercised CLI daemon client output, daemon/lifecycle/worker/CLI package tests, binary builds, and the lifecycle stream milestone smoke.

The existing `docs/architecture-and-packages.md` baseline is stale: it lists only the pre-lifecycle workspace packages and old dependency/runtime flow. That gap is the red evidence for effort 15.

## Baseline evidence recommendation

Use a doc gap/baseline audit as red evidence rather than a failing behavior test.

Recommended red evidence shape:

- Command: `git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff -- docs/architecture-and-packages.md`
- Command: `Select-String -Path docs/architecture-and-packages.md -Pattern 'lifecycle|daemon|worker|tonic-prost|doric-daemon|doric-worker|worker-local|confinement|LlmDebugLogger|external Git'`
- Expected baseline result before the doc update: no or incomplete matches for the new lifecycle packages, binaries, dependency direction, current protobuf stack, external Git repo clone, worker-local provider/runtime composition, lifecycle debug-log bypass, and tools-not-confinement note.

This is acceptable red evidence because the effort explicitly says no behavior test is expected and manual documentation assertions should be updated from manifests and source imports.

## Commands

Focused pre-update evidence commands for the documentation writer:

```text
git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric status --short
cargo metadata --format-version 1 --no-deps
Select-String -Path docs/architecture-and-packages.md -Pattern 'lifecycle|daemon|worker|tonic-prost|doric-daemon|doric-worker|worker-local|confinement|LlmDebugLogger|external Git'
Get-Content Cargo.toml
Get-Content package.json
Get-ChildItem packages -Directory | ForEach-Object { $p = Join-Path $_.FullName 'Cargo.toml'; if (Test-Path $p) { Get-Content $p } }
Get-ChildItem packages -Directory | ForEach-Object { $p = Join-Path $_.FullName 'project.json'; if (Test-Path $p) { Get-Content $p } }
rg -n "pub mod|built_in_tools_with_trace_dir|LlmDebugLogger|tonic_prost_build|protoc_bin_vendored|OpenRouterProvider|OpenAiProvider|GitCommandRunner|doric-worker|doric-daemon|LifecycleService|WorkerSession" packages/lifecycle packages/daemon packages/worker packages/cli packages/tools
```

Focused post-update documentation checks:

```text
Select-String -Path docs/architecture-and-packages.md -Pattern 'lifecycle|daemon|worker|doric-daemon|doric-worker|tonic-prost-build|tonic-prost|prost|external Git|worker-local|not confinement|debug-log'
git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff -- docs/architecture-and-packages.md
git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff --check
cargo metadata --format-version 1 --no-deps
cargo fmt --all -- --check
```

Final effort regression matrix from `efforts/15_architecture_docs_validation.md`:

```text
cargo fmt --all -- --check
cargo metadata --format-version 1 --no-deps
cargo build -p lifecycle
cargo test -p lifecycle --no-fail-fast
cargo test -p daemon --no-fail-fast
cargo test -p tools --no-fail-fast
cargo test -p worker --no-fail-fast
cargo test -p cli --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p cli --bin doric
cargo build -p daemon --bin doric-daemon
cargo build -p worker --bin doric-worker
npx nx run lifecycle:build
npx nx run lifecycle:test
npx nx run daemon:build
npx nx run daemon:test
npx nx run tools:test
npx nx run worker:build
npx nx run worker:test
npx nx run cli:build
npx nx run cli:test
```

Likely local tool risks:

- Nx commands require `node_modules` and the local `nx`/`@monodon/rust` toolchain to be available.
- Nx may take extra time because each target uses its own `dist/target/<project>` target dir.
- Cargo workspace tests and clippy may be time-expensive because this is the final full matrix.
- Windows file locks can affect Cargo target dirs; if that appears, record the lock/tool failure and use an isolated `CARGO_TARGET_DIR` for reruns when allowed by the coordinator.
- `cargo build -p lifecycle` and `npx nx run lifecycle:build` are codegen gates and may fail if protobuf build tooling or generated `tonic_prost` wiring is broken.
- `cargo clippy --workspace --all-targets -- -D warnings` is the broadest product-quality gate and should be treated as product failure unless the failure is clearly a local tool/environment issue.

## Files changed

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/156_effort_15_test_planner.md`

## Blocking questions

None for planning.

Operational note for the doc writer/coordinator: this checkout already had unrelated dirty files before this receipt was written, including skill files, run state files, `PROMPT.md`, and effort 15 transition artifacts. Preserve them and scope doc edits carefully.

## Coordinator decision

Coordinator decision: accepted.
