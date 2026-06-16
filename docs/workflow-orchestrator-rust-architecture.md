# Workflow Orchestrator Rust Architecture

Status: target architecture. This document ties the phase-specific Doric
workflow documents into one Rust-owned workflow architecture. The first
implementation should be small and concrete, while preserving a clean path to
nested workflows. It does not claim the current code already enforces the full
workflow.

Current implementation evidence:

- `docs/architecture-and-packages.md` defines `worker` as the daemon-controlled
  runtime owner, `daemon` as process lifecycle and event routing, and
  `lifecycle` as the shared contract package.
- `packages/worker/src/runtime.rs` currently runs repository preparation and a
  prompt-only `RuntimePromptRunner`.
- `packages/worker/src/agents/prompt.rs` writes `PROMPT.md` under the worker
  `artifacts/` directory.
- `packages/daemon/src/service.rs`, `packages/daemon/src/worker_session.rs`,
  and `packages/daemon/src/registry.rs` route lifecycle commands, stream
  worker events, and validate input answers without owning Doric workflow
  legality.

## Current Gap

The phase docs define the root Doric workflow:

```text
prompt -> prd -> tdd -> decomposition -> development -> handover -> complete
```

The code does not yet have one executable root workflow. The worker
currently treats prompt completion as worker success. That is correct for the
prompt-only implementation, but it is not sufficient for the target workflow.
In the full workflow, prompt completion must advance the workflow cursor to
`prd` after the prompt-to-PRD alignment gate is satisfied; it must not mark the
worker terminal.

The missing component is a Rust-owned workflow orchestrator that:

- Persists canonical workflow state and events.
- Runs one legal root workflow edge at a time.
- Starts with concrete worker-local reducers for the root and phase workflows.
- Allows phase workflows to become nested workflow scopes with self-contained
  state, artifacts, gates, and receipts when the implementation needs that
  shape.
- Blocks on human gates and required-agent proof.
- Regenerates `STATE.md` as a projection.
- Recovers the same workflow cursor after restart.
- Emits workflow progress through the existing lifecycle event stream without
  requiring new public lifecycle contracts first.

## Ownership Model

The orchestrator must preserve the package boundaries already documented in
`docs/architecture-and-packages.md`.

| Package | Target workflow responsibility |
| ------- | ------------------------------ |
| `worker` | Owns canonical workflow execution for one worker run. It hosts the future `worker::workflow` module, performs reducer-based legal transitions, persists machine state, writes artifacts, regenerates projections, handles human-gate waits, and recovers from persisted events. |
| `daemon` | Owns worker process lifecycle, worker registry, worker-session attachment, command routing, event replay, and optional opaque workflow-state mirroring for list output. It must not decide whether `prd` can start, whether the TDD-to-decomposition edge is legal, or whether an effort is complete. |
| `lifecycle` | Owns shared wire-safe domain contracts used across `cli`, `daemon`, and `worker`: worker identity/status/events today, plus additive workflow display fields only when those become public lifecycle contracts. |
| `cli` | Renders lifecycle streams, lists workers, dispatches work, and answers input requests. It may display workflow root step and gate status, but it does not compute legal transitions. |
| `agent` | Remains the generic model/tool runtime. It must not know Doric workflow phases, `STATE.md`, effort gates, or worker process lifecycle. |
| `tools` | Remains the generic built-in tool implementation package. Tool output belongs under the worker `tool-output/` directory; tools do not own phase legality. |
| `config`, `llms` | Continue to own local configuration and provider/model contracts. Workflow state must not leak provider secrets or config credentials into events or artifacts. |

This keeps the executable workflow near the prepared repository, worker-local
provider selection, shared tool registration, and run-local filesystem layout.
The daemon can restart, stream, or route to the worker, but the worker remains
the authority for what the run is allowed to do next.

## Root Workflow

The canonical root workflow step id is `tdd`; the accepted artifact remains
`TDD.md` to match the existing Step 03 documents and run artifact naming.

