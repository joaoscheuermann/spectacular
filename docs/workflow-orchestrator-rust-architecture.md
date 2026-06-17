# Workflow Orchestrator Rust Architecture

Status: target architecture with a concrete first Rust implementation slice.
This document ties the phase-specific Doric workflow documents into one
Rust-owned workflow architecture. The current implementation enforces the
worker-local root workflow path from prompt through handover/complete with
durable evidence gates, while preserving a clean path to future generic nested
workflow machinery.

Current implementation evidence:

- `docs/architecture-and-packages.md` defines `worker` as the daemon-controlled
  runtime owner, `daemon` as process lifecycle and event routing, and
  `lifecycle` as the shared contract package.
- `packages/worker/src/workflow/` contains the first worker-local workflow
  orchestrator slice: canonical root state, reducer dispatch, append-only event
  storage, snapshot persistence, `STATE.md` projection, prompt-to-PRD gate
  handling, Prompt Reflection and prompt artifact readiness validation for
  `PROMPT.md`,
  prompt requirements receipt proof, required-agent receipt lifecycle, PRD/TDD
  artifact promotion, PRD-to-TDD and TDD-to-decomposition gates,
  decomposition-to-development approval gating,
  effort-order reducer checks, development effort start, validation record
  tracking, worktree/staging checkpoint evidence, active lock tracking, and
  commit-checkpoint completion guards. Development completion now
  requires typed red or red-exception evidence, green evidence, reviewer
  approval evidence, accepted effort-scoped development role receipts, durable
  worktree and staging checkpoint artifacts, no active locks, and a non-empty
  commit checkpoint. The orchestrator can write Git-backed worktree and
  staging checkpoint artifacts from read-only status and diff commands before
  recording those checkpoints. It can also run injected repository-local
  validation commands, write red/green/review validation artifacts, and record
  validation only when the command status matches the expected evidence kind.
  A scoped Git commit checkpoint boundary can inspect staged paths, reject
  out-of-scope staged files, create the commit through an injected Git runner,
  resolve `HEAD`, and record effort completion with the resulting commit hash.
  A staging-aware commit path can also inspect Git status for explicit
  reviewer-provided stage paths, stage only those paths, re-check the staged
  path set against the allowed scope, and then reuse the same commit boundary.
  Final effort completion enters `handover`; the worker can generate the
  handover report, artifact index, and final validation summary from canonical
  workflow state, and explicit close approval completes the root workflow.
  Required-agent rows now preserve a concrete spawn handle as evidence when a
  phase-agent runner starts a role, and receipt submission replaces that
  handle with the durable receipt path. Public orchestrator methods now verify
  published artifacts, receipt artifacts, effort files, validation records, and
  checkpoint records exist under `artifacts/` before appending events that
  depend on them.
