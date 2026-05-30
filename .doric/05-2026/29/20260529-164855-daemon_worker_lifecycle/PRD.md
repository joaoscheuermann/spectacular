# Product Requirements Document

## Problem statement

Doric currently behaves too much like a local interactive coding-agent surface for the lifecycle direction the product is taking. Users need a raw CLI entry point that can submit long-running feature or debug jobs, delegate execution to a managed worker, and observe or answer workflow prompts through a daemon-mediated lifecycle.

The first slice must prove the product loop without pretending the full Doric SDLC exists yet: a user submits a prompt and target repo, the daemon owns worker liveness and routing, the worker runs the prompt/requirements agent workflow, and the user can inspect enough state to know what is running, what it is doing, and whether it needs input.

## Goals

1. Provide a raw CLI workflow for dispatching feature and debug jobs with a prompt and target repo.
2. Make the daemon the user-visible lifecycle authority for worker identity, liveness, status, events, and input routing.
3. Let users list live workers and understand job identity, mode, current activity, and health from CLI output.
4. Let users stream readable worker events for a selected worker through the daemon.
5. Support a daemon-mediated path for human-in-the-loop input when the prompt/requirements workflow pauses for an answer.
6. Keep v1 scoped to the prompt/requirements agent so users get a reliable lifecycle foundation before deeper SDLC phases are added.
7. Allow the first slice to run on the host under a configured root folder while making the Docker sandbox boundary a later product step.

## Non-goals

1. No TUI for the initial slice.
2. No full multi-step Doric SDLC beyond the prompt/requirements agent.
3. No Docker sandboxing requirement for v1.
4. No direct CLI-to-worker observation or control path.
5. No permission-management or multi-user authorization model in v1.
6. No full agent evaluator or tournament workflow execution in this slice.
7. No product promise that `feature` and `debug` perform different internal workflows before that behavior is intentionally defined.

## Personas

1. Developer/operator running `doric` locally who needs to start jobs, inspect liveness, and diagnose failures without a TUI.
2. Feature/debug requester who submits a prompt and target repo and expects a clear job identity plus visible progress.
3. Future coordinator/user who must answer human-in-the-loop workflow prompts after a worker has already started.

## Generator persona debate

| Disputed point | Persona tension | Decision | Tradeoff accepted |
| --- | --- | --- | --- |
| Raw CLI scope | End-User wanted the shortest path from prompt/repo to a running job; Product Marketer wanted a crisp first demo; Security/Compliance wanted fewer hidden side effects. | Start with explicit raw CLI commands for dispatch, list, stream, and daemon-mediated input. | v1 favors operational clarity over polished UI or broad workflow coverage. |
| Daemon mediation | End-User wanted fast status access; Security/Compliance rejected direct worker inspection because it weakens auditability and routing guarantees. | CLI observes and answers through the daemon only. | Slight routing overhead is accepted to preserve a single lifecycle authority. |
| Worker state detail | End-User wanted enough detail to recover from hangs; Security/Compliance wanted minimal data exposure; Product Marketer wanted simple status language. | Surface job identity, mode, repo identity, liveness, current activity, input waits, and failure reasons while avoiding credential leakage. | Output is informative but not a raw transcript dump. |
| v1 workflow depth | Product Marketer wanted a compelling lifecycle story; End-User wanted useful output quickly; Security/Compliance preferred a narrow blast radius on host execution. | Implement only the prompt/requirements agent workflow in this first lifecycle slice. | Doric demonstrates orchestration value before promising full SDLC automation. |
| Human-in-the-loop interaction | End-User wanted a clear place to see and answer prompts; Security/Compliance wanted correlation and no bypass path. | A waiting worker must expose a request id or equivalent correlation handle and accept the answer through a daemon-mediated CLI path. | Exact command syntax can be finalized in technical design, but the product behavior is required. |

## Primary workflows

1. Dispatch a feature job.
   - User runs a command equivalent to `doric feature --prompt="<prompt>" --repo="<remote_to_clone>"`.
   - CLI sends the request to the daemon.
   - Daemon accepts or rejects the job and returns a stable job/worker identity, mode, repo identity, and initial status.
   - The worker begins the prompt/requirements workflow for the target repo.

