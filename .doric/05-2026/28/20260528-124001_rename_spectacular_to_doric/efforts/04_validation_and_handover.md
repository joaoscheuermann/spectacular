# Effort: Validation and handover

Status: done

## Requirement links

PRD-AC1, PRD-AC3, PRD-SM1, PRD-SM2, PRD-SM3, PRD-SM4, PRD-SM5, PRD-SM6, TDD-TEST-METADATA, TDD-TEST-FOCUSED, TDD-TEST-WORKSPACE, TDD-TEST-STALE-SCAN, TDD-RISK-HANDOVER

## Goal

Regenerate checked-in metadata, run focused and workspace validation, classify any remaining old-name references, and write the Doric handover report.

## Target files

- `Cargo.lock`
- `.doric/05-2026/28/20260528-124001_rename_spectacular_to_doric/HANDOVER.md`

## Coupled files

- All files touched by efforts 01-03.

## Tests to add or update

- Add only validation-oriented checks if stale references remain in active repo-controlled files.

## Regression suites

- `cargo metadata --format-version 1 --no-deps`
- `npx nx show projects`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-features`
- `cargo nextest run --workspace --all-features`
- `npx nx run-many -t lint build typecheck`
- Doric stale-reference scans.

## Acceptance criteria

- `Cargo.lock` contains new package names and no stale active Spectacular package entries.
- Focused package validation passes or failures are documented with root cause.
- Workspace/CI-equivalent validation passes where tooling is available or blockers are documented.
- Final stale-reference scan has zero active old-name hits or only documented allowed exceptions.
- `HANDOVER.md` summarizes changed files, validation, risks, and hard-rename consequences.

## Notes

Rust validation passed with `cargo metadata`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo test --workspace --all-features --no-fail-fast`. Local Nx validation is
blocked because Nx modules are not installed in the checkout.