- `packages/worker/src/runtime.rs` runs repository preparation, starts the root
  workflow, runs the prompt runner, publishes durable prompt completion into
  workflow state only when `PROMPT.md` records the canonical prompt sections,
  explicitly states that no blocking scope questions remain, and records Prompt
  Reflection status `ready_for_extraction` or `ready_with_warnings`; then it
  waits for prompt-to-PRD alignment instead of treating prompt completion as
  terminal worker success.
  It also has a production repo-preparer adapter that uses the worker Git
  boundary for external clone execution, plus an injected workflow runner
  boundary. The production
  workflow runner can execute PRD, TDD, and decomposition planning phase
  agents, write durable receipt artifacts under `artifacts/<phase>/receipts/`,
  and submit and accept those receipts through workflow state in canonical role
  order, including PRD/TDD proximity review between generation and reflection.
  Later same-phase roles wait while an earlier role is started, running,
  submitted, blocked, or rejected instead of accepted. The runner can
  also process structured phase-agent output with explicit `promotion_ready`
  plus typed `promotion_evidence`: generation, proximity, reflection, ranking,
  evolution, `zero_gap`, and `champion_confident` from the PRD/TDD promotion
  role for PRD/TDD publication, and extraction, impact, effort-plan,
  proximity, ranking, evolution, and `coverage_validated` from the
  decomposition publisher role for decomposition publication. It
  writes phase-owned artifact payloads, limits non-promotion planning roles to
  phase-local agent-record, candidate, log, manifest, or proximity artifacts, rejects
  accepted artifact path writes unless the promotion role sets
  `promotion_ready` with required evidence, allows PRD/TDD promotion roles to
  write active gap reports at `prd/GAPS.md` or `tdd/GAPS.md`, rejects
  `promotion_ready` payloads that include an active PRD/TDD gap report,
  publishes ready `prd/PRD.md` and `tdd/TDD.md` artifacts through the existing
  reducer gates only when the relevant gap report is absent, empty, or
  explicitly superseded, and records a
  decomposition package from
  `FEATURES.md` plus contiguous `efforts/NN_*.md` files only after pre-write
  validation proves the listed effort order and durable effort artifact
  payloads match. Unstructured
  phase-agent output remains receipt-only
  and cannot trigger automatic publication. After decomposition approval, the
  runner starts the next legal `todo` development effort before spawning
  development roles. Development required-agent roles now execute for the
  active effort and produce durable accepted receipts. Structured development
  role output can now request worker-executed red/green/review validation
  commands; validation records are written only when the injected command
  runner returns the expected status. When a Git runner is available, the
  workflow runner then captures Git-backed worktree/staging checkpoint evidence
  after accepted development receipts and red/green/review evidence exist.
  A development reviewer can emit a structured scoped commit request with
  explicit stage paths; after checkpoint evidence exists, the runner inspects
  status for those paths, stages only those paths, verifies the staged path
  set against the allowed scope, and completes the effort through the existing
  scoped Git commit boundary. The workflow runner also
  generates ready handover artifacts once final-effort completion has legally
  entered `handover`, and it does not re-open the close gate after a human
  rejection. The runtime also handles daemon-sent typed workflow
  commands through `packages/worker/src/workflow_command.rs`; those commands
  call the existing orchestrator entrypoints for phase artifact publication,
  receipt submission or review, decomposition effort-order recording,
  development effort start, command-backed validation, Git checkpoint capture,
  and scoped Git commit completion.
- `packages/worker/src/main.rs`, `packages/worker/src/process.rs`, and
  `packages/worker/src/session.rs` provide the `doric-worker` process
  composition root: daemon launch argument parsing, tonic worker-session
  transport, production repo preparation, production prompt execution, and the
  production workflow runner. This makes the spawned worker process attach to
  the daemon session instead of being an empty binary. Development phase-agent
  execution currently covers receipt generation and structured validation
  command requests, while PRD/TDD phase-agent execution now covers the linear
  generation, proximity, reflection, tournament, evolution, and promotion
  receipt path, and decomposition execution covers extraction, impact,
  planning, proximity, validation, ranking, evolution, and publishing receipts.
  The implementation remains concrete and linear rather than a generic dynamic
  nested supervisor engine.
- `packages/worker/src/prompt_runner.rs` contains the session-facing
  production prompt runner. It selects the worker-local runtime from persisted
  config and model cache, constructs the worker provider, builds
  `prompt_agent_for_worker`, runs the generic `agent::Agent`, extracts the
  assistant response, writes `artifacts/PROMPT.md`, and reports prompt events.
  The workflow publish boundary then blocks PRD alignment for prompt artifacts
  whose canonical sections are missing, whose open questions do not explicitly
  state `Blocking scope questions: none`, or whose Prompt Reflection status is
  missing, `needs_more_questions`, `blocked`, or `rejected`; accepted prompt
  artifacts also submit and accept the prompt requirements receipt using
  `PROMPT.md` as the durable evidence before the alignment gate.