2. Dispatch a debug job.
   - User runs a command equivalent to `doric debug --prompt="<prompt>" --repo="<remote_to_clone>"`.
   - CLI and daemon follow the same lifecycle as feature dispatch.
   - If debug and feature share v1 behavior, the mode label still remains visible in status and events.

3. List live workers.
   - User runs a command equivalent to `doric list`.
   - CLI asks the daemon for known live worker state.
   - Output shows empty state, active workers, input-waiting workers, failed workers, or terminal workers clearly enough for the user to choose the next command.

4. Stream a worker.
   - User runs a command equivalent to `doric worker <id>`.
   - CLI receives worker events through the daemon.
   - Output is ordered and readable, including lifecycle transitions, current activity, human-input waits, failures, and terminal completion.

5. Answer a human-in-the-loop request.
   - A running worker reaches a workflow point requiring user input.
   - List and stream output show that the worker is waiting, including the request text and a correlation handle.
   - User sends an answer through a daemon-mediated CLI command.
   - Worker resumes and emits a continuation event.

6. Diagnose failure.
   - User dispatches an invalid request, targets an unavailable daemon, references an unknown worker id, or a worker fails.
   - CLI exits nonzero for command failures and prints a concise actionable message.
   - Daemon-visible state reflects worker failures rather than making the user infer them from process state or files.

## Functional requirements

| ID | Requirement |
| --- | --- |
| FR-1 | The CLI must expose raw commands for feature dispatch, debug dispatch, worker listing, worker event streaming, and daemon-mediated human input. |
| FR-2 | Feature and debug dispatch must require a prompt and repo value, reject missing required inputs, and avoid creating jobs for invalid requests. |
| FR-3 | Successful dispatch must return a stable job or worker id, requested mode, repo identity, and initial lifecycle status. |
| FR-4 | The daemon must be the source of truth for CLI-visible worker liveness, status, events, and input routing. |
| FR-5 | `doric list` or equivalent must show an empty state when no workers are known and a structured status row for each known live or recently terminal worker. |
| FR-6 | Worker status must cover, at minimum, accepted/starting, running, waiting for input, succeeded, failed, and stopped or unavailable states. Exact names may differ if the states remain testable. |
| FR-7 | `doric worker <id>` or equivalent must stream readable, ordered worker events through the daemon and must fail clearly for unknown ids. |
| FR-8 | Events must expose user-relevant lifecycle milestones: dispatch accepted, worker starting, repo preparation, prompt/requirements agent start, current activity, human-input wait, failure, and completion. |
| FR-9 | When a workflow step needs user input, CLI-visible state must show the worker as waiting and provide enough correlation for the user to answer the correct request through the daemon. |
| FR-10 | The first slice must run only the prompt/requirements agent workflow; later PRD, technical design, decomposition, and implementation phases must not be implied by v1 command output. |
| FR-11 | The first slice may run workers on the host under a configured root folder; missing or inaccessible root configuration must fail before job execution with a clear message. |
| FR-12 | CLI output and worker events must redact credentials embedded in repo URLs and must not print environment secrets. |
| FR-13 | Worker crashes, repo preparation failures, and prompt-agent failures must be visible in both stream output and daemon-reported status. |
| FR-14 | If v1 worker registry state is in-memory, daemon restart behavior must be explicit: the CLI must not report stale workers as live and must explain when a requested worker is no longer tracked. |

## User Value Hops

