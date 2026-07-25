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
6. If present, `.agents/bundles/project/index.md` as advisory generated
   repository context.

Use `rg` or the fastest available local search tool to find nested
instructions and relevant files. If a repository skill applies, load the
smallest useful set and state which skill instructions are being followed.

`.agents/bundles/project/index.md` is generated context, not an authority
source. Use it only after the grounding contract, applicable agent
instructions, and current source files; verify generated claims against the
real files before acting.

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

## Prompt Authoring

Use `.agents/skills/write-agent-prompts/SKILL.md` when writing or materially
revising model-facing prompts.

- Write system prompts and user-message inputs passed to model `complete` or
  `stream` calls for human readers first, using simple, direct, unambiguous,
  evidence-grounded Markdown.
- Do not serialize instructions or context as a JSON object or array and use
  that serialization as the outer `complete` or `stream` user message. Literal
  JSON evidence may appear only in a labeled fenced `json` block inside an
  otherwise readable Markdown prompt.
- Use structural Markdown such as headings, lists, tables, and labeled fences
  when it improves comprehension. Avoid decorative formatting.
- Request Markdown or text output by default. Reserve JSON output for results
  that are genuinely machine-structured, and prefer the runtime's schema and
  validation mechanism over prompt-only JSON instructions.
- These prompt rules do not govern provider transport JSON, JSON-RPC,
  configuration, storage or persistence, tools or tool payloads, schemas, or
  other non-prompt JSON.

## Code Work

For production code updates, the top-level agent must coordinate the following
five phases in order. It manages handoffs and information isolation but does
not perform work assigned to a phase. Use a different, fresh sub-agent context
for each phase role; do not combine roles or skip a phase.

Every handoff must tell the child that it is the sub-agent for that assignment
and must execute only the bounded handoff. Specify the goal, supplied inputs,
read scope, write scope, non-goals, required proof, stop conditions, and the
requirement to preserve unrelated worktree changes.

1. **Exploration:** Assign read-only discovery of the applicable instructions,
   skills, ownership boundaries, source, tests, manifests, and configured
   checks. The explorer must return an evidence-backed context report without
   planning the change or editing files.
2. **Planning:** Give the planner the goal and exploration report. The planner
   must define the intended behavior, interfaces, dependency direction,
   failure handling, acceptance criteria, implementation scope, and non-goals
   without writing code or tests.
3. **Test writing:** Give the test writer the approved goal, plan, and
   interfaces. It may edit only approved test files and test-support artifacts.
   It must run the narrowest relevant test to establish red evidence for the
   expected behavioral reason. If meaningful red evidence is inapplicable, it
   must explain why and identify the closest validation proof before work
   continues.
4. **Code implementation:** Give the implementation agent the goal, plan,
   interfaces, production write scope, and non-goals. It may edit only
   production code. It must never read, search, list, or otherwise query test
   files or test-support artifacts; run tests; or consume test output. It may
   run only production-scoped, non-test checks such as build, typecheck, or
   lint.
5. **Validation:** Give an independent, read-only validator the goal, plan,
   implementation, tests, and configured checks. It must load
   `.agents/skills/coding-conventions/SKILL.md`, run the relevant tests and
   checks, review architecture and behavior, and verify worktree scope. It must
   report findings, green evidence, and proposed fixes without editing files.

The top-level agent must not disclose test paths, contents, diffs, assertions,
or results to the implementation agent. Route validation findings by
ownership: test defects to the test writer, production defects to the
implementation agent using only test-safe behavior or convention descriptions,
and contract defects to the planner. Preserve the implementation agent's test
isolation during corrections, and repeat independent validation after every
correction.

If the required sub-agent mechanism is unavailable, blocked, or unsafe, stop
before code changes unless the user explicitly authorizes a scoped top-level
edit.

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
