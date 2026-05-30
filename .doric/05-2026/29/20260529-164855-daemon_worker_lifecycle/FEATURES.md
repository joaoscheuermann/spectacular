# Features

## Source artifacts

| Artifact | Role in extraction |
| --- | --- |
| `PROMPT.md` | Source feature intent, product constraints, architecture constraints, resolved decisions, open questions, and non-goals. |
| `PRD.md` | Source product workflows, functional requirements, User Value Hops, acceptance criteria, success measures, risks, and product exclusions. |
| `TDD.md` | Source technical design, package boundaries, component responsibilities, interface contracts, Dependency Hops, tests, rollout, repaired decisions, and technical exclusions. |
| `GAPS_REPORT.md` | Historical technical-gap handoff. The current `TDD.md` is treated as repaired, and this extraction preserves the repaired decisions rather than the superseded gaps. |
| `.agents/skills/doric/references/04-decomposition.md` | Required `FEATURES.md` schema and coverage expectations. |

## Extracted features

| Feature | Name | Description | Product trace | Technical trace |
| --- | --- | --- | --- | --- |
| F-01 | Raw lifecycle CLI and chat-safe routing | Add the raw `doric` lifecycle commands for daemon startup, feature dispatch, debug dispatch, list, worker stream, and answer input. Lifecycle and config commands parse and dispatch before chat/provider debug-log setup, preserving bare chat behavior while preventing lifecycle commands from failing on chat debug logging. | FR-1, FR-2, FR-3, FR-4, FR-7, FR-9; AC-1, AC-2, AC-3, AC-4, AC-8, AC-11; UVH-1, UVH-2, UVH-5, UVH-6 | CLI command surface; CLI output contracts; Dependency Hops: CLI command surface, CLI entrypoint dispatch |
| F-02 | Lifecycle contracts, codegen, DTOs, and redaction | Add the shared `lifecycle` package with protobuf/gRPC contracts, generated Rust modules, DTO wrappers, worker/request/repo identity helpers, and central redaction helpers. Use the current repaired `tonic`, `tonic-prost`, `prost`, `prost-types`, `tonic-prost-build`, and `protoc-bin-vendored` stack. | FR-3, FR-4, FR-7, FR-8, FR-9, FR-12, FR-13, FR-14; AC-1, AC-2, AC-7, AC-9, AC-11, AC-12, AC-14 | `packages/lifecycle`; shared lifecycle DTOs; gRPC services; generated Rust contract; Dependency Hops: gRPC/protobuf build, redaction/secrets |
| F-03 | Daemon service, registry, root validation, and lifecycle authority | Add the daemon library and `doric-daemon` binary. The daemon owns the worker registry, in-memory statuses, retained event rings, pending-input records, root validation, worker process liveness, CLI-facing service methods, worker-session routing, restart semantics, and lifecycle authority for every CLI-visible state. | FR-4, FR-5, FR-6, FR-7, FR-9, FR-11, FR-13, FR-14; AC-4, AC-5, AC-6, AC-7, AC-8, AC-9, AC-11, AC-13, AC-14; UVH-2, UVH-3, UVH-5, UVH-6, UVH-8 | `packages/daemon`; daemon lifecycle flow; daemon internal state; daemon registry interface; security/privacy; performance/operations; Dependency Hops: daemon registry, root/repo handling, worker process/runtime, human input routing, event stream |
| F-04 | Worker process launch and authenticated daemon session | Spawn `doric-worker` from the daemon, require worker attach through a bidirectional gRPC stream with a one-time token, send job payload over daemon-controlled gRPC, route daemon commands back to the worker, enforce attach deadlines, and map child process terminal states into daemon-visible status. | FR-3, FR-4, FR-6, FR-8, FR-13, FR-14; AC-1, AC-2, AC-7, AC-9, AC-14; UVH-2, UVH-3, UVH-4, UVH-6 | `packages/worker`; daemon lifecycle flow; worker-facing session service; worker process spawner trait; performance/operations; Dependency Hop: worker process/runtime |
| F-05 | Feature/debug dispatch lifecycle | Implement feature and debug dispatch as separate user-facing modes over the same v1 prompt/requirements workflow. Dispatch requires prompt and repo, rejects invalid input before creating jobs, returns stable worker identity, preserves the requested mode label, and avoids false claims that `feature` and `debug` have separate internal behavior in v1. | FR-1, FR-2, FR-3, FR-10; AC-1, AC-2, AC-3, AC-10, AC-15; UVH-1, UVH-2, UVH-7 | CLI command surface; dispatch flow; shared lifecycle DTOs; CLI output contracts; Dependency Hops: CLI command surface, prompt-agent-only scope |
| F-06 | Host-root repo preparation with external Git | Resolve and validate the daemon worker root, create a per-worker `repo/`, `state/`, `artifacts/`, and `tool-output/` layout, and have the worker clone the requested remote repository into `repo/` with an injected external `git` command runner. Missing Git, clone failures, timeouts, cancellation, invalid roots, and credential-bearing URLs become redacted lifecycle failures. | FR-8, FR-11, FR-12, FR-13; AC-7, AC-9, AC-12, AC-13; UVH-4, UVH-6, UVH-8 | root and repo handling; worker internal state; repo preparation interfaces; testing strategy; Dependency Hops: root/repo handling, repo clone mechanism, redaction/secrets |
| F-07 | Prompt/requirements worker runtime and artifact output | Keep v1 worker execution limited to the prompt/requirements agent. The worker composes provider/runtime locally from public `config`, `llms`, and `agent` APIs, uses existing agent runtime and worker tools, writes `PROMPT.md` under worker artifacts, may request human input, and emits only prompt-agent lifecycle events. | FR-8, FR-9, FR-10, FR-13; AC-7, AC-10, AC-11, AC-15; UVH-4, UVH-5, UVH-7 | worker prompt-agent execution; worker provider/runtime composition; worker internal state; Rust interfaces; testing strategy; Dependency Hops: worker provider/runtime composition, prompt-agent-only scope, testability |
| F-08 | Shared tools registration without confinement claims | Keep `packages/tools` as the shared owner for built-in tools. The worker depends on `tools` and registers built-ins with the cloned repo as workspace root/default and `tool-output/` as trace storage, while existing chat registration remains untouched. This is explicitly not sandboxing or path confinement. | FR-10, FR-11, FR-12; AC-10, AC-12, AC-13; UVH-7, UVH-8 | tool ownership graph; worker tooling module; security/privacy; testing strategy; rollout/migration; Dependency Hops: tool ownership graph, root/repo handling |
| F-09 | Worker list, status model, event replay, and stream output | Provide daemon-mediated listing and streaming. Statuses cover accepted/starting, running, waiting for input, succeeded, failed, stopped, and unavailable. Streams replay retained in-memory events from sequence `0`, then tail live events, include lifecycle milestones, surface history truncation, and fail clearly for unknown workers. | FR-4, FR-5, FR-6, FR-7, FR-8, FR-13, FR-14; AC-4, AC-5, AC-6, AC-7, AC-8, AC-9, AC-14; UVH-3, UVH-4, UVH-6 | daemon lifecycle flow; shared lifecycle DTOs; daemon internal state; CLI output contracts; testing strategy; performance/operations; Dependency Hops: daemon registry, event stream, testability |
| F-10 | Daemon-mediated human input flow | Represent worker input waits with request text and a correlation handle, expose waiting state in list and stream output, accept answers only through `doric answer <worker-id> <request-id> --text`, validate pending input in the daemon, forward answers over the worker session, and emit a continuation event. | FR-1, FR-4, FR-9; AC-4, AC-8, AC-11; UVH-5 | daemon lifecycle flow; gRPC services; shared lifecycle DTOs; CLI output contracts; Rust registry interface; Dependency Hop: human input routing |
| F-11 | Secret-safe and scope-honest user-visible output | Redact credentials embedded in repo URLs, avoid printing environment secrets, keep provider/token details out of lifecycle events, render concise actionable failures, and ensure stream/terminal output identifies only prompt/requirements work in v1. | FR-8, FR-10, FR-12, FR-13; AC-7, AC-9, AC-10, AC-12, AC-15; UVH-4, UVH-6, UVH-7 | lifecycle redaction helpers; CLI output contracts; security/privacy; risks; Dependency Hops: redaction/secrets, prompt-agent-only scope |
| F-12 | Test seams, validation gates, and architecture documentation | Preserve the design's testable seams and validation plan: package-level unit tests, fake daemon/worker/repo/prompt seams, in-process integration smoke, Cargo/Nx codegen gates, workspace validation, and `docs/architecture-and-packages.md` updates for the new package map and repaired decisions. | FR-1 through FR-14; AC-1 through AC-15; all User Value Hops indirectly through test evidence | testing strategy; rollout/migration; validation commands; architecture documentation note; Dependency Hop: testability |

