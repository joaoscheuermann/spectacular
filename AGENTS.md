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
3. The relevant skill under `.agents/skills/`.
4. Nearby source, tests, manifests, schemas, generated contracts, or Doric run
   artifacts needed for the task.

Read only enough context to act correctly. Broaden the search when evidence is
missing or the task crosses package or workflow boundaries.

## Skill Routing

Use repository skills when their trigger matches the task:

- `.agents/skills/coding-conventions/SKILL.md` for implementation and testing
  standards.
- `.agents/skills/doric/SKILL.md` for Doric feature workflows, sub-agent
  coordination, PRD/TDD/decomposition/development/handover artifacts,
  harness-owned workflow gates, generated `STATE.md` audit projections,
  required-agent receipts, validation records, or effort checkpoints.
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
- Prefer existing package patterns, helper APIs, and tests before adding new
  abstractions.
- Use `rg` or the fastest available local search tool first. Use a bounded
  fallback if it is unavailable.
- Use `apply_patch` for manual file edits.
- Do not use destructive commands, broad rewrites, or rollback operations
  unless the user explicitly approves them.
- Do not silently skip validation. Run the relevant checks or state why they
  could not be run.

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

## Doric Workflow Rules

When the user asks to run or continue a Doric feature workflow:

- Load `.agents/skills/doric/SKILL.md`.
- Treat harness-managed structured state/events as the target authority for
  legal workflow transitions once the harness exists. Until then, use
  `STATE.md` as the durable bootstrap projection/audit ledger and do not claim
  current code already enforces harness transitions.
- Use the live `.doric/<MM-YYYY>/<DD>/<timestamp>-<feature_name>/` run
  directory when one exists.
- Read harness state/events when available, plus generated `STATE.md`, current
  phase, approvals, required-agent rows, agent receipts, effort order,
  validation records, active locks, and worktree status before choosing the
  next action.
- Do not skip legal phase transitions.
- Stop after `PROMPT.md` to raise open questions and record
  `prompt_to_prd_alignment` before PRD generation.
- Do not advance gated phases while required-agent rows are pending, spawned,
  blocked, or rejected.
- Process development efforts strictly in numeric order.
- Commit only approved effort-owned changes and required Doric artifacts when
  an effort checkpoint requires a commit.

## Sub-Agent Handoffs

When spawning or instructing a sub-agent, pass explicit context instead of
assuming hidden conversation state.

Include:

- `GROUNDING.md`.
- This `AGENTS.md`.
- Relevant skill paths.
- The Doric run directory and artifact paths when applicable.
- The specific read scope, write scope, output path, validation expectation,
  and stop condition.
- A reminder that the sub-agent is not alone in the codebase and must not
  revert edits made by others.

If the required sub-agent mechanism is unavailable for a gated Doric role,
record the role as blocked through canonical workflow state and ensure the
`STATE.md` projection shows it; until the harness exists, mark the role blocked
in `STATE.md` and stop. Do not fabricate sub-agent proof.

## Architecture Work

For architecture-sensitive work:

- Treat `GROUNDING.md` as the architecture overview and validity contract.
- Verify against source and manifests when implementation details are unclear
  or when a task may change package responsibilities.
- Keep dependency direction explicit.
- Update `GROUNDING.md` when the task intentionally changes package
  responsibilities, dependency direction, provider composition, built-in tools,
  lifecycle command behavior, event streaming, user-input request handling,
  session persistence, config schema, lifecycle contracts, daemon or worker
  behavior, or vendored dependency strategy.

## Validation

Use the narrowest reliable proof for the change:

- Docs-only changes: check Markdown hygiene and `git diff --check` when useful.
- Rust changes: prefer the relevant `cargo fmt`, `cargo test`, `cargo clippy`,
  `cargo nextest`, or package-specific command.
- Nx/package changes: prefer documented `npx nx` targets.
- Doric workflow changes: validate harness gates when implemented, generated
  `STATE.md` projection, required-agent receipts, effort order, and artifact
  consistency.

If validation is blocked by missing tools, sandbox limits, external services, or
time, report the blocker and residual risk.

## Final Response Expectations

Summarize what changed, name the files touched, and report validation. Keep the
answer concise. If work could not be completed, state the blocker and the next
safe step.