- `packages/worker/src/workflow/agents.rs` contains the workflow execution
  boundary for required-agent rows. Its production runner writes real PRD, TDD,
  and decomposition receipt artifacts from model output using read-only
  planning tools in canonical role order, including PRD/TDD generation,
  proximity, reflection, tournament, evolution, and promotion, and
  decomposition extraction, impact, planning, proximity, validation, ranking,
  evolution, and publication. Later role prompts name the accepted prior
  same-phase receipt paths they should read, while
  `packages/worker/src/workflow/phase_output.rs` parses
  optional structured artifact payloads and advances publication only after the
  phase promotion role's accepted receipt exposes the required typed promotion
  evidence validated by `packages/worker/src/workflow/phase_promotion.rs`
  (generation/proximity/reflection/ranking/evolution plus `zero_gap` and
  `champion_confident` for PRD/TDD, and
  extraction/impact/effort-plan/proximity/ranking/evolution plus
  `coverage_validated` for decomposition), while
  `packages/worker/src/workflow/phase_artifact.rs` writes
  only phase-owned artifact paths, rejects accepted artifact path writes without
  both the promotion role and `promotion_ready`, and pre-validates
  decomposition effort payload/list contiguity before writing artifacts.
  Development roles are scoped to the active effort, use role-appropriate tool
  registration, support optional scoped repair-writer rows for concrete
  validator or reviewer findings, and write accepted receipt artifacts without
  satisfying validation, checkpoint, or commit gates by themselves.
- `packages/cli/src/main/cli_types.rs`,
  `packages/cli/src/main/lifecycle.rs`,
  `packages/cli/src/main/lifecycle_client.rs`, and
  `packages/cli/src/main/lifecycle_workflow.rs` expose narrow `doric workflow`
  subcommands for phase-artifact publication, receipt submission or review,
  effort-order recording, development effort start, command-backed validation,
  Git checkpoint capture, and scoped Git commit completion. The CLI formats
  and forwards commands; worker workflow reducers still own legality.
- `packages/worker/src/repo.rs` contains the worker layout and clone boundary,
  including an OS-backed `GitCommandRunner` implementation for production
  command execution and fakeable tests for clone, workflow checkpoint, and
  commit paths.
- `packages/worker/src/agents/prompt.rs` writes `PROMPT.md` under the worker
  `artifacts/` directory.
- `packages/daemon/src/service.rs`, `packages/daemon/src/worker_session.rs`,
  `packages/daemon/src/worker_session_command.rs`,
  `packages/daemon/src/tonic_service.rs`, and
  `packages/daemon/src/registry.rs` route lifecycle commands, serve the
  bidirectional worker-session gRPC transport, stream worker events, validate
  input answers, route typed workflow commands through the attached worker
  session, and preserve workflow progress event names without owning Doric
  workflow legality.

## Current Gap

The phase docs define the root Doric workflow:

```text
prompt -> prd -> tdd -> decomposition -> development -> handover -> complete
```

The code now has an executable root workflow reducer for the full root phase
order from `prompt` through `complete`. Prompt completion no longer marks the worker
terminal. It publishes only an existing durable `PROMPT.md` with the canonical
prompt sections, explicit `Blocking scope questions: none` evidence, and Prompt
Reflection status `ready_for_extraction` or `ready_with_warnings`; writes and
accepts the prompt requirements receipt; regenerates `STATE.md`; and waits for
the prompt-to-PRD alignment gate. Missing sections, blocking open-question
evidence, or blocking Prompt Reflection statuses are rejected before workflow
events are persisted.
After approval, the root cursor enters `prd` and
registers PRD required-agent rows. Reducer inputs can then promote accepted
`prd/PRD.md` and `tdd/TDD.md` artifacts, enforce receipt status transitions,
request PRD-to-TDD and TDD-to-decomposition gates after accepted artifacts and
receipts, record contiguous decomposition effort order, and require explicit
decomposition-to-development approval before development starts. In
development, the reducer now requires the selected effort to become
`in-progress`, records validation evidence under `validation/`, tracks active
locks, rejects completion with active locks, records worktree/staging
checkpoint evidence, and requires a non-empty commit checkpoint before
advancing `Next effort index`. Validation evidence is typed as `red`,
`red_exception`, `green`, or `review`; the reducer enforces red or
red-exception before green, green before review, worktree and staging checkpoint
artifacts, accepted effort-scoped development role receipts, and then the
commit checkpoint. Completing the final effort enters `handover`; the
orchestrator can generate `HANDOVER.md`, `ARTIFACTS.md`, and a final validation
summary under `validation/` from canonical state, then request the close
decision. Public orchestrator entrypoints reject missing or out-of-run artifact
evidence before persisting artifact, receipt, effort, validation, checkpoint,
or handover events. Approval completes the root workflow and maps to terminal
worker success.

