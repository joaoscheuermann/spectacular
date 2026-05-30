# Effort: lifecycle proto codegen

Status: todo

## Requirement links

- Features: F-02, F-09, F-10, F-12
- PRD: FR-3, FR-4, FR-7, FR-8, FR-9, FR-14; AC-1, AC-2, AC-7, AC-11, AC-14
- TDD: `packages/lifecycle`, shared lifecycle DTOs, generated Rust contract, gRPC services, Dependency Hop: gRPC/protobuf build

## Goal

Create the shared lifecycle protobuf contract and generated Rust module pipeline in `packages/lifecycle` using the repaired tonic/prost stack.

## Sequence

- Position: 02 of 15
- Previous effort: 01_workspace_package_skeletons.md
- Enables: daemon and worker can depend on one shared protocol contract instead of duplicating service/message shapes.

## Target files

- `packages/lifecycle/Cargo.toml`
- `packages/lifecycle/build.rs`
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- `packages/lifecycle/src/lib.rs`
- `packages/lifecycle/src/proto.rs`
- `packages/lifecycle/tests/unit/proto_contract.rs`
- `Cargo.lock`

## Coupled files

- `packages/daemon/Cargo.toml` and `packages/worker/Cargo.toml` are downstream consumers, read-only in this effort.
- `packages/cli/Cargo.toml` is a later consumer for the CLI client.
- Generated code must not be hand-edited or copied into daemon/worker packages.

## Ownership

- Intended worker write scope: lifecycle package contract/codegen files and lockfile updates caused by lifecycle dependencies.
- Read-only context: TDD protocol requirements and future daemon/worker target package boundaries.
- Known conflict risks: `Cargo.lock` can conflict with any parallel dependency addition; do not combine unrelated dependency churn.

## Tests to add or update

- Add `packages/lifecycle/tests/unit/proto_contract.rs`.
- Prove generated service/message modules are reachable from the public `lifecycle::proto::doric::lifecycle::v1` namespace.
- Prove the generated contract includes CLI-facing and worker-facing service types needed by later efforts.

## Regression suites

- `cargo build -p lifecycle`
- `cargo test -p lifecycle --no-fail-fast`
- `npx nx run lifecycle:build`
- `npx nx run lifecycle:test`

## Acceptance criteria

- `build.rs` uses `tonic-prost-build` and falls back to `protoc-bin-vendored` when `PROTOC` is not explicitly set.
- Runtime dependencies include the current repaired stack: `tonic`, `tonic-prost`, `prost`, and `prost-types`.
- The proto defines lifecycle dispatch, list, stream, answer, and worker session messages/services sufficient for daemon, worker, and CLI efforts.
- Codegen succeeds from a clean Windows checkout without requiring a system `protoc`.
- `lifecycle` does not depend on `cli`, `daemon`, `worker`, or `tools`.

## Notes

- Preserve the repaired TDD decision: do not use an older or invalid tonic/prost build stack.
- Keep hand-written domain wrappers and redaction out of this effort; they are covered by the next slice.
