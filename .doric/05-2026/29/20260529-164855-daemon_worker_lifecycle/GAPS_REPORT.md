# Gaps Report

## Source evaluator receipts

- `agents/008_technical_grounded_evaluator.md`
- `agents/010_technical_initial_filter_evaluator_epoch_2.md`
- `agents/013_technical_grounded_evaluator_epoch_3.md`
- `agents/015_technical_grounded_evaluator_epoch_4.md`

## Blocking gaps

1. Tool ownership migration is underspecified and conflicts with preserving existing chat. The current repo has `cli` depending on `tools`, `chat/runner.rs` building tools through the `tools` crate, and `docs/architecture-and-packages.md` naming `packages/tools` as tool owner. The TDD must choose the final tool ownership graph before decomposition.
2. The repaired TDD still names an invalid current tonic/prost codegen plan. Current docs for `tonic-build` 0.14 direct protobuf compilation to `tonic-prost-build`, and generated prost service code uses `tonic-prost` at runtime.
3. Worker provider/runtime composition is not designed. Current provider/runtime selection and OpenAI auth storage live under `packages/cli/src/chat/*`, while `worker` cannot depend on `cli`.
4. The shared-tools plan claims worker tools are confined to the cloned repo root, but current `packages/tools` uses the root as a default/base and still allows absolute paths and traversal.

## Non-blocking gaps

- The proposed `lifecycle`, `daemon`, and `worker` package split otherwise fits the composition-root model.
- CLI command surface, status model, event replay, daemon-mediated answer path, in-memory restart behavior, host-root convention, and test location strategy are directionally compatible with the current repo.

## Invalid Dependency Hops

- Tool ownership hop is not concrete enough to decompose.
- gRPC/protobuf build hop is missing.
- gRPC/protobuf build hop was added but uses the wrong current crates.
- CLI entrypoint/debug-log setup hop is missing.
- Repo clone mechanism hop is missing.
- Worker provider/runtime composition hop is missing.
- Tool root/confinement semantics are inaccurate.

## Required rewrites

1. Choose the final package graph for tool ownership.
2. Add a gRPC/protobuf build Dependency Hop naming codegen strategy, build dependencies, Windows/protoc handling, and Nx/Cargo validation path.
3. Add a CLI entrypoint rewrite note so lifecycle commands do not require chat/provider debug-log setup.
4. Add a repo-clone mechanism assumption naming external `git` versus a Rust library and the failure/test seam.
5. Update the gRPC/protobuf plan to the current `tonic`, `tonic-prost`, `prost`, `prost-types`, `tonic-prost-build`, and `protoc-bin-vendored` stack, or explicitly pin an older stack with a compatibility rationale.
6. Replace the ambiguous `monodon-rust.md` evidence citation with current repo package metadata/project files or the full skill reference path.
7. Add a worker provider/runtime composition section and Dependency Hop. Decide between worker-local composition from `config` + `llms`, or extracting the seam into a lower package. Update package graph, rollout, risks, and tests.
8. Repair tool-root semantics. Choose either repo-root defaults without confinement for v1, with Docker/scoped tooling deferred, or add a scoped tool mode/wrapper with tests.

## Minimal context for next architect

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/GAPS_REPORT.md`
- `docs/architecture-and-packages.md`
- `Cargo.toml`
- `packages/cli/Cargo.toml`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/chat/runner.rs`
- `packages/tools/src/lib.rs`
- `packages/cli/src/chat/mod.rs`
- `packages/cli/src/chat/runtime_selection.rs`
- `packages/cli/src/chat/provider.rs`
- `packages/cli/src/chat/auth.rs`
- `packages/tools/src/path.rs`
- `packages/tools/src/write.rs`
- `packages/tools/src/terminal.rs`
- `docs.rs tonic-build 0.14 documentation`
- `docs.rs tonic-prost 0.14 documentation`

## Coordinator notes

Fresh architect should repair the existing TDD in place. Preferred product constraint: keep existing chat functional unless a new accepted artifact explicitly retires it. Do not broaden implementation into a full chat retirement.
