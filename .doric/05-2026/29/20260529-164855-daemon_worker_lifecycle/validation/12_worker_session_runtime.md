# Validation: worker session runtime

## Red evidence

- `cargo test -p worker runtime --no-fail-fast`
  - Exit code: 1
  - Expected red failure:
    - `error[E0432]: unresolved import worker::runtime` at `packages\worker\tests\unit\runtime_support.rs:10:13`
    - `error[E0432]: unresolved import worker::runtime` at `packages\worker\tests\unit\runtime.rs:4:13`
    - Compiler detail: `could not find runtime in worker`
  - Interpretation: the focused runtime tests were discovered through the worker unit harness, and compilation stops at the intended future `worker::runtime` API boundary before any live daemon, socket, LLM, or network repo work can run.
- Blank-token repair red:
  - `cargo test -p worker runtime --no-fail-fast`
  - Exit code: 1
  - Expected red failure:
    - `runtime::run_worker_session_blank_token_preserves_outbound_status_and_event_text ... FAILED`
    - Assertion failed because the outbound messages no longer contained `Worker session running` after empty-token replacement corrupted every message.
  - Interpretation: the regression reached the public runtime path and reproduced the reviewer blocker before the sanitizer repair.

## Green evidence

- `cargo test -p worker runtime --no-fail-fast`
  - Exit code: 0
  - Evidence: 20 runtime-filtered worker tests passed after the blank-token sanitizer repair.
  - Covers attach/start ordering, shutdown before `StartJob`, repo-before-prompt ordering, repo failure redaction, prompt event sequencing, input wait/resume, answer-before-wait redaction, shutdown stopped mapping, running-without-pending-input outcome, prompt failure redaction, and terminal status ordering.
- `cargo test -p worker --no-fail-fast`
  - Exit code: 0
  - Evidence: 59 worker unit tests passed; worker lib/main/doc test targets passed.
- `cargo build -p worker --bin doric-worker`
  - Exit code: 0
  - Evidence: worker binary target built successfully.
- `cargo clippy -p worker --all-targets -- -D warnings`
  - Exit code: 0
  - Evidence: worker clippy completed with warnings denied.
- `cargo fmt --all -- --check`
  - Exit code: 0
  - Evidence: workspace formatting check passed.
- `cargo test -p daemon --no-fail-fast`
  - Exit code: 0
  - Evidence: 58 daemon unit tests passed; daemon lib/main/doc test targets passed.
- `cargo test -p lifecycle --no-fail-fast`
  - Exit code: 0
  - Evidence: 19 lifecycle tests passed; lifecycle doc tests passed.
- `npx nx run worker:build`
  - Exit code: 0
  - Evidence: Nx worker build target ran `cargo check --target-dir dist/target/worker -p worker` and succeeded.
- `npx nx run worker:test`
  - Exit code: 0
  - Evidence: Nx worker test target succeeded with cached output for `cargo test --target-dir dist/target/worker -p worker`; cached evidence shows 58 worker unit tests passed.

## Static validation

- Added `packages/worker/tests/unit/runtime.rs` and `packages/worker/tests/unit/runtime_support.rs`, then wired both from `packages/worker/tests/unit.rs`.
- Blank-token repair:
  - `packages/worker/src/event.rs` now redacts provider/repo secrets first, then replaces the runtime token only when `token` is non-empty.
  - `packages/worker/tests/unit/runtime.rs` covers a blank runtime token through `run_worker_session` and asserts ordinary outbound status/event messages remain intact and do not contain `[REDACTED]`.
  - `packages/worker/tests/unit/runtime_support.rs` adds a token-configurable fixture path and status/event message collection helper.
- File-size gate corrected:
  - `packages/worker/src/runtime.rs`: 404 lines.
  - `packages/worker/src/event.rs`: 122 lines.
  - `packages/worker/src/error.rs`: 240 lines.
  - `packages/worker/src/state.rs`: 151 lines.
  - `packages/worker/src/repo.rs`: 193 lines.
  - `packages/worker/src/agents/prompt.rs`: 221 lines.
  - `packages/worker/tests/unit.rs`: 20 lines.
  - `packages/worker/tests/unit/runtime.rs`: 280 lines.
  - `packages/worker/tests/unit/runtime_support.rs`: 438 lines.
- The red tests target a future `worker::runtime` module with explicit fakeable dependencies:
  - `DaemonSession` fake for inbound daemon frames and captured outbound worker frames.
  - `RepoPreparer` fake above Git/repo preparation.
  - `RuntimePromptRunner` fake for prompt events, waits, answers, failures, and shutdown.
- Tests use generated lifecycle proto frames from `lifecycle::proto::doric::lifecycle::v1` and remain offline.
- Tests avoid direct CLI command handling, daemon process spawning, socket binding, live LLM calls, and network repository cloning.
- Tests preserve non-empty token redaction through `run_worker_session_prompt_failure_sends_failed_terminal_status_and_redacted_reason`, which still asserts `one-time-token` is absent from outbound status/event text.
- Source/event scope scan:
  - `rg -n "prd|tdd|technical_design|technical design|decomposition|implementation|handover|validation|test" packages/worker/src/runtime.rs packages/worker/src/event.rs packages/worker/src/state.rs packages/worker/src/main.rs packages/worker/tests/unit/runtime.rs packages/worker/tests/unit/runtime_support.rs`
  - Later-phase terms appear only in Rust `#[test]` attributes and test fixture strings, not in runtime source event names or source messages.