| Hop | User-facing outcome | Assumption | Evidence or signal | Failure mode | Acceptance coverage |
| --- | --- | --- | --- | --- | --- |
| 1 | User can submit a feature or debug job without opening a TUI. | Raw CLI commands are sufficient for the first target user. | Dispatch command accepts prompt and repo, exits successfully, and returns job identity. | User cannot start lifecycle work from the command line. | AC-1, AC-2, AC-3 |
| 2 | User knows the daemon accepted the job and can refer to it later. | A stable job/worker id plus mode and repo identity is enough for follow-up commands. | Dispatch output includes id, mode, repo identity, and initial status. | User cannot correlate list/stream output with the submitted job. | AC-1, AC-2 |
| 3 | User can check what is alive and what each worker is doing. | Daemon-reported status is timely and understandable enough for operational use. | `list` shows empty, active, waiting, failed, and terminal states. | User has to inspect processes or files to infer liveness. | AC-5, AC-6, AC-9, AC-14 |
| 4 | User can observe worker progress as readable lifecycle events. | Event stream contains the right milestones without exposing excessive internals. | Stream includes ordered dispatch, repo prep, prompt-agent, wait, failure, and completion events. | User sees a silent or noisy stream that cannot guide action. | AC-7, AC-9, AC-10, AC-12 |
| 5 | User can answer workflow prompts after dispatch. | Waiting state plus a correlation handle prevents answering the wrong request. | Stream and list identify waiting workers; answer command resumes the worker and emits continuation. | Worker blocks indefinitely or answers are misrouted. | AC-11 |
| 6 | User can recover from invalid commands and unavailable services. | Clear errors and nonzero exits are enough for first-slice recovery. | Missing args, unknown workers, daemon unavailable, and root config failures are distinct. | User cannot tell whether the issue is input, daemon, worker, or repo setup. | AC-3, AC-4, AC-8, AC-13 |
| 7 | User understands v1 scope and does not expect full SDLC automation. | Prompt/requirements-only output can still demonstrate lifecycle value. | Commands and terminal events identify the prompt/requirements workflow and do not claim later phases ran. | Users believe Doric completed design, decomposition, or coding when it did not. | AC-10 |
| 8 | Doric can evolve toward sandboxing and deeper agents without changing the user contract. | Daemon-mediated lifecycle and configured worker root are compatible with future isolation. | Host-root execution is explicit, Docker is not required, and direct worker access is absent. | The first slice creates a product path that later sandboxing or lifecycle routing must break. | AC-4, AC-11, AC-13 |

## Acceptance criteria

| ID | Criterion |
| --- | --- |
| AC-1 | Given the daemon is reachable and worker root configuration is valid, when the user runs a feature dispatch command with non-empty prompt and repo values, then the command exits 0 and prints a stable id, mode `feature`, repo identity, and initial status. |
| AC-2 | Given the daemon is reachable and worker root configuration is valid, when the user runs a debug dispatch command with non-empty prompt and repo values, then the command exits 0 and prints a stable id, mode `debug`, repo identity, and initial status. |
| AC-3 | Given the user omits prompt or repo, when they run feature or debug dispatch, then the command exits nonzero, names the missing required input, and no worker appears in `list`. |
| AC-4 | Given the daemon is unavailable, when the user runs dispatch, list, stream, or answer commands, then the command exits nonzero with a daemon-unreachable message and does not attempt direct worker communication. |
| AC-5 | Given the daemon is reachable and no workers are known, when the user runs `list`, then the command exits 0 and prints a clear empty state rather than an error or blank output. |
| AC-6 | Given one or more workers are known, when the user runs `list`, then each worker row includes id, mode, repo identity, lifecycle status, current activity or terminal reason, and enough timing or ordering information to distinguish jobs. |
| AC-7 | Given a known worker id, when the user runs `worker <id>` or equivalent, then events are shown in daemon-provided order and include lifecycle milestones for accepted, starting, repo preparation, prompt/requirements agent activity, waiting for input when applicable, failure, and completion. |
| AC-8 | Given an unknown worker id, when the user runs `worker <id>` or an answer command for that id, then the command exits nonzero with an unknown-worker message and does not hang. |
| AC-9 | Given a worker crashes or repo preparation fails, when the user runs `list` or streams that worker, then the worker is marked failed and a concise failure reason is visible. |
| AC-10 | Given a successful v1 worker run, when the user inspects stream output or terminal status, then the output identifies the prompt/requirements workflow and does not claim PRD, technical design, decomposition, code, or tests were produced. |
| AC-11 | Given a worker requests human input, when the user checks `list` or streams the worker, then the worker is marked waiting for input with request text and a correlation handle; when the user answers through the daemon-mediated CLI path, the worker resumes and emits a continuation event. |
| AC-12 | Given a repo URL contains credentials or the environment contains secrets, when dispatch, list, and stream output are rendered, then credentials in repo identity are redacted and environment secrets are not printed. |
| AC-13 | Given the configured worker root is missing, inaccessible, or invalid, when the user dispatches a job, then the command exits nonzero before job execution and explains the root configuration problem. |
| AC-14 | Given v1 registry state is in-memory and the daemon restarts, when the user lists or streams a previously known worker, then stale workers are not reported as live and unknown or untracked workers produce a clear message. If persistence is selected instead, the equivalent restart test must prove restored status accuracy. |
| AC-15 | Given `feature` and `debug` share the same prompt/requirements workflow in v1, when users dispatch both modes, then status and events preserve the requested mode label while making no false claim that separate internal behavior exists. |

