# Prompt

## Feature summary

Rename the project identity from Spectacular to Doric across package, workspace, documentation, configuration, and user-facing naming surfaces. Remove the `spectacular` package prefix from all packages. Rename the current `spectacular` package, which is the CLI entry point, to `cli`.

## User value

Users and contributors see one consistent product and repository identity: Doric. Package names become shorter and easier to scan, and the CLI entry package becomes clearly identified by role instead of carrying the old project name.

## Business or commercial driver

Complete the product rebrand from Spectacular to Doric and reduce naming friction in the monorepo before future development builds on the new identity.

## Personas

- End users running the CLI.
- Contributors navigating packages, Nx targets, Cargo crates, docs, and tests.
- Maintainers publishing, releasing, or validating the workspace.

## Constraints

- Treat `spcetacular` as a typo for `spectacular`.
- Preserve existing behavior unless the rename explicitly requires visible name changes.
- Rename package, crate, and project identifiers coherently across build metadata, imports, tests, docs, scripts, and CI/Nx targets.
- Apply a hard rename with no Spectacular compatibility aliases.
- Avoid unrelated refactors or behavior changes.
- Preserve unrelated worktree changes during implementation.

## Resolved decisions

- The project name should become Doric.
- Package prefixes named `spectacular` should be removed.
- The current CLI entry package named `spectacular` should be renamed to `cli`.
- Existing typo `spcetacular` in the request is not a distinct target name.
- The installed binary and user command should become `doric`.
- Legacy Spectacular binary aliases, config paths, environment variable names, command aliases, and publish names should not be retained.
- Internal packages should use unscoped role names: `cli`, `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.

## Open questions

- None currently blocking PRD or technical design.

## Non-goals

- Redesigning CLI behavior, commands, or runtime architecture.
- Changing feature behavior beyond rename fallout.
- Broad package-boundary refactors unrelated to removing the old name.
- Introducing a migration system unless required by the chosen compatibility policy.