The worker now has a concrete planning runner for PRD, TDD, and decomposition
phase agents. It writes receipts, and when a phase agent returns the structured
artifact envelope with explicit `promotion_ready` plus required typed
`promotion_evidence`, it can write and publish accepted `prd/PRD.md` or
`tdd/TDD.md` from terminal-role generation, proximity, reflection, ranking,
evolution, `zero_gap`, and `champion_confident` evidence, or record
`FEATURES.md` plus contiguous effort files for decomposition from
decomposition-publisher extraction, impact, effort-plan, proximity, ranking,
evolution, and `coverage_validated` evidence.
Decomposition publication
pre-validates contiguous two-digit effort paths and requires the `efforts`
list to match the `efforts/NN_*.md` artifact payloads before any phase artifact
is written. Non-promotion phase roles can write durable agent-record,
candidate, log, manifest, and proximity artifacts under their phase-local directories, but
accepted artifact paths require both the promotion role and `promotion_ready`
true. PRD/TDD promotion roles may write `prd/GAPS.md` or `tdd/GAPS.md` when
active blockers remain, but a `promotion_ready` payload that includes an active
gap report is rejected before artifacts or receipts are written. Previously
persisted active gap reports prevent publication until empty or explicitly
superseded by the current champion. It includes linear PRD/TDD
generation, proximity, reflection, tournament, evolution, and promotion receipt
rows and linear decomposition extraction, impact, planning, proximity,
validation, ranking, evolution, and publishing rows. It does not yet replace
those concrete role rows with a generic adaptive nested supervisor engine, and
unstructured receipt text cannot trigger automatic publication. The runtime
can invoke the workflow-runner boundary after gate answers; that boundary advances
required-agent rows in canonical order, records spawn-backed rows with run
handles, writes, submits, and accepts receipt artifacts, advances structured
ready artifacts, starts the next legal development effort after decomposition
approval, runs active-effort development roles for durable receipts, executes
structured red/green/review validation command requests through injected
runners, captures Git-backed worktree/staging checkpoint evidence when
validation and receipt proof is complete, applies structured reviewer commit
requests by inspecting and staging explicit reviewer-provided paths before
scoped staged-path verification, generates ready handover artifacts, and avoids
re-opening a rejected handover close gate. Development
role receipts do not fabricate proof for staging gates. The worker records
durable Git-backed worktree/staging checkpoint artifacts,
command-backed red/green/review validation artifacts, and scoped commit
checkpoints through injected runners.
It can also enforce caller-provided stage and staged-path scopes and create a
Git commit checkpoint through an injected runner. The Git runner now has a
production OS
implementation, and the `doric-worker` binary entry point now parses daemon
launch arguments, attaches to the daemon's `WorkerSessionService`, and runs the
existing worker-session runtime with production prompt and phase-agent runners.
The lifecycle protobuf and daemon service now expose a narrow `WorkflowCommand`
route for forwarding phase-artifact, receipt-review, effort-order, development
effort start, command-backed validation, Git checkpoint, and scoped commit
completion commands to the worker, where the existing orchestrator validates
them. The CLI exposes matching `workflow` subcommands. Development agents can
now request structured red/green/review validation commands and reviewer-scoped
stage/commit checkpoints. The worker must continue blocking or failing
explicitly rather than fabricating proof when required execution is
unavailable.

The remaining architectural work is not to replace the first-slice reducers
with a generic workflow engine. The worker-owned orchestrator should keep
running one legal root edge at a time, persisting canonical state and events,
blocking on human gates and required-agent proof, regenerating `STATE.md`, and
emitting lifecycle progress through the existing event stream. Shared nested
workflow machinery should be extracted only after the concrete PRD, TDD,
decomposition, development, and handover reducers produce enough duplication to
justify it.

