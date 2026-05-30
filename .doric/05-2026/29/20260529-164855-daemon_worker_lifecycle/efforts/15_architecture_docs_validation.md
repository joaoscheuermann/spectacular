# Effort: architecture docs validation

Status: done

## Requirement links

- Features: F-12 plus documentation coverage for F-01 through F-11
- PRD: all goals and AC-1 through AC-15 through validation evidence
- TDD: architecture documentation note, rollout step 12, validation commands, repaired decisions

## Goal

Update architecture documentation for the completed lifecycle package graph and run the final package/workspace validation matrix.

## Sequence

- Position: 15 of 15
- Previous effort: 14_lifecycle_integration_smoke.md
- Enables: coordinator can request handover review with current architecture and validation evidence.

## Target files

- `docs/architecture-and-packages.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/15_architecture_docs_validation.md`

## Coupled files

- `Cargo.toml`, `package.json`, and `packages/*/project.json` are read-only evidence for the documented package map.
- `packages/lifecycle`, `packages/daemon`, `packages/worker`, `packages/cli`, `packages/tools`, `packages/config`, `packages/llms`, and `packages/agent` source trees are read-only evidence for dependency direction.
- Earlier effort validation records are read-only context for final validation.

## Ownership

- Intended worker write scope: architecture document and this effort's validation record only.
- Read-only context: all package manifests and source boundaries needed to document the final graph.
- Known conflict risks: architecture docs are shared repo truth. Confirm the implementation graph before editing; do not document intended edges that are not present in code.

## Tests to add or update

- No new behavior tests are expected.
- Update documentation assertions manually from manifests and source imports.
- Record validation command results in the run validation artifact during development.

## Regression suites

- `cargo fmt --all -- --check`
- `cargo metadata --format-version 1 --no-deps`
- `cargo build -p lifecycle`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo test -p daemon --no-fail-fast`
- `cargo test -p tools --no-fail-fast`
- `cargo test -p worker --no-fail-fast`
- `cargo test -p cli --no-fail-fast`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build -p cli --bin doric`
- `cargo build -p daemon --bin doric-daemon`
- `cargo build -p worker --bin doric-worker`
- `npx nx run lifecycle:build`
- `npx nx run lifecycle:test`
- `npx nx run daemon:build`
- `npx nx run daemon:test`
- `npx nx run tools:test`
- `npx nx run worker:build`
- `npx nx run worker:test`
- `npx nx run cli:build`
- `npx nx run cli:test`

## Acceptance criteria

- `docs/architecture-and-packages.md` lists `lifecycle`, `daemon`, and `worker` with accurate responsibilities and dependency direction.
- Documentation states that `packages/tools` remains shared and worker registration is not confinement.
- Documentation states that worker provider/runtime composition is worker-local in v1 and chat composition remains in `cli`.
- Documentation states that lifecycle commands bypass chat debug-log startup while bare chat behavior remains.
- Documentation states that v1 uses the current `tonic-prost-build`/`tonic-prost`/`prost` stack and external Git for repo clone.
- Final validation records distinguish product failures from unavailable local tools such as Nx, if any.

## Notes

- This is the correct effort for the architecture document update because it should describe the implemented package graph, not just the planned one.
- Do not broaden this effort into README/product marketing unless the coordinator explicitly requests it.
