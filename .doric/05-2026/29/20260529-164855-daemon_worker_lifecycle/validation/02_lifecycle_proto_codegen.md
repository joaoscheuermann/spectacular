# Validation: lifecycle proto codegen

## Red evidence

Captured before implementation by `agents/034_effort_02_test_writer.md`.

| Command | Exit code | Failure summary | Acceptance criteria mapping |
| --- | --- | --- | --- |
| `cargo test -p lifecycle --lib --no-fail-fast` | 1 | `error[E0433]: failed to resolve: could not find proto in the crate root` at `use lifecycle::proto::doric::lifecycle::v1;` | Proves the public generated namespace and generated message/service symbols are missing before lifecycle proto/codegen implementation. |
| `cargo build -p lifecycle` | 0 | Non-test crate build still succeeds because the new contract assertions are test-only. | Establishes baseline package compilation while red contract tests fail at the intended public boundary. |
| `rustfmt packages/lifecycle/src/lib.rs packages/lifecycle/tests/unit/proto_contract.rs` | 0 | Touched Rust test files format cleanly. | Keeps red tests valid and formatted before implementation. |

No tooling was unavailable during red evidence capture.

## Green evidence

Captured by `agents/036_effort_02_validator_refactor.md`.

| Command | Exit code | Summary |
| --- | --- | --- |
| `cargo build -p lifecycle` | 0 | Lifecycle crate builds with `tonic-prost-build` codegen and vendored `protoc` fallback. |
| `cargo test -p lifecycle --lib --no-fail-fast` | 0 | Public generated namespace contract tests pass: 4 passed. |
| `cargo test -p lifecycle --no-fail-fast` | 0 | Package tests and doctests pass: 4 unit tests passed, 0 doctests. |
| `cargo fmt --all -- --check` | 0 | Workspace formatting check passes after formatting `build.rs`. |
| `npx nx run lifecycle:build` | 0 | Nx lifecycle build target succeeds through `cargo check --target-dir dist/target/lifecycle -p lifecycle`. |
| `npx nx run lifecycle:test` | 0 | Nx lifecycle test target succeeds through `cargo test --target-dir dist/target/lifecycle -p lifecycle`: 4 unit tests passed. |
| `cargo tree -p lifecycle --depth 1` | 0 | Direct dependencies are `prost`, `prost-types`, `tonic`, `tonic-prost`; build dependencies are `protoc-bin-vendored`, `tonic-prost-build`; no direct `cli`, `daemon`, `worker`, or `tools` dependency. |
| `git diff --check -- Cargo.lock packages/lifecycle` | 0 | No whitespace errors in effort-owned diff. |

## Focused commands

- `cargo build -p lifecycle`
- `cargo test -p lifecycle --lib --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`

## Regression commands

- `cargo fmt --all -- --check`
- `npx nx run lifecycle:build`
- `npx nx run lifecycle:test`
- `cargo tree -p lifecycle --depth 1`
- `git diff --check -- Cargo.lock packages/lifecycle`

## Unavailable tooling

None.

## Refactors applied

- Renamed the worker session RPC from `Connect` to `Session` inside `WorkerSessionService` because tonic 0.14 generates a client constructor named `connect`; an RPC with the same name creates duplicate inherent method definitions.
- Adjusted the generated service module test to account for tonic 0.14 service traits with associated stream types. The correction preserves the accepted public contract intent by still proving the generated client modules, server modules, service traits, and server wrapper types are reachable from `lifecycle::proto::doric::lifecycle::v1`.
- Validator/refactor applied no additional refactors.

## Reviewer decision

Initial reviewer `agents/037_effort_02_reviewer.md` rejected checkpoint readiness because unrelated files were already staged and the post-commit effort 01 ledger hash update was outside effort 02 implementation scope. Implementation scope and validation evidence had no reported blockers.

Replacement reviewer `agents/039_effort_02_reviewer_checkpoint_retry.md` approved checkpoint readiness after scope reconciliation. The reviewer found no blocking findings, accepted the explicit `git commit --only -- ...` checkpoint scope while preserving unrelated staged files outside the pathspec, and found no remaining implementation or validation blocker.