- Runtime source and test assertions now prove:
  - shutdown before `StartJob` returns `RuntimeOutcome::Stopped` with a single stopped terminal status and no repo/prompt execution.
  - `PromptRunState::Running` with no pending input and no daemon frame returns `RuntimeOutcome::Running`, not `WaitingForStartJob`.
  - terminal status is sent at most once and no status follows terminal in the focused success/failure assertion.
  - answer text that arrives before wait is not forwarded to the prompt runner and does not appear in outbound frames.
  - worker token, provider API-key-like values, OAuth token labels, repo credentials, and early answer text are absent from outbound event/status text.
- `packages/worker/src/main.rs` remains `fn main() {}`. This is accepted for this effort because the validator prompt explicitly forbids implementing real gRPC production wiring, the binary builds, and the runtime library behavior is covered through fakeable daemon/session traits. Operational `doric-worker` gRPC attachment remains a residual integration risk for a later wiring effort.

## Focused command

- `cargo test -p worker runtime --no-fail-fast`
  - Final exit code: 0.
  - Final evidence: 20 runtime-filtered worker tests passed.

## Regression commands

- `cargo test -p worker --no-fail-fast`
  - Final exit code: 0.
  - Final evidence: 59 worker tests passed.
- `cargo build -p worker --bin doric-worker`
- `cargo clippy -p worker --all-targets -- -D warnings`
  - Final exit code: 0.
- `cargo fmt --all -- --check`
  - Final exit code: 0.
- `cargo test -p daemon --no-fail-fast`
- `cargo test -p lifecycle --no-fail-fast`
- `npx nx run worker:build`
- `npx nx run worker:test`

## Post-repair validation refresh

- Post-repair validator reran all requested commands after `agents/128_effort_12_blank_token_redaction_repair.md`.
- `cargo test -p worker runtime --no-fail-fast`
  - Exit code: 0.
  - Evidence: 20 runtime-filtered worker tests passed, including `runtime::run_worker_session_blank_token_preserves_outbound_status_and_event_text` and `runtime::run_worker_session_prompt_failure_sends_failed_terminal_status_and_redacted_reason`.
  - Interpretation: blank runtime tokens no longer corrupt ordinary outbound status/event text, and non-empty runtime-token redaction remains covered.
- `cargo test -p worker --no-fail-fast`
  - Exit code: 0.
  - Evidence: 59 worker tests passed.
- `cargo build -p worker --bin doric-worker`
  - Exit code: 0.
  - Evidence: worker binary target built successfully.
- `cargo clippy -p worker --all-targets -- -D warnings`
  - Exit code: 0.
  - Evidence: worker clippy completed with warnings denied.
- `cargo fmt --all -- --check`
  - Exit code: 0.
  - Evidence: workspace formatting check passed.
- `cargo test -p daemon --no-fail-fast`
  - Exit code: 0.
  - Evidence: 58 daemon unit tests passed; daemon lib/main/doc test targets passed.
- `cargo test -p lifecycle --no-fail-fast`
  - Exit code: 0.
  - Evidence: 19 lifecycle tests passed; lifecycle doc tests passed.
- `npx nx run worker:build`
  - Exit code: 0.
  - Evidence: Nx worker build target ran `cargo check --target-dir dist/target/worker -p worker` and succeeded.
- `npx nx run worker:test`
  - Exit code: 0.
  - Evidence: Nx worker test target ran `cargo test --target-dir dist/target/worker -p worker` and passed 59 worker tests.
- File-size refresh:
  - `packages/worker/src/event.rs`: 122 lines.
  - `packages/worker/src/runtime.rs`: 404 lines.
  - `packages/worker/src/state.rs`: 151 lines.
  - `packages/worker/tests/unit/runtime.rs`: 280 lines.
  - `packages/worker/tests/unit/runtime_support.rs`: 438 lines.

## Unavailable tooling

- `rustfmt packages/worker/tests/unit.rs packages/worker/tests/unit/runtime.rs`
  - Exit code: 1
  - Formatting the harness directly caused rustfmt to parse included async tests as Rust 2015 and fail in `packages\worker\tests\unit\tooling.rs`.
- `rustfmt --edition 2024 packages/worker/tests/unit/runtime.rs`
  - Exit code: 0
  - Runtime test file formatted successfully.
- `rustfmt --edition 2024 packages/worker/tests/unit/runtime.rs packages/worker/tests/unit/runtime_support.rs`
  - Exit code: 0
  - Runtime test and support files formatted successfully.

## Refactors applied

- Split runtime helpers and fakes from `packages/worker/tests/unit/runtime.rs` into `packages/worker/tests/unit/runtime_support.rs` to satisfy the 500-line coding-conventions hard threshold.
- Added `RuntimeOutcome::Running` and a `StartJobWait` internal enum so distinct nonterminal states do not collapse into `WaitingForStartJob`.
- Changed pre-start shutdown handling to return stopped through the same terminal path instead of emitting stopped frames while reporting `WaitingForStartJob`.
- Added focused runtime tests for pre-start shutdown and running-without-pending-input.
- Added a focused terminal-ordering assertion proving no status frame follows terminal success/failure in the covered terminal cases.

## Reviewer decision

green-ready
