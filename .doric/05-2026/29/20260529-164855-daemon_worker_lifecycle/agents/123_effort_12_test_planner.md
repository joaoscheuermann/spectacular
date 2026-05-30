# Agent Receipt: effort 12 test planner

## Role

Development test planner for `efforts/12_worker_session_runtime.md`.

## Spawn proof

- Tool: `multi_agent_v1.spawn_agent`
- Agent type: `worker`
- Agent id: `019e773e-f008-7182-90c6-9634626bb38f`
- Required-agent row: `STATE.md` lists development effort `efforts/12_worker_session_runtime.md`, role `test planner`, receipt `agents/123_effort_12_test_planner.md`, status `spawned`.
- Cursor proof: `STATE.md` current effort is `efforts/12_worker_session_runtime.md`, next effort index `11`, and effort 12 is `in-progress`.
- Prior-effort proof: `agents/121_effort_11_done_transition.md` records effort 11 accepted with commit `c05137ed98c07b643c4fd62e9c9ad5a0dae39e61`.

## Inputs read

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/coding-conventions/references/implementation-standards.md`
- `.agents/skills/coding-conventions/references/sexy-rust.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/12_worker_session_runtime.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/FEATURES.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/121_effort_11_done_transition.md`
- `packages/worker/src/lib.rs`
- `packages/worker/src/main.rs`
- `packages/worker/src/error.rs`
- `packages/worker/src/state.rs`
- `packages/worker/src/event.rs`
- `packages/worker/src/repo.rs`
- `packages/worker/src/provider.rs`
- `packages/worker/src/tooling.rs`
- `packages/worker/src/agents/prompt.rs`
- `packages/daemon/src/worker_session.rs`
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- `packages/lifecycle/src/proto.rs`
- Existing tests under `packages/worker/tests/unit*`
- Existing daemon counterpart test `packages/daemon/tests/unit/worker_session.rs`

## Existing seam snapshot

- `packages/worker/src/lib.rs` exports `agents`, `error`, `event`, `provider`, `repo`, `state`, and `tooling`; it does not yet export `runtime`.
- `packages/worker/src/main.rs` is still an empty `fn main() {}`.
- Worker repo preparation already has a fakeable `GitCommandRunner`, but runtime tests should force a higher-level `RepoPreparer` seam so session orchestration does not invoke Git or the network.
- Worker prompt support already has `PromptAgentRunner`, `PromptAgentOrchestrator`, `PromptJob`, `PromptAgentEvent`, and lifecycle event mapping. Effort 12 tests should force a session-oriented runner seam that can pause for daemon input and continue with answers.
- Worker provider and tooling composition are already separately tested. Runtime tests should not exercise live providers, model APIs, shared tool execution, or network clones.
- Daemon `SessionManager` already validates attach tokens, returns `StartJob`, forwards answers, records worker frames, closes sessions on terminal status, and maps child exit status. Worker runtime tests should use a fake daemon session instead of a real daemon process.
- The lifecycle proto already defines `WorkerSessionService`, `WorkerFrame`, `DaemonFrame`, `StartJob`, `AnswerInputCommand`, and `ShutdownWorker`.

## Test target files

- Primary red file: `packages/worker/tests/unit/runtime.rs`
- Worker harness update required by the next test-writer: `packages/worker/tests/unit.rs` with `#[path = "unit/runtime.rs"] mod runtime;`
- Production seams the tests should compile against once introduced: `packages/worker/src/runtime.rs`, `packages/worker/src/lib.rs`, and minimal runtime support in `packages/worker/src/error.rs`, `packages/worker/src/event.rs`, `packages/worker/src/state.rs`, and `packages/worker/src/main.rs` as needed.
- Daemon counterpart regression target only if effort 12 needs a daemon-side shutdown command seam: `packages/daemon/tests/unit/worker_session.rs`. Do not broaden daemon source edits unless the runtime implementation cannot prove shutdown/stopped behavior through fake inbound `ShutdownWorker` frames alone.

## Fake dependencies for runtime tests

