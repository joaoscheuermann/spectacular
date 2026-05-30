# Validation: Effort 15 Architecture Docs Baseline

## Scope

Effort 15 is documentation-baseline validation for stale architecture documentation after the daemon/worker lifecycle package graph landed. No behavior test is expected for this documentation-only effort.

## Skills Loaded

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/current-architecture/SKILL.md`

References used:

- Coding conventions top-level testing and architecture guidance: this effort should stay documentation-first and not add behavior tests.
- Current architecture top-level source-of-truth guidance: `docs/architecture-and-packages.md` is the repo map, but manifests and source are authoritative when they disagree.

## Baseline Commands

### Documentation gap scan

Command:

```powershell
Select-String -Path docs/architecture-and-packages.md -Pattern 'lifecycle|daemon|worker|tonic-prost|doric-daemon|doric-worker|worker-local|confinement|LlmDebugLogger|external Git'
```

Result: red baseline.

Observed matches:

- `docs/architecture-and-packages.md:14` mentions agent request lifecycle in the old agent responsibility row.
- `docs/architecture-and-packages.md:78` mentions command lifecycle events in persisted chat session records.

Observed gap:

- No package rows for `lifecycle`, `daemon`, or `worker`.
- No `doric-daemon` or `doric-worker` binary documentation.
- No `tonic-prost-build`/`tonic-prost`/`prost` protobuf stack documentation.
- No worker-local provider/runtime composition note.
- No statement that `packages/tools` remains shared and worker registration is not confinement.
- No lifecycle command debug-log bypass note.
- No external Git clone note.

### Cargo workspace metadata

Command:

```powershell
cargo metadata --format-version 1 --no-deps
```

Result: pass.

Relevant evidence from metadata:

- Workspace members include `cli`, `agent`, `llms`, `commands`, `config`, `daemon`, `lifecycle`, `tools`, `tui`, and `worker`.
- `cli` has the `doric` binary and depends on `daemon` and `lifecycle` in addition to the existing chat packages.
- `daemon` has the `doric-daemon` binary and depends on `config`, `lifecycle`, `tokio`, `tokio-stream`, and `tonic`.
- `lifecycle` is a library with `build.rs` and depends on `prost`, `prost-types`, `tonic`, and `tonic-prost`; build dependencies are `protoc-bin-vendored` and `tonic-prost-build`.
- `worker` has the `doric-worker` binary and depends on `agent`, `config`, `lifecycle`, `llms`, and `tools`.

### Manifest and source package graph scan

Commands:

```powershell
Get-ChildItem packages -Directory | ForEach-Object {
  $cargo = Join-Path $_.FullName 'Cargo.toml'
  $project = Join-Path $_.FullName 'project.json'
  if (Test-Path $cargo) {
    Select-String -Path $cargo -Pattern '^name\s*=|^build\s*=|^\[\[bin\]\]|tonic-prost|tonic_prost|prost|protoc-bin-vendored|agent|config|lifecycle|llms|tools|daemon|worker'
  }
  if (Test-Path $project) {
    Select-String -Path $project -Pattern 'name|targets|build|test|daemon|worker|lifecycle|doric-daemon|doric-worker'
  }
}