| Current root step | Required evidence | Next root step | Worker status during edge |
| ----------------- | ----------------- | -------------- | ------------------------- |
| `prompt` | `PROMPT.md`, Prompt Reflection ready or warning-accepted, no blocking scope questions, required prompt receipts accepted, explicit human alignment approval | `prd` | `running` or `waiting_for_input` |
| `prd` | Accepted zero-gap `PRD.md`, PRD generation/reflection/tournament/evolution receipts accepted, no active product gaps | `tdd` | `running` or `waiting_for_input` |
| `tdd` | Accepted zero-gap `TDD.md`, technical review/evaluation receipts accepted, current repository evidence recorded, explicit TDD approval | `decomposition` | `running` or `waiting_for_input` |
| `decomposition` | Published `FEATURES.md`, contiguous ordered `efforts/NN_*.md`, accepted decomposition receipts, `Next effort index: 0`, explicit implementation approval | `development` | `running` or `waiting_for_input` |
| `development` | Active effort has red evidence when required, green evidence, validation record, accepted required-agent receipts, reviewer approval, scoped commit checkpoint, and no active locks | `development` until all efforts are complete | `running` or `waiting_for_input` |
| `development` | All efforts complete in numeric order, final validation summary exists, no pending receipts, no active locks, worktree/staging evidence recorded | `handover` | `running` or `waiting_for_input` |
| `handover` | Handover report published, final artifacts indexed, final status and residual risks recorded, close/archive behavior decided | `complete` | `running` then terminal |
| `complete` | Terminal workflow state | none | `succeeded`, `failed`, or `stopped` |

Workflow status and worker status are separate:

- `WorkflowStatus` describes whether the Doric workflow is running, blocked,
  waiting on a gate, failed, stopped, or complete.
- `WorkerStatus` describes the daemon-visible process/session state. It should
  become terminal only when the workflow reaches `complete`, a non-recoverable
  failure occurs, or the user stops the worker.

## Nested Workflow Model

Doric should model PRD, TDD, decomposition, development, and handover as
workflows, not as ad hoc helper routines. They have their own steps, durable
artifacts, gate criteria, required receipts, validation records, and recovery
rules.

That does not mean the first Rust implementation should begin with a generic
workflow engine, definition registry, or public workflow API. The first slice
should be one worker-local orchestrator with concrete workflow states and
reducers. The code should be written so those concrete phase workflows can be
nested later without changing package ownership or persisted evidence.

The root Doric workflow is the top-level workflow:

```text
root
  prompt
  prd
  tdd
  decomposition
  development
  handover
```

Each phase workflow owns its own local state, legal edges, gates, receipts,
artifacts, validation records, locks, and recovery metadata. The parent should
only depend on the phase workflow from the outside: start it, observe its
summary, wait for completion or a blocker, and advance the root edge when the
completion evidence satisfies the parent.

This keeps orchestration rules self-contained without forcing a broad
abstraction too early:

| Workflow | Example local steps |
| -------- | ------------------- |
| `prd` | Generate candidates, reflect, rank tournament, evolve, promote champion, await approval, done. |
| `tdd` | Generate candidates, reflect, rank tournament, evolve, promote champion, await approval, done. |
| `decomposition` | Load accepted artifacts, generate plans, validate coverage, publish feature map, publish efforts, await approval, done. |
| `development` | Select effort, acquire locks, plan tests, write red tests, implement, validate green, review, commit checkpoint, complete effort, repeat or done. |

Phase workflows communicate upward through durable events and summarized
completion evidence. A phase workflow may emit `WorkflowBlocked` or
`GateRequested`; the root workflow remains running but projects the blocker
into `STATE.md` and the daemon event stream. A phase workflow may not advance
its parent directly.

If the PRD, TDD, decomposition, and development reducers converge on the same
shape, extract a shared workflow runner then. Until that second shape exists in
code, concrete reducers are simpler and easier to verify.

## Rust Interfaces

The interface surface should start smaller than the final conceptual model.
No new public `lifecycle` workflow API is required for the first implementation
slice. The worker can project progress through existing `WorkerEvent` records
and regenerate `STATE.md` for human and agent inspection.

Do not add a public `WorkflowCommand`, `WorkflowKind`, definition key, generic
definition trait, or transition-id enum up front. The first slice needs a small
worker-local reducer boundary and durable events:

