# Agent Instructions

`GROUNDING.md` is the repository validity contract. Read it before
non-trivial planning, reviewing, artifact generation, architecture discussion,
code editing, or workflow execution.

If `GROUNDING.md` cannot be read, stop and report that the repository
grounding contract cannot be satisfied.

## Required Context

For non-trivial work, read only the context needed to act correctly, in this
order:

1. `GROUNDING.md`.
2. This root `AGENTS.md`.
3. Applicable nested `AGENTS.md` files, from broadest to most specific.
4. Matching `.agents/skills/*/SKILL.md` files.
5. Nearby source, tests, manifests, schemas, generated contracts, or artifacts.

Use `rg` or the fastest available local search tool to find nested
instructions and relevant files. If a repository skill applies, load the
smallest useful set and state which skill instructions are being followed.

## Working Rules

- Inspect the real local files before making architecture or implementation
  claims.
- Keep edits scoped to the request and the relevant ownership boundary.
- Preserve unrelated dirty worktree changes.
- Prefer existing package patterns, helper APIs, and tests before adding new
  abstractions.
- Use `apply_patch` for manual file edits.
- Do not use destructive commands, broad rewrites, or rollback operations
  unless the user explicitly approves them.
- Use `GROUNDING.md` for hard constraints, scope gates, and architecture
  validity.
- Update `GROUNDING.md` when the task intentionally changes product scope,
  package responsibilities, dependency direction, provider composition,
  built-in tools, event streaming, user-input handling, session persistence,
  config schema, runtime behavior, or vendored dependency strategy.

## Code Work

For code updates, the main agent orchestrates instead of acting as the primary
editor:

1. Explore the current behavior, ownership boundary, relevant instructions,
   nearby tests, and configured checks.
2. Define the implementation goal, dependency direction, write scope,
   validation proof, non-goals, and stop conditions.
3. Delegate implementation to a sub-agent with explicit context, read scope,
   write scope, validation expectations, and a reminder not to revert others'
   work.
4. Delegate focused review or validation to a sub-agent using
   `.agents/skills/coding-conventions/SKILL.md`, the implementation goal,
   `GROUNDING.md`, this file, and the relevant tests or checks.

Each handoff must tell the child it is the sub-agent for that assignment and
that its role is to execute the main agent's bounded handoff. If the required
sub-agent mechanism is unavailable, blocked, or unsafe, stop before code
changes unless the user explicitly authorizes a scoped main-agent edit.

## Nx And Packages

- Product packages belong under `packages/*`; do not add root-level product
  packages.
- Use current manifests and source as the package inventory.
- Create TypeScript packages with the `@nx/js` generator when available.
- Export package APIs through `src/index.ts`.
- Consume sibling packages by public package entrypoint, not private source
  paths or `../` imports.
- Declare workspace package dependencies in consuming package metadata when
  metadata exists.
- Run `npx nx sync` when TypeScript project references need reconciliation.
- Validate package changes with `npx nx show projects` plus affected
  `typecheck`, `test`, or `build` targets.

## Validation And Response

Use the narrowest reliable proof for the change. Prefer configured Nx targets
for TypeScript work, repository formatter checks for formatting work, and
`git diff --check` for docs-only changes.

Do not claim completion without validation evidence. If validation is blocked,
state the blocker and residual risk.

Final responses should name changed files, summarize the change, and report
validation. Keep them concise.