## Requirement coverage map

| Source item | Feature coverage |
| --- | --- |
| FR-1 raw commands for feature, debug, list, worker stream, and human input | F-01, F-05, F-10 |
| FR-2 dispatch requires prompt and repo and rejects missing inputs without creating jobs | F-01, F-05 |
| FR-3 successful dispatch returns stable id, mode, repo identity, and initial status | F-01, F-02, F-05 |
| FR-4 daemon is source of truth for liveness, status, events, and input routing | F-01, F-02, F-03, F-09, F-10 |
| FR-5 list shows empty state and structured rows for known workers | F-03, F-09 |
| FR-6 worker status covers accepted/starting, running, waiting, succeeded, failed, stopped/unavailable | F-03, F-04, F-09 |
| FR-7 worker stream is readable, ordered, daemon-mediated, and fails clearly for unknown ids | F-01, F-02, F-03, F-09 |
| FR-8 events expose dispatch, starting, repo prep, prompt-agent, activity, input wait, failure, completion | F-02, F-04, F-06, F-07, F-09, F-11 |
| FR-9 user-input wait exposes enough correlation for daemon-mediated answer | F-01, F-03, F-07, F-10 |
| FR-10 v1 runs only prompt/requirements workflow and does not imply later SDLC phases | F-05, F-07, F-08, F-11 |
| FR-11 host workers may run under configured root; invalid root fails before execution | F-03, F-06, F-08 |
| FR-12 output and events redact repo credentials and avoid environment secrets | F-02, F-06, F-08, F-11 |
| FR-13 worker crashes, repo failures, and prompt-agent failures visible in stream and status | F-03, F-04, F-06, F-07, F-09, F-11 |
| FR-14 in-memory restart behavior is explicit and stale workers are not reported live | F-02, F-03, F-04, F-09 |
| UVH-1 user can submit feature/debug job without TUI | F-01, F-05 |
| UVH-2 user gets accepted job identity for later commands | F-03, F-04, F-05 |
| UVH-3 user can check what is alive and current activity | F-03, F-09 |
| UVH-4 user can observe readable lifecycle progress | F-04, F-06, F-07, F-09, F-11 |
| UVH-5 user can answer workflow prompts after dispatch | F-01, F-03, F-07, F-10 |
| UVH-6 user can recover from invalid commands and unavailable services | F-01, F-03, F-04, F-06, F-09, F-11 |
| UVH-7 user understands v1 scope and does not expect full SDLC automation | F-05, F-07, F-08, F-11 |
| UVH-8 lifecycle can evolve toward sandboxing and deeper agents without changing user contract | F-03, F-06, F-08 |
| AC-1 feature dispatch succeeds with daemon/root valid and prints id, feature mode, repo, status | F-01, F-02, F-05 |
| AC-2 debug dispatch succeeds with daemon/root valid and prints id, debug mode, repo, status | F-01, F-02, F-05 |
| AC-3 missing prompt or repo exits nonzero, names input, and creates no listed worker | F-01, F-05 |
| AC-4 daemon unavailable causes dispatch/list/stream/answer to fail without direct worker communication | F-01, F-03, F-09, F-10 |
| AC-5 list with no workers exits 0 and prints clear empty state | F-03, F-09 |
| AC-6 list rows include id, mode, repo, status, activity/reason, and timing/order signal | F-03, F-09 |
| AC-7 stream known worker in order with lifecycle milestones | F-02, F-04, F-06, F-07, F-09, F-11 |
| AC-8 stream or answer unknown worker exits nonzero and does not hang | F-01, F-03, F-09, F-10 |
| AC-9 worker crash or repo failure visible in list and stream as failed with reason | F-03, F-04, F-06, F-09, F-11 |
| AC-10 successful v1 output identifies prompt/requirements workflow only | F-05, F-07, F-11 |
| AC-11 input request shows waiting state, request text, correlation handle, daemon-mediated answer, and continuation | F-01, F-03, F-07, F-10 |
| AC-12 dispatch/list/stream redact repo credentials and avoid environment secrets | F-02, F-06, F-08, F-11 |
| AC-13 invalid worker root fails before execution with clear root-config message | F-03, F-06, F-08 |
| AC-14 in-memory restart does not report stale workers live and gives unknown/untracked message; persistence alternative would prove restored accuracy | F-02, F-03, F-04, F-09 |
| AC-15 shared feature/debug workflow preserves mode labels without claiming separate internal behavior | F-05, F-07, F-11 |

