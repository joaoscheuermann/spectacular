# Agent Instructions

All agents and sub-agents working in this repository must read
`GROUNDING.md` before planning, reviewing, generating artifacts, or editing
files. This file is the project operating contract. `GROUNDING.md` is the
validity contract.

If an agent cannot read `GROUNDING.md`, it must stop and report that the
repository grounding contract cannot be satisfied.

## Required Read Order

For non-trivial work, use this order:

1. `GROUNDING.md`.
2. This `AGENTS.md`.
3. The relevant skill under `.agents/skills/`, when its trigger matches the
   task.
4. Nearby source, tests, manifests, schemas, generated contracts, or artifacts
   needed for the task.

Read only enough context to act correctly. Broaden the search when evidence is
missing or the task crosses package, runtime, or workflow boundaries.

## Skill Routing

Use repository skills when their trigger matches the task:

- `.agents/skills/coding-conventions/SKILL.md` for implementation and testing
  standards.
- `.agents/skills/agent-skill-authoring/SKILL.md` for creating or changing
  skills.

If multiple skills apply, load the smallest set that covers the task and say
which ones are being followed.

## Operating Contract

- Work in files when the user asks for repository deliverables.
- Inspect the real local files before making architecture or implementation
  claims.
- Preserve unrelated dirty worktree changes.
- Keep edits scoped to the user request and the relevant ownership boundary.
- For code updates, follow the Sub-Agent Handoffs rule: the main agent
  orchestrates while sub-agents perform implementation and focused validation.
- Treat this repository as a TypeScript agent-core project unless the user
  explicitly expands the scope.
- Prefer existing package patterns, helper APIs, and tests before adding new
  abstractions.
- Use `rg` or the fastest available local search tool first. Use a bounded
  fallback if it is unavailable.
- Use `apply_patch` for manual file edits.
- Do not use destructive commands, broad rewrites, or rollback operations
  unless the user explicitly approves them.
- Do not silently skip validation. Run the relevant checks or state why they
  could not be run.

## Agent Work Sequence

For implementation-bearing work, agents should follow this sequence:

1. Explore and understand the problem before proposing or editing. Read the
   required grounding, instructions, relevant skill files, manifests, nearby
   source, tests, and current behavior needed to identify constraints,
   uncertainty, and the real ownership boundary.
2. Create the architecture and define the implementation goals. State the
   target shape, dependency direction, output files, validation proof,
   non-goals, and stop conditions before handing off work. Keep the plan within
   the current TypeScript agent-core scope unless the user explicitly expands
   it.
3. Handoff and orchestrate development to one or more sub-agents when code is
   being changed. Give each sub-agent a bounded implementation assignment with
   explicit context, read scope, write scope, validation expectation, and stop
   condition. Coordinate sequencing, review returned work, and resolve
   conflicts at the main-agent level.
4. After implementation, orchestrate one or more focused review or validation
   handoffs. These handoffs must review the code against
   `.agents/skills/coding-conventions/SKILL.md`, the implementation goals,
   `GROUNDING.md`, this `AGENTS.md`, and the relevant tests or configured
   checks before the main agent claims completion.

## Grounding Enforcement

Hard Constraints in `GROUNDING.md` are gates. If a request conflicts with a
Hard Constraint:

1. Cite the HC ID.
2. Explain the conflict.
3. Stop before violating it.
4. Ask the user only when the constraint permits a scoped human decision.

Convention Parameters in `GROUNDING.md` are defaults. Follow them when
practical. When deviating, record the reason in the artifact, state ledger, or
final response when useful.

## Architecture Work

For architecture-sensitive work:

- Treat `GROUNDING.md` as the architecture overview and validity contract.
- Verify against current source and manifests before making implementation
  claims.
- Keep the near-term product boundary explicit: TypeScript agent core only.
- Treat CLI, daemon, worker, lifecycle service, TUI, slash-command, Rust
  package, service, and multi-process architecture terminology as out-of-scope
  markers only, unless the user explicitly expands the product scope.
- Do not reintroduce CLI, daemon, worker, lifecycle service, TUI, slash-command,
  Rust package, or multi-process architecture unless the user explicitly asks
  for that scope.