The next test-writer should define these fakes in `packages/worker/tests/unit/runtime.rs`:

- `FakeDaemonSession`: captures outbound `pb::WorkerFrame` values, provides inbound `pb::DaemonFrame` values in deterministic order, and never opens a socket.
- `FakeRepoPreparer`: records start-job repo/root inputs, returns a prepared `WorkerLayout`, and can fail with a redacted `WorkerError`.
- `FakePromptRunner`: emits deterministic prompt events, records the prepared layout and prompt, can pause on `InputRequested`, can continue only with a matching `AnswerInputCommand`, and can fail with secret-bearing text for redaction tests.
- `FrameLog` or equivalent helper: asserts outbound frame order without depending on internal runtime state.
- `TempRoot`: creates isolated temp worker roots under `std::env::temp_dir()` and deletes them on drop.

These are test doubles for injected production traits. They should not spawn a daemon, bind ports, call a live LLM, read user config, run real tools, or clone a network repo.

## Production seams forced by tests

- `worker::runtime` public module exported from `lib.rs`.
- A small session entry point such as `run_worker_session(config, deps)` or `WorkerRuntime::run()` that receives dependencies explicitly.
- `WorkerSession` or `DaemonSession` trait with operations equivalent to:
  - send `WorkerHello`
  - receive `DaemonFrame`
  - send lifecycle `WorkerEvent`
  - send `WorkerStatusUpdate`
- `RepoPreparer` trait above `prepare_worker_repo`, so runtime tests can prove ordering without invoking `git`.
- Session-oriented prompt runner seam that supports event streaming and input continuation. The current one-shot `PromptAgentRunner` may remain for prompt-agent tests, but runtime needs a fakeable boundary that can pause on `InputRequested` and resume from daemon answers.
- Runtime event/status sink with monotonic sequence ownership, so prompt events sent to the daemon are ordered and terminal status is sent exactly once.
- Runtime-level redaction before any worker frame leaves the worker. Existing `WorkerError` and lifecycle redaction helpers should be reused rather than duplicating string rules.
- Runtime shutdown/cancellation path that maps inbound `ShutdownWorker` to stopped status and prevents later success/failure frames.
- `doric-worker` main composition root that parses worker id/token/daemon address/root only after library behavior is tested. Direct CLI commands remain out of scope.

## Red-test order

1. `run_worker_session_valid_attach_waits_for_start_job_before_repo_preparation`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakeDaemonSession` with no initial `StartJob`, `FakeRepoPreparer`, `FakePromptRunner`.
   - Arrange: runtime config includes `worker_id = worker-runtime-attach`, `token = one-time-token`, daemon session starts empty.
   - Act: drive the runtime one polling step or run until blocked on daemon command.
   - Assert: first outbound frame is `WorkerHello` with worker id and token; repo preparer and prompt runner have not been called before `StartJob`; no emitted event/message includes the token.
   - Expected red: compile failure because `worker::runtime` and the session dependency traits do not exist. After the seam compiles, expected assertion failure is repo or prompt execution starting before daemon-controlled `StartJob`.

2. `run_worker_session_start_job_prepares_repo_before_prompt_runner`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakeDaemonSession` yields one `StartJob`, `FakeRepoPreparer` returns a prepared layout, `FakePromptRunner` records the received `PromptJob`.
   - Assert: repo preparation receives the daemon job repo and worker root before prompt execution; prompt runner receives the prepared `repo/`, `state/`, `artifacts/`, and `tool-output/` layout; outbound frames include running/repo-preparation activity before prompt-agent events.
   - Expected red: either missing runtime seam or prompt runner called without the prepared repo layout.