```rust
pub struct WorkflowId(String);

pub enum WorkflowStatus {
    Running,
    WaitingForInput,
    WaitingForGate,
    Blocked,
    Failed,
    Stopped,
    Complete,
}

pub enum ReceiptStatus {
    Registered,
    Spawned,
    Running,
    ReceiptSubmitted,
    Accepted,
    Rejected,
    Blocked,
}

pub struct WorkflowSummary {
    pub id: WorkflowId,
    pub parent: Option<WorkflowId>,
    pub name: String,
    pub status: WorkflowStatus,
    pub step: String,
}

pub enum WorkflowState {
    Root(RootState),
    Prd(PrdState),
    Tdd(TddState),
    Decomposition(DecompositionState),
    Development(DevelopmentState),
    Handover(HandoverState),
}

pub enum WorkflowInput {
    Start,
    AgentFinished {
        workflow_id: WorkflowId,
        role: String,
        receipt_path: String,
    },
    AnswerReceived {
        request_id: String,
        answer: String,
    },
    ArtifactReady {
        workflow_id: WorkflowId,
        path: String,
    },
    ChildCompleted {
        workflow_id: WorkflowId,
        evidence_path: String,
    },
    Stop {
        reason: String,
    },
    Recover,
}

pub enum WorkflowEvent {
    WorkflowStarted { summary: WorkflowSummary },
    StepEntered { workflow_id: WorkflowId, step: String },
    WorkflowCompleted { workflow_id: WorkflowId },
    GateRequested { workflow_id: WorkflowId, request_id: String },
    GateApproved { workflow_id: WorkflowId, request_id: String },
    GateRejected { workflow_id: WorkflowId, request_id: String, reason: String },
    ArtifactPublished { workflow_id: WorkflowId, path: String },
    ReceiptAccepted { workflow_id: WorkflowId, role: String, path: String },
    ValidationRecorded { path: String },
    LockAcquired { owner: String, path: String },
    LockReleased { owner: String, path: String },
    EffortCompleted { effort: String, commit: Option<String> },
    InputRequested { request_id: String },
    InputAnswered { request_id: String },
    ProjectionRegenerated { path: String },
    WorkflowFailed { reason: String },
}

pub fn reduce(
    state: &WorkflowState,
    input: WorkflowInput,
) -> WorkflowResult<Vec<WorkflowEvent>>;
```

This shape is intentionally ordinary Rust. A single `WorkflowInput` enum is
acceptable while there is one orchestrator boundary; it is worker-local, not a
public command protocol. Each concrete reducer can still use smaller local
input/event enums internally if that keeps the code clearer.

Artifact paths can start as strings relative to `artifacts/` or `tool-output/`.
Promote them to a richer `ArtifactRef` with kind and checksum only after a
consumer needs those fields.

Stable summary types can move into `lifecycle` later when the CLI or daemon has
a concrete display or wire-protocol need. Until then, avoid widening the shared
contract package.

Target worker module boundaries:

| Module | Responsibility |
| ------ | -------------- |
| `worker::workflow` | First-slice orchestrator boundary. Converts worker runtime inputs into `WorkflowInput`, dispatches reducers, writes events, and emits worker events. |
| `worker::workflow::state` | Serializable canonical state with root state, phase workflow states, active gates, effort order, receipt rows, locks, validation records, artifact paths, and recovery metadata. |
| `worker::workflow::store` | Append-only durable event log and optional snapshot under `state/workflow/`; exposes replay into canonical state. |
| `worker::workflow::root` | Root workflow reducer for `prompt -> prd -> tdd -> decomposition -> development -> handover -> complete`. |
| `worker::workflow::prd`, `tdd`, `decomposition`, `development`, `handover` | Concrete phase workflow states and reducers. Extract shared runner code only after duplication proves it. |
| `worker::workflow::projection` | Regenerates `<run>/STATE.md` from canonical state and event history. |
| `worker::workflow::artifacts` | Writes workflow artifacts under the logical run root and records promoted artifact paths. |
| `worker::workflow::agents` | Boundary for spawning specialized agents, collecting receipts, and mapping unavailable spawn support into blocked required-agent rows. |
| `worker::workflow::gates` | Human-gate request creation, request id correlation, stale-answer rejection, and answer-to-command mapping. |
| `worker::workflow::runtime` | Integrates the orchestrator with the existing worker session loop and lifecycle event emission. |

Reducers must be deterministic over persisted input. Workflow runners may
perform side effects, but only by appending workflow events after successful
artifact writes, validation, agent receipt review, lock changes, child workflow
completion, or human-gate commands.

## Persistence And Projection

The daemon prepares a per-worker layout:

```text
<worker-root>/<worker-id>/
  repo/
  state/
  artifacts/
  tool-output/
```

The workflow architecture maps the logical run root to `artifacts/`:

```text
<run>/                 == <worker-root>/<worker-id>/artifacts/
<run>/PROMPT.md        == artifacts/PROMPT.md
<run>/prd/PRD.md       == artifacts/prd/PRD.md
<run>/tdd/TDD.md       == artifacts/tdd/TDD.md
<run>/FEATURES.md      == artifacts/FEATURES.md
<run>/efforts/*.md     == artifacts/efforts/*.md
<run>/STATE.md         == artifacts/STATE.md
```