rg -n 'pub mod|built_in_tools_with_trace_dir|LlmDebugLogger|GitCommandRunner|Command::new\("git"\)|LifecycleService|WorkerSession|OpenAiProvider|OpenRouterProvider|doric-worker|doric-daemon|tonic_prost_build|protoc_bin_vendored' packages/lifecycle packages/daemon packages/worker packages/cli packages/tools
```

Result: pass.

Relevant evidence:

- `Cargo.toml` includes `packages/lifecycle`, `packages/daemon`, and `packages/worker` as workspace members.
- `packages/lifecycle/Cargo.toml` uses `build = "build.rs"`, `prost`, `prost-types`, `tonic`, `tonic-prost`, `protoc-bin-vendored`, and `tonic-prost-build`.
- `packages/lifecycle/build.rs` calls `protoc_bin_vendored::protoc_bin_path()` and `tonic_prost_build::configure()`.
- `packages/daemon/Cargo.toml` defines `doric-daemon`; `packages/worker/Cargo.toml` defines `doric-worker`.
- `packages/lifecycle/src/lib.rs` exports `event`, `identity`, `proto`, `redaction`, `repo`, and `status`.
- `packages/daemon/src/lib.rs` exports `error`, `event`, `process`, `registry`, `root`, `server`, `service`, and `worker_session`.
- `packages/worker/src/lib.rs` exports `agents`, `error`, `event`, `provider`, `repo`, `runtime`, `state`, and `tooling`.
- `packages/daemon/src/process.rs` references the `doric-worker` binary name.
- `packages/daemon/src/server.rs` exposes daemon-side `LifecycleService` and worker session setup.
- `packages/cli/src/main/lifecycle_client.rs` uses the generated lifecycle gRPC client.
- `packages/cli/src/main/entry.rs` routes `config`, `daemon`, and lifecycle commands before creating `LlmDebugLogger`; bare chat still creates the debug logger before `chat::run`.
- `packages/worker/src/provider.rs` composes worker-local OpenAI/OpenRouter providers and `LlmDebugLogger`.
- `packages/worker/src/repo.rs` builds an external `git clone -- <repo_url> <repo_dir>` command through the injectable `GitCommandRunner`.
- `packages/daemon/src/root.rs` and `packages/worker/src/repo.rs` both prepare per-worker `repo`, `state`, `artifacts`, and `tool-output` directories.
- `packages/tools/src/lib.rs` remains the shared built-in tool owner; `packages/worker/src/tooling.rs` calls `tools::built_in_tools_with_trace_dir(layout.repo(), layout.tool_output())`.

## Acceptance Criteria Mapping

This is red/baseline evidence for effort 15 because the current architecture document does not list the implemented `lifecycle`, `daemon`, and `worker` packages or their dependency direction, while the workspace manifests and source do.

It also maps directly to the effort 15 acceptance criteria:

- The stale doc lacks accurate responsibilities for `lifecycle`, `daemon`, and `worker`.
- The stale doc lacks the shared-tools-not-confinement distinction.
- The stale doc lacks the worker-local provider/runtime composition note.
- The stale doc lacks the lifecycle debug-log bypass distinction from bare chat.
- The stale doc lacks the current protobuf stack and external Git clone detail.

This does not claim product failure. The code and manifests establish the implemented graph; the failure is that `docs/architecture-and-packages.md` has not caught up yet.

## Tooling Notes

- `cargo metadata --format-version 1 --no-deps` completed successfully.
- One initial `rg` attempt used PowerShell-unfriendly glob/quoting and failed before producing evidence. The focused manifest/source scans above were rerun successfully with PowerShell-native file enumeration and a correctly quoted `rg` pattern.

## Green Documentation Evidence

Effort 15 code writer updated `docs/architecture-and-packages.md` from the observed manifests and source boundaries.

Commands run after the documentation update:

```powershell
Select-String -Path docs/architecture-and-packages.md -Pattern 'lifecycle|daemon|worker|doric-daemon|doric-worker|tonic-prost-build|tonic-prost|prost|external Git|worker-local|not confinement|debug-log'
```

Result: pass. The updated architecture document now contains matches for the new `lifecycle`, `daemon`, and `worker` package rows, `doric-daemon`, `doric-worker`, the `tonic-prost-build`/`tonic-prost`/`prost` stack, external Git repo preparation, worker-local provider/runtime composition, shared worker tool registration, the not confinement note, and the lifecycle debug-log bypass.

```powershell
git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff -- docs/architecture-and-packages.md
```

Result: pass. The diff is limited to `docs/architecture-and-packages.md` and updates the workspace package table, dependency direction, runtime flow, lifecycle debug-log behavior, worker repo preparation, provider composition, and shared tool registration notes.

```powershell
git -c safe.directory=C:/Users/jvito/Documents/git/spectacular/doric diff --check -- docs/architecture-and-packages.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/158_effort_15_code_writer.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/15_architecture_docs_validation.md
```

Result: pass. Git reported only the existing line-ending normalization warning for `docs/architecture-and-packages.md`; no whitespace errors were reported.

## Validator/Refactor Final Validation

Effort 15 validator/refactor reloaded the required local skills, read the effort file, checked the `STATE.md` effort and required-agent rows, and reviewed receipts 156, 157, and 158 before running the final matrix.

Documentation validation result: pass.

- `docs/architecture-and-packages.md` lists `lifecycle`, `daemon`, and `worker` with responsibilities matching the workspace manifests and source boundaries.
- The dependency direction matches `Cargo.toml`, package manifests, and the observed source graph: `cli` composes the application, `lifecycle` is the shared contract below `cli`/`daemon`/`worker`, `daemon` launches `doric-worker` as a process instead of depending on the `worker` crate, and `worker` depends on `agent`, `config`, `lifecycle`, `llms`, and `tools`.
- The document states that `packages/tools` remains shared and worker registration is not confinement.
- The document states that worker provider/runtime composition is worker-local in v1 and chat composition remains in `cli`.
- The document states that lifecycle commands bypass chat debug-log startup while bare chat still creates the provider debug logger.
- The document states that v1 uses `tonic-prost-build`/`tonic-prost`/`prost` and external Git for worker repo clone.
- No concrete documentation bug was found, so `docs/architecture-and-packages.md` was not changed by the validator/refactor.

Source/manifests checked:

- `Cargo.toml`, `package.json`, and package `project.json` files.
- `packages/lifecycle/Cargo.toml`, `packages/daemon/Cargo.toml`, `packages/worker/Cargo.toml`, and `packages/cli/Cargo.toml`.
- Focused source boundaries in `packages/lifecycle/build.rs`, `packages/lifecycle/src/lib.rs`, `packages/daemon/src/lib.rs`, `packages/daemon/src/process.rs`, `packages/daemon/src/root.rs`, `packages/daemon/src/server.rs`, `packages/worker/src/lib.rs`, `packages/worker/src/provider.rs`, `packages/worker/src/repo.rs`, `packages/worker/src/tooling.rs`, `packages/cli/src/main/entry.rs`, and `packages/tools/src/lib.rs`.

Final validation matrix:

| Command | Result | Evidence |
| ------- | ------ | -------- |
| `cargo fmt --all -- --check` | pass | Exit 0. |
| `cargo metadata --format-version 1 --no-deps` | pass | Exit 0; workspace members include `cli`, `agent`, `llms`, `commands`, `config`, `daemon`, `lifecycle`, `tools`, `tui`, and `worker`. |
| `cargo build -p lifecycle` | pass | Exit 0; finished dev build. |
| `cargo test -p lifecycle --no-fail-fast` | pass | Exit 0; 19 unit tests plus doc tests passed. |
| `cargo test -p daemon --no-fail-fast` | pass | Exit 0; 59 unit tests, 1 integration test, and doc tests passed. |
| `cargo test -p tools --no-fail-fast` | pass | Exit 0; 60 unit tests plus doc tests passed. |
| `cargo test -p worker --no-fail-fast` | pass | Exit 0; 59 integration/unit tests plus doc tests passed. |
| `cargo test -p cli --no-fail-fast` | pass | Exit 0; 181 unit tests and 2 debug-log startup process tests passed. Cargo emitted the expected temporary Git CRLF warning from a test fixture only. |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass | Exit 0; workspace checked with warnings denied. |
| `cargo build -p cli --bin doric` | pass | Exit 0; finished dev build. |
| `cargo build -p daemon --bin doric-daemon` | pass | Exit 0; finished dev build. |
| `cargo build -p worker --bin doric-worker` | pass | Exit 0; finished dev build. |
| `npx nx run lifecycle:build` | pass | Exit 0; Nx ran `cargo check --target-dir dist/target/lifecycle -p lifecycle`. |
| `npx nx run lifecycle:test` | pass | Exit 0; Nx ran lifecycle tests, 19 unit tests plus doc tests passed. |
| `npx nx run daemon:build` | pass | Exit 0; Nx ran `cargo check --target-dir dist/target/daemon -p daemon`. |
| `npx nx run daemon:test` | pass | Exit 0; Nx ran daemon tests, 59 unit tests and 1 integration test passed. |
| `npx nx run tools:test` | pass | Exit 0; Nx ran tools tests, 60 unit tests plus doc tests passed. |
| `npx nx run worker:build` | pass | Exit 0; Nx ran `cargo check --target-dir dist/target/worker -p worker`. |
| `npx nx run worker:test` | pass | Exit 0; Nx ran worker tests, 59 tests plus doc tests passed. |
| `npx nx run cli:build` | pass | Exit 0; Nx ran `cargo check --target-dir dist/target/cli -p cli`. |
| `npx nx run cli:test` | pass | Exit 0; Nx ran cli tests, 181 unit tests and 2 debug-log startup process tests passed. Cargo emitted the expected temporary Git CRLF warning from a test fixture only. |

Product failure summary: none.

Local tooling summary: all requested tools were available. Nx commands emitted Node's experimental CommonJS/ESM warning from the local npm installation, but every Nx target completed successfully.

## Commit Checkpoint

- Commit: `e59ff2ef0a094e8d1439e14dd1224271fe536a81`
- Message: `docs(architecture): document lifecycle package graph`
- Scope: `docs/architecture-and-packages.md`, effort 15 Doric artifacts, `STATE.md`, and carried-forward effort 14 hash finalization.
