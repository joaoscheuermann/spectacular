# Effort: workspace package skeletons

Status: done

## Requirement links

- Features: F-02, F-03, F-04, F-12
- PRD: FR-1, FR-4, FR-7, FR-8, FR-11, FR-13, FR-14; AC-4, AC-7, AC-9, AC-13, AC-14
- TDD: Package additions, target package direction, rollout step 1, package boundary tournament

## Goal

Add compileable empty workspace packages for `lifecycle`, `daemon`, and `worker`, including Cargo workspace membership and Nx project metadata, without adding lifecycle behavior yet.

## Sequence

- Position: 01 of 15
- Previous effort: none
- Enables: all later efforts can target real packages and package-level validation commands.

## Target files

- `Cargo.toml`
- `packages/lifecycle/Cargo.toml`
- `packages/lifecycle/project.json`
- `packages/lifecycle/src/lib.rs`
- `packages/daemon/Cargo.toml`
- `packages/daemon/project.json`
- `packages/daemon/src/lib.rs`
- `packages/daemon/src/main.rs`
- `packages/worker/Cargo.toml`
- `packages/worker/project.json`
- `packages/worker/src/lib.rs`
- `packages/worker/src/main.rs`

## Coupled files

- `package.json` is read-only context because existing `packages/*` workspaces should already include new package folders.
- Existing `packages/*/project.json` files are read-only templates for target shape and target-dir naming.
- `Cargo.lock` should change only if Cargo records the new dependency-free workspace package entries during validation.

## Ownership

- Intended worker write scope: root Cargo workspace membership and the three new package directories only.
- Read-only context: existing package manifests, Nx project metadata, and `docs/architecture-and-packages.md`.
- Known conflict risks: root `Cargo.toml` is a shared workspace graph file; coordinate before any parallel worker edits package membership.

## Tests to add or update

- No behavior tests yet.
- Add minimal package source files that compile cleanly under package-level `cargo check`.

## Regression suites

- `cargo metadata --format-version 1 --no-deps`
- `cargo check -p lifecycle`
- `cargo check -p daemon`
- `cargo check -p worker`
- `npx nx show projects` if local Nx dependencies are available.

## Acceptance criteria

- The Cargo workspace lists `packages/lifecycle`, `packages/daemon`, and `packages/worker` with no gaps in existing package membership.
- `lifecycle` is a library package.
- `daemon` is a library plus `doric-daemon` binary package.
- `worker` is a library plus `doric-worker` binary package.
- Each new package has Nx `build`, `test`, and `lint` targets matching existing `@monodon/rust` target style.
- Existing packages continue to appear in Cargo metadata.

## Notes

- Keep this effort deliberately mechanical. Do not add gRPC, proto, daemon registry, worker runtime, or CLI command behavior here.
- This establishes the selected `lifecycle` / `daemon` / `worker` package boundary from the repaired TDD.
