# Product Requirements Document

## Problem statement

The repository, packages, documentation, configuration, and user-facing naming surfaces currently carry the old project identity, Spectacular. This creates inconsistent branding, longer package names, and ambiguity around the CLI entry package.

The product identity must be renamed to Doric across the monorepo. Packages that use the `spectacular` prefix must have that prefix removed. The current CLI entry package named `spectacular` must be renamed to `cli`. Existing behavior must remain unchanged except where rename fallout requires visible naming updates.

## Goals

1. Rename the project identity from Spectacular to Doric across repository-controlled product, package, documentation, configuration, build, test, and release surfaces.
2. Remove the `spectacular` prefix from package, crate, project, module, target, and workspace identifiers.
3. Rename the current CLI entry package from `spectacular` to `cli`.
4. Treat `spcetacular` as a typo for `spectacular` and include it in rename discovery.
5. Preserve existing runtime behavior, command behavior, tests, and architecture except for direct consequences of the rename.
6. Keep package names coherent and role-based after prefix removal.
7. Ensure contributors can build, test, lint, and navigate the workspace using the new Doric naming.

## Non-goals

1. Redesigning CLI commands, UX flows, runtime behavior, provider behavior, or command semantics.
2. Reworking package boundaries beyond what is required to remove old naming.
3. Introducing broad architectural refactors.
4. Adding a migration system or compatibility alias layer.
5. Changing unrelated documentation content beyond product and package naming.
6. Reformatting unrelated files or normalizing code style outside touched rename surfaces.
7. Preserving legacy Spectacular binary aliases, config paths, environment variable names, command aliases, or publish names.

## Personas

1. End users running the CLI who expect the product name, help text, errors, logs, documentation, and command surfaces to present a consistent Doric identity.
2. Contributors navigating packages, crates, Nx projects, imports, tests, scripts, and docs who need concise role-based names without the old `spectacular` prefix.
3. Maintainers publishing, releasing, validating, or troubleshooting the workspace who need metadata, CI, release scripts, and package identifiers to agree.

## Primary workflows

1. End user invokes the CLI and sees Doric naming in visible product surfaces.
2. Contributor opens the monorepo and identifies packages by role, such as `cli`, rather than old prefixed names.
3. Contributor builds, tests, lints, and runs affected packages through existing workspace tooling using renamed package and target identifiers.
4. Maintainer prepares release or validation artifacts and sees Doric branding in package metadata, generated outputs, docs, and release-related configuration.
5. Developer searches for old Spectacular naming and finds zero matches or only explicitly documented historical references, immutable external data, or external registry constraints that affect publication timing only.

## Functional requirements

1. Product identity rename:
   - All active product-facing references to `Spectacular`, `spectacular`, and typo `spcetacular` must be reviewed.
   - Active project identity references must be renamed to `Doric` or `doric` using casing appropriate to the surface.
   - Historical references may remain only when they are explicitly historical or part of immutable external data.