## Ownership Model

The orchestrator must preserve the package boundaries already documented in
`docs/architecture-and-packages.md`.

| Package | Target workflow responsibility |
| ------- | ------------------------------ |
| `worker` | Owns canonical workflow execution for one worker run. It hosts the future `worker::workflow` module, performs reducer-based legal transitions, persists machine state, writes artifacts, regenerates projections, handles human-gate waits, and recovers from persisted events. |
| `daemon` | Owns worker process lifecycle, worker registry, worker-session attachment, input/workflow command routing, event replay, and optional opaque workflow-state mirroring for list output. It must not decide whether `prd` can start, whether the TDD-to-decomposition edge is legal, or whether an effort is complete. |
| `lifecycle` | Owns shared wire-safe domain contracts used across `cli`, `daemon`, and `worker`: worker identity/status/events, the narrow workflow command routing envelope, plus additive workflow display fields only when those become public lifecycle contracts. |
| `cli` | Renders lifecycle streams, lists workers, dispatches work, answers input requests, and forwards typed workflow commands. It may display workflow root step and gate status, but it does not compute legal transitions. |
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
| `prd` | Accepted champion-confident zero-gap `PRD.md`, PRD generation/proximity/reflection/tournament/evolution receipts accepted, `prd/GAPS.md` absent, empty, or superseded | `tdd` | `running` or `waiting_for_input` |
| `tdd` | Accepted champion-confident zero-gap `TDD.md`, TDD generation/proximity/reflection/tournament/evolution receipts accepted, current repository evidence recorded, `tdd/GAPS.md` absent, empty, or superseded, explicit TDD approval | `decomposition` | `running` or `waiting_for_input` |
| `decomposition` | Published `FEATURES.md`, contiguous ordered `efforts/NN_*.md`, accepted decomposition receipts, `Next effort index: 0`, explicit implementation approval | `development` | `running` or `waiting_for_input` |
| `development` | Active effort has red evidence when required, green evidence, validation record, accepted required-agent receipts, reviewer approval, durable worktree/staging checkpoint evidence, scoped commit checkpoint, and no active locks | `development` until all efforts are complete | `running` or `waiting_for_input` |
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

The interface surface should stay smaller than the final conceptual model. The
worker projects progress through existing `WorkerEvent` records and regenerates
`STATE.md` for human and agent inspection. The public lifecycle surface now has
one narrow workflow API: a `WorkflowCommand` routing envelope that lets the
daemon forward phase-artifact, receipt-review, and effort-order commands to the
attached worker. It is not a workflow definition API, and the daemon must not
interpret command legality beyond worker identity, terminal state, and envelope
shape.

Do not add a public `WorkflowKind`, definition key, generic definition trait,
or transition-id enum up front. The first slice needs a small worker-local
reducer boundary and durable events:

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