## Technical coverage map

| TDD component or Dependency Hop | Feature coverage |
| --- | --- |
| `packages/lifecycle` library, proto ownership, generated Rust modules, wrappers, redaction helpers, current tonic/prost stack | F-02, F-11, F-12 |
| `packages/daemon` library and `doric-daemon` binary with `server`, `registry`, `process`, `service`, `worker_session`, and `root` modules | F-03, F-04, F-09, F-10, F-12 |
| `packages/worker` library and `doric-worker` binary with `runtime`, `repo`, `provider`, `agents/prompt`, `tooling`, `state`, and `event` modules | F-04, F-06, F-07, F-08, F-12 |
| CLI command surface and defaults | F-01, F-05, F-09, F-10 |
| CLI entrypoint rewrite keeping lifecycle commands outside chat/provider debug-log startup | F-01 |
| Daemon dispatch flow | F-03, F-04, F-05, F-06 |
| Daemon list flow | F-03, F-09 |
| Daemon stream flow with replay, live tail, and history truncation | F-03, F-09 |
| Daemon-mediated human input flow | F-03, F-10 |
| Daemon restart semantics and orphan worker handling | F-03, F-04, F-09 |
| Worker prompt-agent execution scope | F-07, F-11 |
| Worker provider/runtime composition from `config`, `llms`, and `agent` APIs | F-07 |
| Worker debug logging remains worker-scoped and may use disabled logger by default | F-07 |
| Deferred runtime extraction after v1 | F-07; excluded from v1 in Exclusions |
| Root resolution and worker-root validation | F-03, F-06 |
| Per-worker root layout: `repo/`, `state/`, `artifacts/PROMPT.md`, `tool-output/` | F-06, F-07, F-08 |
| External Git repo preparation and failure mapping | F-06, F-11 |
| Tool ownership graph keeps `packages/tools` shared and not confined | F-08 |
| Shared lifecycle DTOs | F-02, F-09, F-10 |
| Daemon internal state and event ring | F-03, F-09, F-10 |
| Worker internal state | F-06, F-07 |
| CLI-facing `LifecycleService` gRPC contract | F-01, F-02, F-03, F-05, F-09, F-10 |
| Worker-facing `WorkerSessionService` gRPC contract | F-02, F-04, F-10 |
| Generated Rust contract and Windows vendored-protoc handling | F-02, F-12 |
| CLI output contracts for dispatch, list, stream, and answer | F-01, F-05, F-09, F-10, F-11 |
| Rust interfaces: `Registry`, `WorkerSpawner`, `RepoPreparer`, `GitCommandRunner` | F-03, F-04, F-06, F-09, F-10, F-12 |
| Dependency Hop: CLI command surface | F-01, F-05 |
| Dependency Hop: CLI entrypoint dispatch | F-01 |
| Dependency Hop: gRPC/protobuf build | F-02, F-12 |
| Dependency Hop: Daemon registry | F-03, F-09 |
| Dependency Hop: Worker process/runtime | F-04 |
| Dependency Hop: Event stream | F-09 |
| Dependency Hop: Human input routing | F-10 |
| Dependency Hop: Root/repo handling | F-03, F-06, F-08 |
| Dependency Hop: Repo clone mechanism | F-06 |
| Dependency Hop: Tool ownership graph | F-08 |
| Dependency Hop: Worker provider/runtime composition | F-07 |
| Dependency Hop: Redaction/secrets | F-02, F-06, F-11 |
| Dependency Hop: Prompt-agent-only scope | F-05, F-07, F-11 |
| Dependency Hop: Testability | F-12 |
| Package boundary tournament selected `lifecycle`, `daemon`, and `worker` | F-02, F-03, F-04, F-07, F-12 |
| Registry persistence tournament selected in-memory registry | F-03, F-09 |
| Worker transport tournament selected worker-initiated bidirectional gRPC stream | F-04, F-10 |
| Provider/runtime composition tournament selected worker-local composition | F-07 |
| Security and privacy requirements | F-02, F-03, F-06, F-08, F-11 |
| Performance and operations requirements | F-03, F-04, F-09 |
| Testing strategy and validation commands | F-12 |
| Rollout and migration list, including docs update | F-12, with implementation-specific items traced to F-01 through F-11 |