Machine state and event history live outside the logical run root. The first
implementation should use one event log and one optional snapshot:

```text
state/workflow/events.jsonl
state/workflow/snapshot.json
state/workflow/projection-manifest.json
```

Every event should carry enough structured context to reconstruct nesting when
it exists: `workflow_id`, optional `parent_workflow_id`, workflow name, local
step, request id, role, artifact path, and safe correlation ids. Split this
into per-workflow logs only if one log becomes operationally painful.

Tool traces and command output remain under `tool-output/`. Workflow artifacts
should link to tool-output paths when evidence matters, but they should not move
tool-owned output into the run root unless publishing a deliberate durable
artifact.

`STATE.md` is always regenerated from canonical workflow state and event
history. Agents and humans read it as the durable audit projection, but manual
edits to it are not transition authority. If `STATE.md` is missing or stale,
the worker should regenerate it before taking the next workflow input and emit
`ProjectionRegenerated`.

## Agent And Receipt Lifecycle

Required-agent proof is a first-class workflow state concern. A coordinator
summary cannot substitute for a required sub-agent receipt.

Receipt rows use one standard lifecycle across workflows:

| Status | Meaning | Legal next statuses |
| ------ | ------- | ------------------- |
| `registered` | The orchestrator recorded that a role is required before a gate can pass. | `spawned`, `blocked` |
| `spawned` | A concrete agent execution was requested and has an identity or run handle. | `running`, `blocked` |
| `running` | The agent accepted work and has not produced a receipt. | `receipt_submitted`, `blocked` |
| `receipt_submitted` | The agent produced a durable receipt artifact for coordinator review. | `accepted`, `rejected` |
| `accepted` | The coordinator accepted the receipt as satisfying the required role. | terminal for that row |
| `rejected` | The receipt is insufficient and must be repaired or superseded. | `spawned`, `blocked` |
| `blocked` | The required role cannot currently proceed, including missing spawn support. | terminal until an explicit recovery command changes it |

If required sub-agent spawning is unavailable, the orchestrator must register
the row as `blocked`, regenerate `STATE.md`, emit a blocked event, and stop
workflow advancement. It must not fabricate spawn ids, receipts, or accepted
proof.

## Human Gates

Human gates use the existing lifecycle input pattern but add workflow-specific
correlation:

1. The reducer decides a gate is required and emits `GateRequested`.
2. The worker maps that event to a daemon-visible `waiting_for_input` worker
   event with a stable request id.
3. The daemon stores the pending request id and rejects duplicate or stale
   answers through its existing input validation path.
4. The CLI renders the question and later sends `AnswerInput`.
5. The worker converts the answer into the active workflow's local command,
   such as a gate approval, gate rejection, or artifact-revision command.
6. The reducer validates that the answer request id still matches the active
   workflow and legal edge before advancing.

Gate answers are scoped to one active request. If the workflow cursor has moved,
the request id has already been answered, or the gate no longer matches the
workflow plus legal edge, the answer is stale and must be rejected.

Required human-gated edges:

- Prompt-to-PRD alignment.
- PRD-to-TDD approval.
- TDD-to-decomposition approval.
- Decomposition-to-development approval.
- Any safety, destructive action, validation substitution, scope expansion, or
  handover close/archive decision that requires human judgment.

## Event And Observability Model

Workflow events are canonical inside `state/workflow/events.jsonl`. The worker
also projects operator-relevant events into the existing daemon stream as
existing `WorkerEvent` records. The daemon stream can remain name/message based
for the first implementation.

Target event names:

| Workflow event | Daemon stream name | Notes |
| -------------- | ------------------ | ----- |
| `WorkflowStarted` | `workflow_started` | Message includes workflow id, parent id, and current step. |
| `StepEntered` | `workflow_step_entered` | Message includes workflow id and local step. |
| `GateRequested` | `waiting_for_input` | Includes request id and gate prompt. |
| `GateApproved` | `workflow_gate_approved` | Message includes the legal edge rendered for humans. |
| `GateRejected` | `workflow_gate_rejected` | Message includes the legal edge and safe reason. |
| `ArtifactPublished` | `workflow_artifact_published` | Message includes artifact path relative to `<run>`. |
| `ReceiptAccepted` | `workflow_agent_receipt_accepted` | Message includes role and receipt artifact. |
| `ValidationRecorded` | `workflow_validation_recorded` | Message includes validation record path. |
| `LockAcquired` | `workflow_lock_acquired` | Message includes owner and path. |
| `LockReleased` | `workflow_lock_released` | Message includes owner and path. |
| `EffortCompleted` | `workflow_effort_completed` | Message includes effort id and commit checkpoint. |
| `InputAnswered` | `answer_provided` | Existing daemon-side event remains useful. |
| `ProjectionRegenerated` | `workflow_projection_regenerated` | Message includes `STATE.md`. |
| `WorkflowCompleted` | `succeeded` | Only root workflow completion maps to terminal worker success. |
| `WorkflowFailed` | `failed` | Worker sends terminal failure only for unrecoverable failure. |

The daemon may later mirror opaque workflow summary fields into list output,
for example root step, active workflow, active edge, or active effort. Such
mirroring is display state only. If the daemon restarts with no worker replay
support, it must not invent workflow state from display cache.

Workflow event payloads should be structured internally even when the current
wire event is name/message based. Worker-local source events should carry
`workflow_id`, optional `parent_workflow_id`, workflow name, step, edge,
artifact path, request id, role, and safe correlation ids without parsing
prose. Public lifecycle events should expose only the subset a client needs.

## Recovery

Recovery starts from worker-owned persisted state:

1. Load `state/workflow/events.jsonl`.
2. Replay events through deterministic reducers to reconstruct canonical state.
3. Compare replayed state with `state/workflow/snapshot.json` if a snapshot
   exists; on mismatch, prefer event replay and write a diagnostic event.
4. Verify required artifacts referenced by accepted state still exist under
   `artifacts/`.
5. Regenerate `artifacts/STATE.md`.
6. Resume at the same root step, phase workflow cursor, gate wait,
   required-agent block, active effort, or terminal state.

Recovery must be idempotent. Re-running projection generation, list rendering,
or event replay must not re-spawn agents, re-run tools, re-answer gates, or
create duplicate commit checkpoints.

For incomplete side effects, use explicit event pairs:

- Write artifact, then append `ArtifactPublished`.
- Acquire lock, then append `LockAcquired`.
- Submit receipt, then append `ReceiptAccepted` only after review.
- Record validation, then append `ValidationRecorded`.
- Commit effort, then append `EffortCompleted`.

If a side effect happened but its event did not persist, the supervisor must
repair by inspecting durable evidence and appending a recovery event only when
the evidence is unambiguous. Otherwise, block and ask for human review.

## Migration Path

1. Add a concrete worker-local workflow module.
   - Add `worker::workflow` with canonical state, reducer dispatch, event
     append/replay, optional snapshot, and `STATE.md` projection.
   - Keep all new workflow types worker-local unless an existing CLI or daemon
     call needs them.
   - Keep one event log at `state/workflow/events.jsonl`.

2. Wrap prompt ingestion as the first root workflow step.
   - Treat existing `RuntimePromptRunner` as the prompt executor behind the
     workflow boundary.
   - Change prompt completion from worker terminal success to root workflow
     advancement only after the prompt-to-PRD gate is approved.
   - Keep the worker `running` or `waiting_for_input` until the full workflow
     completes or stops.

3. Add PRD as the first concrete phase workflow.
   - Persist PRD candidate artifacts under `artifacts/prd/`.
   - Require zero-gap accepted `PRD.md` and accepted receipts before the root
     advances to `tdd`.

4. Add TDD as a concrete phase workflow.
   - Use root step id `tdd`.
   - Persist accepted artifact as `artifacts/tdd/TDD.md`.
   - Require TDD approval before decomposition.

5. Add decomposition as a concrete phase workflow.
   - Publish `FEATURES.md` and contiguous `efforts/NN_*.md`.
   - Persist ordered effort state and `Next effort index: 0`.
   - Require implementation approval before development.

6. Add development as a concrete phase workflow.
   - Process efforts strictly in numeric order.
   - Require red/green evidence, validation records, accepted receipts, review,
     active-lock cleanup, and one scoped commit checkpoint per effort.

7. Add handover and complete.
   - Publish a final handover report, artifact index, validation summary,
     residual risks, and close/archive decision.
   - Send the worker terminal success event only after root workflow completion.

8. Extract shared workflow runner code only after concrete duplication exists.
   - If PRD, TDD, decomposition, and development reducers converge on the same
     orchestration shape, extract a worker-local runner.
   - Do not introduce public lifecycle workflow summary types until a CLI,
     daemon, or wire-protocol consumer needs them.

