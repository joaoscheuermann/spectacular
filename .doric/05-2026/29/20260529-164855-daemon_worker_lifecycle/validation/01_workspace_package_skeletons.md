# Validation: workspace package skeletons

## Red evidence

Captured before implementation by `agents/026_effort_01_test_writer.md`.

| Command | Exit code | Failure summary | Acceptance criteria mapping |
| --- | --- | --- | --- |
| Cargo metadata package/target assertion | 1 | `missing Cargo packages: lifecycle, daemon, worker` | Proves the workspace does not yet list the three new packages and their required lib/bin targets cannot be present. |
| `cargo check -p lifecycle` | 1 | `error: package ID specification 'lifecycle' did not match any packages` | Proves the `lifecycle` library package does not yet exist as a Cargo package. |
| Nx metadata assertion | 1 | `missing Nx project metadata: packages/lifecycle/project.json` | Proves the new packages do not yet have required Nx `build`, `test`, and `lint` metadata. |

No tooling was unavailable during red evidence capture.

## Green evidence

Captured by `agents/028_effort_01_validator_refactor.md`.

| Command | Exit code | Summary |
| --- | --- | --- |
| `cargo metadata --format-version 1 --no-deps` | 0 | Workspace metadata generated successfully. |
| Cargo metadata PowerShell assertion | 0 | `cli`, `agent`, `commands`, `llms`, `config`, `tools`, `tui`, `lifecycle`, `daemon`, and `worker` packages are present; `lifecycle` has a lib target, `daemon` has lib plus `doric-daemon`, and `worker` has lib plus `doric-worker`. |
| `cargo check -p lifecycle` | 0 | `lifecycle` library crate checks. |
| `cargo check -p daemon` | 0 | `daemon` package checks. |
| `cargo check -p worker` | 0 | `worker` package checks. |
| `cargo build -p daemon --bin doric-daemon` | 0 | `doric-daemon` binary builds. |
| `cargo build -p worker --bin doric-worker` | 0 | `doric-worker` binary builds. |
| Nx metadata PowerShell assertion | 0 | New `project.json` files expose required `@monodon/rust` build, test, and lint executors with `dist/target/<project>` target dirs. |
| `npx nx show projects` | 0 | Nx discovery completed. |
| `npx nx show projects --json` | 0 | Output includes `lifecycle`, `daemon`, and `worker`. |
| `npx nx run lifecycle:build` | 0 | Nx build target checks `lifecycle`. |
| `npx nx run daemon:build` | 0 | Nx build target checks `daemon`. |
| `npx nx run worker:build` | 0 | Nx build target checks `worker`. |
| `cargo fmt --all -- --check` | 0 | Rust formatting check passed. |
| `git diff --check -- Cargo.toml Cargo.lock packages/lifecycle packages/daemon packages/worker` | 0 | No whitespace errors in effort-owned diff. |

## Focused commands

- `cargo metadata --format-version 1 --no-deps`
- Cargo metadata PowerShell assertion
- `cargo check -p lifecycle`
- `cargo check -p daemon`
- `cargo check -p worker`
- `cargo build -p daemon --bin doric-daemon`
- `cargo build -p worker --bin doric-worker`
- Nx metadata PowerShell assertion

## Regression commands

- `npx nx show projects`
- `npx nx show projects --json`
- `npx nx run lifecycle:build`
- `npx nx run daemon:build`
- `npx nx run worker:build`
- `cargo fmt --all -- --check`
- `git diff --check -- Cargo.toml Cargo.lock packages/lifecycle packages/daemon packages/worker`

## Unavailable tooling

None. `npx nx` emitted Node experimental CommonJS/ESM warnings, but all Nx commands completed successfully.

## Refactors applied

None.

## Reviewer decision

Initial reviewer `agents/029_effort_01_reviewer.md` rejected checkpoint readiness because unrelated files were staged before this effort. Replacement reviewer `agents/030_effort_01_reviewer_checkpoint_retry.md` approved the implementation and explicit `git commit --only -- <effort paths>` checkpoint plan.