pub enum ValidationKind {
    Red,
    RedException,
    Green,
    Review,
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
    PromptArtifactReady { path: String },
    PhaseArtifactReady { step: String, path: String },
    AnswerReceived {
        request_id: String,
        answer: String,
    },
    RegisterRequiredAgent { role: String },
    AgentSpawned { role: String, handle: String },
    AgentRunning { role: String },
    ReceiptSubmitted { role: String, path: String },
    ReceiptAccepted { role: String, path: String },
    ReceiptRejected { role: String, reason: String },
    RequiredAgentBlocked { role: String, reason: String },
    EffortOrderApproved { features_path: String, efforts: Vec<String> },
    EffortStarted { effort: String },
    ValidationRecorded { effort: String, kind: ValidationKind, path: String },
    CheckpointRecorded {
        effort: String,
        worktree_path: String,
        staging_path: String,
    },
    LockAcquired { owner: String, path: String },
    LockReleased { owner: String, path: String },
    EffortCompleted { effort: String, commit: Option<String> },
    HandoverReady {
        report_path: String,
        artifact_index_path: String,
        validation_summary_path: String,
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
    ReceiptStatusChanged {
        workflow_id: WorkflowId,
        role: String,
        status: ReceiptStatus,
        path: Option<String>,
        reason: Option<String>,
    },
    ReceiptAccepted { workflow_id: WorkflowId, role: String, path: String },
    RequiredAgentBlocked { workflow_id: WorkflowId, role: String, reason: String },
    ValidationRecorded { effort: String, kind: ValidationKind, path: String },
    CheckpointRecorded {
        effort: String,
        worktree_path: String,
        staging_path: String,
    },
    LockAcquired { owner: String, path: String },
    LockReleased { owner: String, path: String },
    EffortOrderRecorded { efforts: Vec<String>, next_effort_index: usize },
    EffortStarted { effort: String },
    EffortCompleted { effort: String, commit: Option<String> },
    InputRequested { request_id: String },
    InputAnswered { request_id: String },
    ProjectionRegenerated { path: String },
    WorkflowBlocked { workflow_id: WorkflowId, reason: String },
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

Artifact paths start as strings relative to `artifacts/`. Tool output evidence
must be copied or summarized into a deliberate artifact before it becomes
workflow state. Promote paths to a richer `ArtifactRef` with kind and checksum
only after a consumer needs those fields.

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
| `worker::workflow::phase` | First-slice root phase helpers for accepted artifact paths and ready-gate generation. |
| `worker::workflow::agent_prompt` | Composes phase-agent system and user prompts, including upstream artifacts, prior same-phase receipt handoffs, role-specific phase guidance, structured artifact, development validation-command, and reviewer stage/commit-request output contracts. |
| `worker::workflow::development_output` | Parses structured development validation and reviewer stage/commit request payloads into typed workflow requests scoped to the active development role. |
| `worker::workflow::phase_artifact` | Validates and writes structured phase-agent artifact payloads under `artifacts/`, limits non-promotion roles to phase-owned agents/candidate/log/manifest/proximity paths, allows only PRD/TDD promotion roles to write root gap reports, rejects accepted artifact paths without both the promotion role and `promotion_ready`, and pre-validates decomposition effort payload/list contiguity before any artifact writes. |
| `worker::workflow::phase_gap` | Reads PRD/TDD gap reports and rejects active gap reports when a phase agent claims `promotion_ready`. |
| `worker::workflow::phase_promotion` | Owns typed promotion evidence validation and promotion-role checks for PRD, TDD, and decomposition publication. |
| `worker::workflow::phase_output` | Parses optional structured phase-agent output envelopes, writes durable receipt artifacts, exposes typed development proof requests, and advances PRD/TDD/decomposition publication only from the accepted promotion-role `promotion_ready` receipt with required evidence accepted by `phase_promotion` and no active PRD/TDD gap report. |
| `worker::workflow::receipt` | Required-agent roles, prompt requirements receipt events, and receipt status transition guards. |
| `worker::workflow::checkpoint` | Git-backed worktree/staging checkpoint artifact writer using read-only status and diff commands. |
| `worker::workflow::validation` | Command-backed validation artifact writer using injected repository-local validation runners and expected status checks for red, green, and review evidence. |
| `worker::workflow::commit` | Scoped Git commit checkpoint boundary that can inspect reviewer-provided stage paths, stage only those paths, verify staged paths against allowed scopes, create the commit through an injected Git runner, resolve the commit hash, and record effort completion. |
| `worker::workflow::development` | First-slice development effort guards for current effort, validation records, worktree/staging checkpoint evidence, active locks, and commit checkpoints. |
| `worker::workflow::development_automation` | Runner-side readiness checks for development proof automation such as Git checkpoint capture after receipts and validation evidence are complete, and scoped staging plus commit completion after reviewer commit requests. |
| `worker::workflow::handover` | First-slice handover artifact generation, artifact checks, and close-decision gate. |
| `worker::workflow::prd`, `tdd`, `decomposition`, `development`, `handover` | Concrete phase workflow states and reducers. Extract shared runner code only after duplication proves it. |
| `worker::workflow::projection` | Regenerates `<run>/STATE.md` from canonical state and event history. |
| `worker::workflow::artifacts` | Validates that workflow evidence paths are relative to `artifacts/` and exist before the orchestrator records published artifacts, receipt files, effort files, validation records, checkpoint records, or recovery projections that depend on them; also enforces canonical `PROMPT.md` sections, explicit no-blocking-scope-question evidence, and allowed Prompt Reflection statuses before prompt-to-PRD alignment can open. |
| `worker::workflow::agents` | Boundary for spawning specialized agents, preserving run handles as receipt evidence, advancing registered required-agent rows in canonical order for PRD/TDD generation, proximity, reflection, tournament, evolution, and promotion; decomposition extraction, impact, planning, proximity, validation, ranking, evolution, and publishing; active-effort development receipts plus optional repair-writer rows; executing structured development proof requests through injected runners; and mapping unavailable execution into blocked required-agent rows. |
| `worker::workflow::gates` | Human-gate request creation, request id correlation, stale-answer rejection, and answer-to-command mapping. |
| `worker::workflow::agents::WorkflowRunner` | Current worker-session execution boundary that advances runnable workflow work after gate answers, starts the next legal development effort, runs receipt-producing phase agents for PRD/TDD/decomposition/development roles in canonical subworkflow order, advances structured ready artifacts, records structured development validation command evidence, captures Git checkpoint evidence when ready, applies structured reviewer stage/commit requests, generates ready handover artifacts, and maps unavailable execution into blocked required-agent rows. |
| `worker::workflow_command` | Runtime adapter that translates daemon `WorkflowCommand` protobuf envelopes into existing worker-local orchestrator methods without giving the daemon transition authority. |
| `worker::prompt_runner` | Worker-session prompt execution boundary that composes provider/runtime selection, prompt-agent tooling, final response extraction, and durable `PROMPT.md` writing behind `RuntimePromptRunner`. |

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

Spawned and running rows use the receipt evidence field for the concrete agent
run handle. Once the agent submits a durable receipt, that receipt path replaces
the run handle as the evidence value. Acceptance records the reviewed receipt
path rather than the transient spawn handle.

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

## Workflow Commands

Typed workflow commands use a separate CLI-to-daemon-to-worker route from human
gate answers. The CLI exposes `doric workflow` subcommands, the daemon accepts
`ApplyWorkflowCommand`, verifies that the worker is known and non-terminal, and
forwards the command over the worker-session stream as
`DaemonFrame::WorkflowCommand`.

The worker applies those commands through existing orchestrator entrypoints:

- `PublishPhaseArtifactCommand` maps to phase artifact publication.
- `SubmitReceiptCommand`, `AcceptReceiptCommand`, and `RejectReceiptCommand`
  map to receipt review transitions.
- `RecordEffortOrderCommand` maps to decomposition effort-order recording.
- `StartEffortCommand` maps to development effort start.
- `RecordCommandValidationCommand` maps to command-backed red/green/review
  validation artifact generation and recording.
- `RecordGitCheckpointCommand` maps to Git-backed worktree/staging checkpoint
  evidence generation and recording.
- `CompleteEffortWithGitCommitCommand` maps to scoped staged-path verification,
  Git commit creation, `HEAD` resolution, and effort completion recording.

The daemon must not decide whether the command is legal for the current root
step, whether referenced artifacts exist, or whether required receipts are
sufficient. Those checks remain worker-owned reducer and artifact-validation
behavior. The current CLI exposes only this narrow routing surface; it does not
compute legal workflow transitions.

## Event And Observability Model

Workflow events are canonical inside `state/workflow/events.jsonl`. The worker
also projects operator-relevant events into the existing daemon stream as
existing `WorkerEvent` records. The daemon stream can remain name/message based
for the first implementation.

The first slice keeps public lifecycle contracts narrow by mapping workflow
events into existing `WorkerEvent` records. Dedicated lifecycle names should be
added only when a CLI, daemon, or wire-protocol consumer needs them.

Current and target event names:

| Workflow event | Daemon stream name | Notes |
| -------------- | ------------------ | ----- |
| `WorkflowStarted` | `workflow_started` | Message includes workflow id, parent id, and current step. |
| `StepEntered` | `workflow_step_entered` | Message includes workflow id and local step. |
| `GateRequested` | `waiting_for_input` | Includes request id and gate prompt. |
| `GateApproved` | `workflow_gate_approved` | Message includes the legal edge rendered for humans. |
| `GateRejected` | `workflow_gate_rejected` | Message includes the legal edge and safe reason. |
| `ArtifactPublished` | `workflow_artifact_published` | Message includes artifact path relative to `<run>`. |
| `ReceiptStatusChanged` | `current_activity` | First-slice mapping for registered/spawned/running/submitted/rejected/accepted receipt states. |
| `ReceiptAccepted` | `current_activity` | First-slice mapping; a dedicated `workflow_agent_receipt_accepted` event can be added when clients need it. |
| `RequiredAgentBlocked` | `workflow_agent_required_blocked` | Message includes role and safe blocker reason. |
| `EffortStarted` | `current_activity` | Message includes the selected effort path. |
| `ValidationRecorded` | `current_activity` | Message includes effort, validation kind, and validation record path. |
| `CheckpointRecorded` | `current_activity` | Message includes effort, worktree checkpoint path, and staging checkpoint path. |
| `LockAcquired` | `current_activity` | Message includes owner and path. |
| `LockReleased` | `current_activity` | Message includes owner and path. |
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
   - Require champion-confident zero-gap accepted `PRD.md` and accepted
     receipts before the root advances to `tdd`.

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
   - Require command-backed red/green evidence where applicable, validation
     records, accepted receipts, review, worktree/staging checkpoint evidence,
     active-lock cleanup, scoped staged paths, and one scoped commit checkpoint
     per effort.

7. Add handover and complete.
   - Generate and publish a final handover report, artifact index, validation
     summary, residual risks, and close/archive decision.
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
| Prompt completion marks worker succeeded before PRD | Keep the current runtime behavior: publish durable `PROMPT.md`, advance only to the prompt-to-PRD gate, and keep the worker running or waiting for input until the root workflow reaches a terminal state. |
| Stale human answer arrives after gate changed | Reject by request id and legal edge; emit a safe rejection event. |
| Required sub-agent spawn support is unavailable | Register the required row as `blocked`, regenerate `STATE.md`, and block the workflow. |
| Required receipt is rejected | Keep the workflow active; do not advance until a replacement receipt is accepted or the run is explicitly stopped. |
| `STATE.md` is manually edited | Ignore it for transition authority; regenerate from canonical state. |
| `STATE.md` is missing or stale | Regenerate before the next action and emit `ProjectionRegenerated`. |
| Artifact path points outside `artifacts/` | Reject the artifact path and block until repaired. |
| Referenced workflow artifact is missing | Reject the transition or recovery attempt until durable evidence exists under `artifacts/`. |
| Tool output is needed as durable evidence | Link to `tool-output/` or publish a deliberate artifact; do not blur tool trace ownership with run artifacts. |
| Daemon restart loses in-memory registry state | Worker state remains canonical; daemon may need a reattach/replay protocol before it can show current workflow state. |
| Worker restarts after side effect but before event append | Repair only from unambiguous durable evidence; otherwise block for review. |
| Development effort tries to skip order | Reducer rejects because `Next effort index` selects the only legal effort. |
| Active locks remain after effort validation | Do not complete the effort; require release or explicit failure handling. |
| Worktree or staging checkpoint evidence is missing | Do not complete the effort; record durable checkpoint artifacts first. |
| Git checkpoint command fails | Reject checkpoint recording before writing checkpoint state; keep the effort in progress. |
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
- Development effort completion requires durable worktree and staging
  checkpoint evidence in addition to validation, receipts, locks, and commit
  checkpoint data.
- Git-backed checkpoint recording writes worktree and staging evidence before
  appending checkpoint state and rejects failed Git commands without recording
  the checkpoint.
- Handover cannot reach `complete` until all efforts, validation, artifact
  index, final report, and close/archive decisions are recorded.
