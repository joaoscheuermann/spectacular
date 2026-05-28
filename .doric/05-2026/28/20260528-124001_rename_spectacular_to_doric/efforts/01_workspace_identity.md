# Effort: Workspace identity

Status: done

## Requirement links

PRD-FR2, PRD-FR3, PRD-FR4, PRD-AC2, PRD-AC3, TDD-01-PREFLIGHT-COLLISIONS, TDD-02-DIRECTORY-RENAME, TDD-03-CARGO-WORKSPACE, TDD-03-CARGO-PACKAGES, TDD-03-CLI-BIN-TARGET, TDD-05-NX-METADATA

## Goal

Rename package directories and workspace metadata to the new role-based names, with the CLI package named `cli` and binary named `doric`.

## Target files

- `Cargo.toml`
- `Cargo.lock`
- `packages/*/Cargo.toml`
- `packages/*/project.json`
- `packages/spectacular*`

## Coupled files

- `package.json`
- `package-lock.json`
- `nx.json`
- `.github/workflows/ci.yml`

## Tests to add or update

- Existing metadata assertions and package tests that depend on package or binary names.

## Regression suites

- `cargo metadata --format-version 1 --no-deps`
- `npx nx show projects`
- `cargo build -p cli --bin doric`

## Acceptance criteria

- Target package directories exist and old package directories do not.
- Cargo workspace members use `packages/cli`, `packages/agent`, `packages/commands`, `packages/config`, `packages/llms`, `packages/tools`, and `packages/tui`.
- Package names are `cli`, `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.
- `packages/cli/Cargo.toml` exposes binary `doric`.
- Nx project names, source roots, and target dirs use the new names.

## Notes

Preflight collision checks returned false for every target package directory.
Cargo workspace metadata now resolves the renamed packages and `packages/cli`
builds the `doric` binary. Nx project files were renamed, but local Nx execution
is blocked until JS dependencies are installed.