## Assumption coverage

| Assumption or repaired decision | Feature coverage | Notes |
| --- | --- | --- |
| Raw CLI commands are sufficient for the first target user. | F-01, F-05 | Preserves the prompt/PRD decision that v1 is raw CLI, not TUI. |
| Stable worker id plus mode and repo identity is enough for follow-up commands. | F-02, F-05, F-09 | Dispatch/list/stream share the same identity model. |
| Daemon-reported status is timely and understandable enough for operational use. | F-03, F-09 | Daemon registry and event ring are the user-visible authority. |
| Event stream can guide action without exposing excessive internals. | F-09, F-11 | Stream output is milestone-based and redacted. |
| Waiting state plus request id prevents answer misrouting. | F-10 | The daemon validates `(worker_id, request_id)` before forwarding answers. |
| Clear nonzero errors are enough for first-slice recovery. | F-01, F-03, F-06, F-09, F-11 | Missing inputs, daemon unavailable, unknown worker, invalid root, repo failure, and worker failure remain distinct. |
| Prompt/requirements-only output can demonstrate lifecycle value. | F-05, F-07, F-11 | V1 output must not claim PRD, TDD, decomposition, implementation, tests, or handover. |
| Host-root execution is acceptable before Docker if non-containment is explicit. | F-03, F-06, F-08 | No sandboxing or confinement claim is made. |
| `packages/tools` remains shared and is not confined to worker. | F-08 | Preserves the repaired TDD and gap resolution: do not move existing tools into `worker`; do not claim repo-root default as confinement. |
| Worker provider/runtime composition is worker-local in v1. | F-07 | Preserves the repaired TDD decision to compose from `config`, `llms`, and `agent`, with no `cli` dependency and no lower runtime package extraction in this slice. |
| Current protobuf stack is `tonic` plus `tonic-prost`, `prost`, `prost-types`, `tonic-prost-build`, and `protoc-bin-vendored`. | F-02, F-12 | Preserves the repaired TDD decision and rejects the superseded invalid codegen plan from the gaps report. |
| Repo clone uses the external `git` CLI through an injected command runner. | F-06 | Preserves the repaired TDD decision to avoid `git2`/libgit2 for v1. |
| Lifecycle commands bypass chat debug-log startup. | F-01 | Preserves the repaired TDD decision that provider debug logging remains chat-scoped. |
| Feature and debug share one prompt/requirements workflow in v1. | F-05, F-07, F-11 | Mode labels stay visible without implying separate internal behavior. |
| Registry state is in-memory in v1. | F-03, F-09 | Restart behavior must be explicit and stale workers must not appear live. |
| Stream replay should include retained history before live tail. | F-09 | Truncation is surfaced when retained history no longer includes requested events. |