3. `run_worker_session_repo_preparation_failure_sends_failed_status_without_prompt_runner`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakeRepoPreparer` returns `WorkerError::git_failed(128, "fatal token sk-secret in https://user:pass@example.com/org/repo.git", repo_url)`.
   - Assert: prompt runner is not called; worker sends a failed terminal `WorkerStatusUpdate` and a failure event; repo credentials and `sk-secret` are absent from all outbound frames.
   - Expected red: runtime does not yet map repo failures into terminal frames, or failure text leaks raw credentials.

4. `run_worker_session_prompt_events_stream_to_daemon_in_sequence`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakePromptRunner` emits `Started`, `ArtifactWritten`, and `Completed`.
   - Assert: outbound `WorkerEvent` frames preserve names `prompt_agent_started`, `prompt_artifact_written`, `prompt_agent_completed`; sequence values are monotonic starting at the runtime-owned base; terminal status is `Succeeded` exactly once after completed.
   - Expected red: no runtime event sink/sequence ownership, or terminal success is not sent after prompt completion.

5. `run_worker_session_input_request_waits_for_matching_answer_before_continuing`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakePromptRunner` emits `InputRequested(request-runtime-1, "...?")` and blocks; `FakeDaemonSession` later yields `AnswerInputCommand { request_id: "request-runtime-1", text: "Use repo-local skills." }`.
   - Assert: runtime sends a waiting-for-input event/status, does not complete before the answer, resumes the prompt with the matching answer, emits `prompt_answer_consumed`, and then succeeds.
   - Expected red: current prompt runner seam is one-shot and cannot pause/resume through daemon commands.

6. `run_worker_session_answer_before_wait_does_not_continue_prompt_or_leak_text`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakeDaemonSession` yields `AnswerInputCommand` before any `InputRequested`; `FakePromptRunner` records supplied answers.
   - Assert: unexpected answer is ignored or reported as a redacted protocol failure, but it is not injected into the prompt job and does not produce a false continuation event. Choose the exact production behavior in the code-writer phase; the key contract is no answer-before-wait continuation.
   - Expected red: no runtime command-state guard exists.

