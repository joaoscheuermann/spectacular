# Effort: worker prompt agent runner

Status: todo

## Requirement links

- Features: F-07, F-10, F-11, F-12
- PRD: FR-8, FR-9, FR-10, FR-12, FR-13; AC-7, AC-10, AC-11, AC-12, AC-15
- TDD: worker prompt-agent execution, prompt-agent-only scope, worker internal state, prompt runner trait

## Goal

Implement the worker prompt/requirements-agent runner seam, lifecycle event mapping, and `artifacts/PROMPT.md` output without claiming later Doric phases.

## Sequence

- Position: 11 of 15
- Previous effort: 10_worker_tooling_registration.md
- Enables: worker runtime can execute the v1 prompt workflow through a fakeable runner and emit prompt-only lifecycle events.

## Target files

- `packages/worker/src/lib.rs`
- `packages/worker/src/agents/mod.rs`
- `packages/worker/src/agents/prompt.rs`
- `packages/worker/src/event.rs`
- `packages/worker/src/state.rs`
- `packages/worker/src/error.rs`
- `packages/worker/tests/unit/prompt_agent.rs`
- `packages/worker/tests/unit/event.rs`

## Coupled files

- `packages/agent/src/lib.rs`, `agent.rs`, `event.rs`, `store.rs`, and `tool.rs` are prompt runner context.
- `packages/worker/src/provider.rs` and `tooling.rs` provide provider/tools for production prompt runs.
- `packages/lifecycle/src/event.rs` supplies lifecycle event types.

## Ownership

- Intended worker write scope: worker prompt-agent modules, event mapping, state artifact helpers, and worker tests.
- Read-only context: `agent`, `llms`, `tools`, and `lifecycle` package APIs.
- Known conflict risks: event names are user-visible and acceptance-bound. Keep them explicitly prompt/requirements-only.

## Tests to add or update

- Fake prompt runner tests for prompt-agent started, prompt artifact written, input requested, answer consumed, prompt-agent completed, and prompt-agent failed events.
- Artifact tests proving `PROMPT.md` is written under `<worker-root>/<id>/artifacts/`.
- Scope tests proving event/output names do not mention PRD, TDD, decomposition, implementation, tests, or handover as completed work.
- Agent config/reasoning/context-policy mapping tests if production runner composes `agent::Agent`.

## Regression suites

- `cargo test -p worker --no-fail-fast`
- `cargo test -p agent --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`

## Acceptance criteria

- The only v1 workflow agent is prompt/requirements.
- Production prompt runner can be built from worker-local provider composition and worker tool registration.
- Tests can use fake prompt runners without live LLMs or network repos.
- Human input requests include request text and a correlation handle suitable for daemon-mediated answer routing.
- Successful output identifies prompt/requirements work only.

## Notes

- Keep full PRD, technical design, decomposition, implementation, validation, and handover execution out of scope.
- This effort does not yet own the worker session loop; that is the next slice.