## Product alternatives and tournament

No product tournament is recorded for this PRD. The apparent alternatives in the prompt are not live candidates for v1:

1. TUI-first versus raw CLI is already resolved in favor of raw CLI.
2. Direct CLI-to-worker observation versus daemon-mediated observation is already resolved in favor of daemon mediation.
3. Docker-first versus host-root execution is already resolved as Docker-later and host-root acceptable for the first slice.
4. Full SDLC execution versus prompt/requirements-only execution is already resolved in favor of prompt/requirements-only.

The exact human-in-the-loop command syntax remains a design choice, not a product tournament. Any selected syntax must preserve the required product behavior: visible waiting state, correlation handle, daemon-mediated answer, and worker continuation.

## Success measures

1. A user can dispatch both `feature` and `debug` jobs from the CLI and receive a usable id without opening a TUI.
2. `list` reflects a newly accepted worker quickly enough for interactive use after dispatch.
3. `worker <id>` shows readable lifecycle events for a normal prompt/requirements run from start through terminal state.
4. A simulated human-input request can be observed, answered through the daemon, and resumed without direct worker access.
5. Missing prompt, missing repo, daemon unavailable, unknown worker, invalid worker root, repo preparation failure, and worker crash each produce distinct testable failures.
6. Output and event streams preserve v1 scope by identifying prompt/requirements work and avoiding claims about unimplemented SDLC phases.
7. Credential-bearing repo URLs are redacted in all user-visible output.

## Risks and compliance

1. Host execution before Docker sandboxing can run against arbitrary repos on the local machine. V1 should make the configured root explicit and avoid implying strong isolation.
2. Repo URLs, prompts, tool events, or failure messages may contain secrets. User-visible output must redact credentials in repo URLs and avoid printing environment secrets.
3. Long-running workers can become orphaned or stale if daemon tracking is weak. The product must clearly distinguish running, failed, stopped, unavailable, and untracked states.
4. Human-in-the-loop answers can be misrouted if waiting prompts lack correlation. Waiting state must include a request handle or equivalent unambiguous routing cue.
5. In-memory lifecycle state may be acceptable for v1, but daemon restart behavior can surprise users unless the CLI reports lost or untracked state clearly.
6. Feature/debug mode labels can create false expectations if they share v1 behavior. Output must preserve the requested mode while being honest about prompt/requirements-only execution.
7. The product aims to remain model- and harness-independent; command output should describe Doric lifecycle concepts rather than vendor-specific runtime details.

## Open questions

1. Should worker lifecycle state persist across daemon restarts in the first slice, or is explicit in-memory behavior acceptable for v1?
2. What exact CLI syntax should send a human-in-the-loop answer back to a waiting worker?
3. What root folder convention should host-run workers use for cloned repos and run artifacts?
4. Should `feature` and `debug` differ in v1 behavior, or are they separate command labels over the same prompt/requirements lifecycle?
5. Should `worker <id>` replay recent event history before live streaming, or only stream events emitted after attachment?

These questions do not block technical design if the design records explicit assumptions and keeps the acceptance criteria above testable.