- Keep dependency direction explicit when adding TypeScript modules or packages.
- Update `GROUNDING.md` when the task intentionally changes product scope,
  package responsibilities, dependency direction, provider composition,
  built-in tools, event streaming, user-input handling, session persistence,
  config schema, runtime behavior, or vendored dependency strategy.

## Nx Monorepo Work

When adding or changing TypeScript packages:

- Inspect `package.json`, `nx.json`, `tsconfig.base.json`, root
  `tsconfig.json`, and existing `packages/*` projects before making package
  claims.
- Treat `packages/<name>` as the Nx workspace location for product packages.
  Do not add root-level product packages. Old Rust package names are out of
  scope unless the user explicitly asks for that scope.
- Create packages with the `@nx/js` generator when available, for example
  `npx nx generate @nx/js:library packages/<name> --bundler=tsc --config=project`.
- After generation, inspect and keep the package-local `project.json`,
  `package.json`, and `tsconfig*.json` files coherent with the root solution
  `tsconfig.json`.
- Give each new package one clear responsibility that is deeper than a folder:
  a stable public contract, a test boundary, a dependency-direction boundary,
  or a proven second consumer.
- Export package APIs through `src/index.ts` and consume sibling packages by
  their package entrypoint, not by `../` paths or deep imports into private
  source files.
- Declare workspace package dependencies in the consuming package metadata
  when metadata exists. Prefer Nx project references and package metadata over
  ad hoc root `compilerOptions.paths` aliases.
- Run `npx nx sync` when TypeScript project references need reconciliation, and
  validate package changes with `npx nx show projects` plus the affected
  `typecheck`, `test`, or `build` targets.

## Sub-Agent Handoffs

For tasks that update code, explicitly delegate implementation work to
sub-agents. The main agent's role is orchestration: define bounded assignments,
pass the required context, coordinate sequencing, review returned changes,
resolve conflicts, and synthesize the final result. The main agent should not
act as the primary code editor for code updates.

Code-update delegation must cover both the implementation assignment and the
focused validation or review assignment, even when the change is narrow. The
review assignment must explicitly use
`.agents/skills/coding-conventions/SKILL.md`. If a sub-agent mechanism is
unavailable, blocked, or unsafe, stop before making code changes and report the
blocker unless the user explicitly authorizes a scoped main-agent code edit.

When spawning or instructing a sub-agent, pass explicit context instead of
assuming hidden conversation state.

Every handoff must explicitly tell the child agent that it is the sub-agent
for that handoff and that its role is to execute the handoff assignment
described by the main agent. A sub-agent is not responsible for orchestrating
the overall task, spawning further sub-agents, or satisfying main-agent-only
handoff rules unless the main agent explicitly assigns that responsibility.

The sub-agent must still follow `GROUNDING.md`, the assignment scope, worktree
safety rules, validation requirements, and any task-relevant skills or nearby
source instructions included in the handoff.

Include:

- `GROUNDING.md`.
- This `AGENTS.md`.
- Relevant skill paths.
- The specific read scope, write scope, output path, validation expectation,
  and stop condition.
- A reminder that the sub-agent is not alone in the codebase and must not
  revert edits made by others.

If a required sub-agent mechanism is unavailable, do not fabricate proof of
sub-agent execution. Record the blocker and continue only when the task can be
completed safely without that proof.

## Validation

Use the narrowest reliable proof for the change:

- Docs-only changes: check Markdown hygiene and `git diff --check` when useful.
- TypeScript changes: prefer configured Nx targets such as
  `npx nx run <project>:typecheck`, `npx nx test <project>`, or
  `npx nx build <project>`.
- Nx/package changes: prefer documented `npx nx` targets and `npx nx show
projects`.
- Formatting changes: use the repository formatter when configured.
- Rust checks are only relevant when the task explicitly targets legacy Rust
  artifacts that still exist.

If validation is blocked by missing tools, sandbox limits, external integrations,
or time, report the blocker and residual risk.

## Final Response Expectations

Summarize what changed, name the files touched, and report validation. Keep the
answer concise. If work could not be completed, state the blocker and the next
safe step.
