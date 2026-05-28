# Effort: Rust identity references

Status: done

## Requirement links

PRD-FR1, PRD-FR2, PRD-FR5, PRD-FR6, PRD-FR7, PRD-AC1, PRD-AC4, PRD-AC6, TDD-04-IMPORTS, TDD-04-TYPE-RENAMES, TDD-06-CLAP-NAME, TDD-07-CONFIG-DIR, TDD-07-DEBUG-LOG, TDD-07-TUI-ENV, TDD-07-VENDORED-IOCRAFT

## Goal

Update Rust crate imports, old-brand type names, product strings, config paths, debug logs, temp names, environment variables, and vendored IOCraft active constants.

## Target files

- `packages/cli/src/**`
- `packages/cli/tests/**`
- `packages/agent/**`
- `packages/commands/**`
- `packages/config/**`
- `packages/llms/**`
- `packages/tools/**`
- `packages/tui/**`
- `packages/vendor/iocraft/**`

## Coupled files

- `Cargo.lock`
- `README.md`
- `.agents/skills/**`

## Tests to add or update

- CLI parse/help/config output tests.
- Config path tests.
- Debug log startup tests.
- TUI env/snapshot tests.
- Existing package integration tests after crate import rename.

## Regression suites

- `cargo test -p cli`
- `cargo test -p config`
- `cargo test -p llms`
- `cargo test -p agent`
- `cargo test -p tools`
- `cargo test -p tui`
- `cargo test -p commands`

## Acceptance criteria

- Rust imports use `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.
- `SpectacularConfig` and related config type names become `DoricConfig`.
- CLI visible command name is `doric`.
- Config directory is `doric`; no old config fallback exists.
- Debug log and env var names are Doric-only.
- Active vendored IOCraft constants no longer contain Spectacular names.

## Notes

Serialized JSON fields were kept stable unless they explicitly contained old
product identity. Short crate names required absolute crate paths in the CLI
package where local modules share names with external crates.