9. Add daemon display mirroring only after worker state is authoritative.
   - Mirror current root step, active workflow, and active gate as optional
     list fields.
   - Do not move transition logic into daemon code.

## Failure Modes

| Failure | Required behavior |
| ------- | ----------------- |
| Prompt completion marks worker succeeded before PRD | Treat as the current prompt-only gap; target orchestrator must keep the worker running and advance the root workflow step instead. |
| Stale human answer arrives after gate changed | Reject by request id and legal edge; emit a safe rejection event. |
| Required sub-agent spawn support is unavailable | Register the required row as `blocked`, regenerate `STATE.md`, and block the workflow. |
| Required receipt is rejected | Keep the workflow active; do not advance until a replacement receipt is accepted or the run is explicitly stopped. |
| `STATE.md` is manually edited | Ignore it for transition authority; regenerate from canonical state. |
| `STATE.md` is missing or stale | Regenerate before the next action and emit `ProjectionRegenerated`. |
| Artifact path points outside `artifacts/` | Reject the artifact path and block until repaired. |
| Tool output is needed as durable evidence | Link to `tool-output/` or publish a deliberate artifact; do not blur tool trace ownership with run artifacts. |
| Daemon restart loses in-memory registry state | Worker state remains canonical; daemon may need a reattach/replay protocol before it can show current workflow state. |
| Worker restarts after side effect but before event append | Repair only from unambiguous durable evidence; otherwise block for review. |
| Development effort tries to skip order | Reducer rejects because `Next effort index` selects the only legal effort. |
| Active locks remain after effort validation | Do not complete the effort; require release or explicit failure handling. |
| Final validation fails during handover | Stay in `handover` or `development` repair path; do not emit terminal success. |

## Research Basis

This design follows durable workflow architecture patterns without importing an
external workflow engine into Doric:

- [Temporal workflow determinism](https://docs.temporal.io/workflow-definition)
  motivates a deterministic workflow reducer and replay from event history.
  Doric should keep non-deterministic model, tool, filesystem, and Git effects
  outside the reducer and append events only after side effects succeed.
- [AWS Step Functions state machines](https://docs.aws.amazon.com/step-functions/latest/dg/concepts-statemachines.html)
  motivates explicit root step ids, legal edges, input/output artifacts, and
  terminal states.
- [AWS Step Functions callback waits](https://docs.aws.amazon.com/step-functions/latest/dg/connect-to-resource.html)
  maps well to Doric human gates: generate a stable token/request id, wait for
  a callback answer, and reject stale or mismatched answers.
- [Tokio channels](https://tokio.rs/tokio/tutorial/channels) support the
  worker's input/event boundary: many producers can send inputs to one
  orchestrator task while preserving one reducer authority.
- [Tokio graceful shutdown](https://tokio.rs/tokio/topics/shutdown) informs
  worker stop handling: detect shutdown, notify owned tasks, wait for
  projection/event flushing, then send a terminal worker status.
- [CloudEvents](https://cloudevents.io/) motivates structured workflow events
  with stable metadata rather than prose-only messages.
- [OpenTelemetry traces](https://opentelemetry.io/docs/concepts/signals/traces/)
  motivates correlation ids, parent/child spans, and links between CLI
  dispatch, daemon routing, worker orchestration, agent executions, and tool
  calls.
- [OpenAI Agents handoffs](https://openai.github.io/openai-agents-python/handoffs/)
  supports modeling specialist delegation as explicit handoff/agent execution
  boundaries with durable receipts, not hidden coordinator narration.

## Acceptance Scenarios For Implementation

Future implementation should include tests or harness scenarios for:

- Prompt completion does not terminate the worker; it advances the workflow
  cursor only through the approved prompt-to-PRD gate.
- Stale human answers are rejected when the active request id or legal edge no
  longer matches.
- A required-agent row in `blocked`, `registered`, `spawned`, `running`,
  `receipt_submitted`, or `rejected` prevents workflow advancement.
- `STATE.md` regenerates from machine state and event history, and manual edits
  do not affect transition legality.
- Replay after restart restores the same root step, active workflow, active
  gate, active effort, pending input, lock state, receipt status, and
  validation records.
- Development efforts execute in numeric order and cannot skip
  `Next effort index`.
- Handover cannot reach `complete` until all efforts, validation, artifact
  index, final report, and close/archive decisions are recorded.