## Exclusions

| Exclusion | Reason and trace |
| --- | --- |
| Effort planning files under `<run>/efforts/` | Explicitly out of scope for this requirement-extractor task; effort planning is a later sub-agent task. |
| TUI implementation or redesign | PRD non-goal and prompt constraint: initial UI is raw CLI. |
| Full Doric SDLC beyond prompt/requirements agent | PRD non-goal and FR-10/AC-10/AC-15 require prompt/requirements-only v1 output. |
| Docker sandboxing or path confinement | PRD non-goal and repaired TDD decision: host-root execution is acceptable, Docker/scoped tooling is later, and `packages/tools` root defaults are not containment. |
| Direct CLI-to-worker observation or control | PRD non-goal and daemon-mediation requirement. |
| Permission-management, multi-user authorization, or remote daemon auth | PRD non-goal and security section limit v1 to local loopback behavior. |
| Full evaluator/tournament workflow execution | PRD non-goal; prompt/requirements agent only in v1. |
| Distinct internal workflows for feature versus debug | PRD non-goal and AC-15: preserve mode labels while sharing v1 execution. |
| Durable daemon registry or restart recovery | TDD selected in-memory registry; persistence is a later design if required. |
| QUIC transport | Prompt says evaluate only as preference; TDD selected local gRPC over loopback for v1. |
| Moving existing chat tools into `worker` or retiring `packages/tools` | Repaired TDD decision keeps `packages/tools` shared to preserve chat. |
| Lower `runtime` or `provider-runtime` package extraction | Repaired TDD decision defers extraction until worker and chat prove a shared shape after v1. |
| `git2` or other Rust Git library clone implementation | Repaired TDD decision uses external Git CLI with fakeable command runner. |
| Live network repository or live LLM dependency in automated tests | TDD testing strategy requires fake repo/prompt/provider seams for deterministic automated tests. |
| Retiring bare chat behavior | TDD preserves existing chat unless a separate accepted artifact retires it. |

## Validator notes

Accepted by `agents/021_decomposition_validator.md`; effort files were planned afterward by `agents/022_decomposition_effort_planner.md`.