7. `run_worker_session_shutdown_before_terminal_prompt_sends_stopped_status`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakeDaemonSession` yields `StartJob`, prompt runner starts, then daemon yields `ShutdownWorker { reason: "daemon shutdown" }`.
   - Assert: runtime cancels or stops prompt execution, sends `Stopped` terminal status/event once, closes the session loop, and does not later emit `Succeeded` or `Failed`.
   - Expected red: no runtime shutdown handling exists.

8. `run_worker_session_prompt_failure_sends_failed_terminal_status_and_redacted_reason`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: `FakePromptRunner` fails with text containing a provider API key-like value, an OAuth `access_token`, and a worker attach token.
   - Assert: outbound terminal status is `Failed`, lifecycle failure event is prompt/requirements-scoped, and all secret-looking values are redacted.
   - Expected red: no runtime prompt-failure mapping or runtime-level redaction.

9. `run_worker_session_success_and_failure_send_one_terminal_status`
   - Target: `packages/worker/tests/unit/runtime.rs`
   - Fakes: one subcase succeeds, one subcase fails.
   - Assert: each run sends exactly one terminal status and no later nonterminal status after terminal.
   - Expected red: terminal status is missing, duplicated, or overwritten after completion.

10. `record_worker_event_input_requested_marks_waiting_and_preserves_correlation`
    - Target: `packages/daemon/tests/unit/worker_session.rs` only if daemon counterpart regression is allowed for effort 12.
    - Fakes: existing `SessionFixture`; send `pb::WorkerFrame::Event` with `InputRequest`.
    - Assert: registry summary becomes `WaitingForInput`, pending request id is answerable through `SessionCommandSender`, and answer command is forwarded only to an attached session.
    - Expected red: this may already be green through existing `record_event`; keep it as counterpart coverage, not a blocker for the first worker-runtime red.

11. `send_shutdown_attached_worker_forwards_shutdown_command`
    - Target: `packages/daemon/tests/unit/worker_session.rs` only if production needs daemon-side shutdown command dispatch in this effort.
    - Fakes: existing `SessionFixture`.
    - Assert: attached worker receives `pb::DaemonFrame::ShutdownWorker` and detached worker returns session unavailable.
    - Expected red: no daemon shutdown command sender seam currently exists. If the coordinator keeps daemon source read-only for effort 12, defer this test to the daemon effort that owns shutdown broadcast and prove worker stopped behavior with inbound fake `ShutdownWorker` instead.

## Focused red commands

Run the smallest focused command after adding each red test:

```text
cargo test -p worker --test unit run_worker_session_valid_attach_waits_for_start_job_before_repo_preparation -- --nocapture
cargo test -p worker --test unit run_worker_session_start_job_prepares_repo_before_prompt_runner -- --nocapture
cargo test -p worker --test unit run_worker_session_repo_preparation_failure_sends_failed_status_without_prompt_runner -- --nocapture
cargo test -p worker --test unit run_worker_session_prompt_events_stream_to_daemon_in_sequence -- --nocapture
cargo test -p worker --test unit run_worker_session_input_request_waits_for_matching_answer_before_continuing -- --nocapture
cargo test -p worker --test unit run_worker_session_shutdown_before_terminal_prompt_sends_stopped_status -- --nocapture
```

For daemon counterpart tests, only if the scope is opened:

```text
cargo test -p daemon --test unit record_worker_event_input_requested_marks_waiting_and_preserves_correlation -- --nocapture
cargo test -p daemon --test unit send_shutdown_attached_worker_forwards_shutdown_command -- --nocapture
```

The first red should be a meaningful compile failure for the missing runtime API. After the API exists, each red should fail on behavior assertions, not syntax, missing fixtures, live services, network access, or provider credentials.

## Green and regression commands

Focused green loop:

```text
cargo test -p worker --test unit runtime -- --nocapture
cargo test -p worker --test unit prompt_agent -- --nocapture
cargo test -p worker --test unit repo -- --nocapture
cargo test -p worker --test unit event -- --nocapture
```

Package regression:

```text
cargo test -p worker --no-fail-fast
cargo build -p worker --bin doric-worker
cargo clippy -p worker --all-targets -- -D warnings
```

Cross-package regression when daemon counterpart or proto assumptions are touched:

```text
cargo test -p daemon --no-fail-fast
cargo test -p lifecycle --no-fail-fast
```

Nx parity from the effort file:

```text
npx nx run worker:build
npx nx run worker:test
```

If Windows target locks interfere with Cargo verification, rerun the same Cargo commands with an isolated `CARGO_TARGET_DIR` and record the workaround in the validator receipt.

## Scope boundaries

- No direct CLI command tests in effort 12. CLI lifecycle command rendering belongs to effort 13.
- No worker-owned public server. Worker initiates a daemon session and consumes daemon frames; tests should use a fake session trait, not a bound port.
- No live LLM, no OpenAI/OpenRouter network call, and no provider credential dependency.
- No network repo clone. Runtime tests use `FakeRepoPreparer`; repo unit tests already cover injected Git command behavior.
- No real daemon process. Runtime tests fake daemon command streams; daemon unit tests stay in-process.
- No claims that prompt runtime performs PRD, TDD, decomposition, implementation, tests, validation, or handover. Event names and messages stay prompt/requirements-only.
- No sandboxing or path-confinement claims. Worker tool registration uses repo-root defaults and trace storage only.
- No lower `runtime` or `provider-runtime` package extraction in this effort.

## Risks

- The current `PromptAgentRunner` is one-shot and synchronous. Input wait/answer continuation will force a new or adapted session-oriented seam; avoid contorting the prompt-agent unit API if a runtime-specific trait keeps the boundary clearer.
- Redaction is already covered in `WorkerError` and lifecycle helpers, but runtime can still leak secrets by forwarding raw `StartJob`, token, or prompt failure text. Secret assertions must inspect every outbound frame.
- Sequence ownership can drift between prompt-agent event mapping and runtime transport. Runtime should own transport sequence numbers so resumed prompt runs cannot reset sequence to zero.
- Shutdown can race terminal prompt completion. Tests should assert one terminal status, with stopped winning once shutdown is accepted.
- Adding daemon shutdown command support may exceed the current effort's target files. Treat it as optional counterpart coverage unless the coordinator explicitly opens daemon source write scope.

Coordinator decision: accepted