2. Package prefix removal:
   - All workspace packages using the `spectacular` prefix must be renamed to remove that prefix.
   - Renamed package identifiers must be unscoped, role-based, and unique within the workspace: `cli`, `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.
   - Internal imports, crate references, package manifests, lockfiles, tests, scripts, docs, and generated target references must be updated to match renamed packages.

3. CLI package rename:
   - The current CLI entry package named `spectacular` must become `cli`.
   - The installed binary and user command must become `doric`.
   - Workspace configuration, build metadata, package references, tests, release scripts, and docs must refer to the CLI package as `cli`.
   - The package rename must not change CLI runtime behavior except for hard-rename user-visible naming changes.

4. Build and workspace metadata:
   - Cargo workspace metadata, Nx project metadata, package manifests, CI configuration, scripts, and test commands must be updated to use the new package names.
   - Any generated or checked-in metadata that directly references renamed packages must be updated or regenerated.
   - No stale references may remain in active build paths.

5. User-facing naming:
   - Help text, docs, examples, error messages, logs, config descriptions, release notes, and README content must use Doric naming unless explicitly historical or immutable external data.
   - Command examples and binary references must use `doric`.

6. Compatibility-sensitive surfaces:
   - Existing Spectacular config paths, environment variables, binary names, command aliases, and external package names must be hard-renamed to Doric names.
   - No legacy Spectacular compatibility aliases should remain.

7. Behavior preservation:
   - Existing tests that do not assert old names should continue to pass without expectation changes.
   - Tests that assert old names must be updated only when the visible name is intentionally changed.
   - No feature behavior, runtime control flow, provider logic, or command semantics may change as part of this work.

## Acceptance criteria

1. Repository search:
   - A case-sensitive and case-insensitive search for `Spectacular`, `spectacular`, and `spcetacular` finds no active old-brand references except explicitly historical references or immutable external data.
   - Any remaining old-brand references are documented in the handover with path, line, and reason.
   - Final old-name scans either return zero matches or only documented allowed exceptions with path, line, and rationale.

2. Package naming:
   - No active package, crate, Nx project, workspace member, or internal target retains the `spectacular` prefix.
   - The former `spectacular` CLI entry package is named `cli`.
   - The CLI binary name is `doric`.
   - All renamed package identifiers are unique and resolve through workspace tooling.

3. Build and tests:
   - Workspace metadata loads successfully after the rename.
   - A clean build of affected packages succeeds.
   - Affected test suites pass with updated package names.
   - CI or equivalent local validation no longer references removed package names.

4. CLI behavior:
   - Existing CLI behavior remains equivalent apart from approved visible naming changes.
   - Help, version, and about text use Doric naming and the `doric` binary name.
   - No commands are added, removed, or semantically changed by the rename.

5. Documentation:
   - README, contributor docs, package docs, command examples, and release-related docs use Doric naming.
   - Examples reference the correct package and command names after the rename.
   - Legacy names appear only in explicitly historical context, immutable external data, or documented external registry constraints that affect publication timing only.

6. Edge cases:
   - The typo `spcetacular` is treated as an old-name match and is not preserved as a separate identity.
   - Case variants are handled correctly: `Doric` for product name, `doric` for lowercase identifiers where appropriate.
   - Generated files or lockfiles are updated only when they are checked in and required for builds or tests.

7. Failure states:
   - If a rename target would collide with an existing package or target, implementation must stop and identify the collision before editing further.
   - If an external registry, credential, or release ownership constraint prevents immediate publication under a renamed package or binary, implementation must document the constraint and still complete repository-controlled package metadata, docs, commands, checked-in release config, and local hard-rename work.

## Success measures

1. Zero unapproved active references to `Spectacular`, `spectacular`, or `spcetacular` remain after implementation.
2. All renamed packages are discoverable and runnable through the workspace's standard build and test tooling.
3. The former CLI package is consistently addressed as `cli` in package metadata, workspace targets, tests, and docs.
4. The CLI binary is consistently addressed as `doric` in package metadata, tests, help text, and docs.
5. Existing behavior-focused tests continue to pass.
6. New or updated rename-focused tests cover package identity, CLI visible naming, and stale-reference prevention where practical.
7. Maintainers can run the documented validation commands without manually translating old package names.

## Risks and compliance

1. Compatibility risk:
   - Hard-renaming the installed binary, config paths, environment variables, or package publish names will break existing users or automation that still use Spectacular names.
   - Mitigation: document this as an intentional hard rename and do not add compatibility aliases.

2. Build graph risk:
   - Package renames can break Cargo workspace membership, Nx project references, imports, generated metadata, and CI scripts.
   - Mitigation: inventory all naming surfaces before edits and validate with workspace build and test commands.

3. Incomplete rename risk:
   - Old names may remain in docs, tests, scripts, comments, generated files, or typo variants.
   - Mitigation: use exhaustive search for `Spectacular`, `spectacular`, and `spcetacular`; classify remaining references.

4. Behavioral regression risk:
   - Rename work may accidentally change command semantics or runtime behavior.
   - Mitigation: scope edits to rename fallout and run behavior-focused tests.

5. Release risk:
   - Published package names and binary names may have external consumers.
   - Mitigation: treat the hard rename as intentional, do not add legacy aliases, and defer only actions that require unavailable external credentials, registry ownership, or release timing.

6. Compliance/privacy:
   - No new privacy, data retention, accessibility, or security behavior is intended.
   - Existing compliance posture should remain unchanged.

## Open questions

1. Non-blocking for PRD generation: Which exact files or generated metadata are checked in and must be regenerated versus ignored as build output? This can be resolved during technical design and implementation inventory.
