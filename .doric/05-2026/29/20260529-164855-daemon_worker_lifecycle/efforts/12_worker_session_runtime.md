# Effort: worker session runtime

Status: done

## Requirement links

- Features: F-04, F-06, F-07, F-08, F-10, F-11
- PRD: FR-6, FR-8, FR-9, FR-10, FR-12, FR-13; AC-7, AC-9, AC-10, AC-11, AC-12, AC-15
- TDD: worker process launch and authenticated daemon session, worker runtime, daemon command stream, terminal status mapping

## Goal

Implement the worker runtime loop that attaches to the daemon, receives `StartJob`, prepares the repo, runs the prompt agent, waits for daemon answers, and emits terminal status.

## Sequence

- Position: 12 of 15
- Previous effort: 11_worker_prompt_agent_runner.md
- Enables: daemon process/session and worker internals can operate together through the selected gRPC protocol.

## Target files

- `packages/worker/Cargo.toml`
- `packages/worker/src/lib.rs`
- `packages/worker/src/runtime.rs`
- `packages/worker/src/main.rs`
- `packages/worker/src/event.rs`
- `packages/worker/src/error.rs`
- `packages/worker/src/state.rs`
- `packages/worker/tests/unit/runtime.rs`
- `Cargo.lock`

## Coupled files

- `packages/daemon/src/worker_session.rs` is the server-side session counterpart.
- `packages/lifecycle/proto/doric/lifecycle/v1.proto` is the generated protocol dependency.
- `packages/worker/src/repo.rs`, `provider.rs`, `tooling.rs`, and `agents/prompt.rs` are direct runtime dependencies.

## Ownership

- Intended worker write scope: worker runtime/main/event/error/state modules, runtime tests, and worker manifest updates.
- Read-only context: daemon worker-session implementation and lifecycle proto contract.
- Known conflict risks: worker runtime spans several worker modules. Keep orchestration thin and push testable behavior to existing repo/provider/tooling/prompt seams.

## Tests to add or update

- Fake daemon command stream tests for attach, `StartJob`, input answer, shutdown, and terminal status.
- Runtime success test with fake repo preparer, fake prompt runner, fake provider/tooling, and no live LLM/network.
- Repo failure, provider failure, prompt-agent failure, worker crash/abort, and answer-before-wait tests.
- Tests proving provider secrets and worker tokens are not emitted in lifecycle events.

## Regression suites

- `cargo test -p worker --no-fail-fast`
- `cargo build -p worker --bin doric-worker`
- `cargo test -p daemon --no-fail-fast`
- `cargo clippy -p worker --all-targets -- -D warnings`
- `npx nx run worker:build`
- `npx nx run worker:test`

## Acceptance criteria

- Worker attaches to daemon using worker id and one-time token, then waits for daemon-controlled job payload.
- Repo preparation runs before prompt-agent execution and failures map to redacted lifecycle failure events.
- Prompt-agent events, input waits, answer continuation, success, failure, and stopped states are sent over the daemon session.
- Worker runtime does not bind its own public server or accept direct CLI commands.
- Worker output and lifecycle events remain prompt/requirements-only in v1.

## Notes

- Do not move daemon registry authority into the worker. The worker emits facts; the daemon remains the source of truth for CLI-visible state.
- Keep worker debug logging worker-scoped and disabled or redacted by default.
