# Technical Design Document

## Summary

Build the first Doric daemon/worker lifecycle slice as an additive raw-CLI path in the current Rust/Nx workspace.

The design adds three workspace packages:

- `lifecycle`: shared gRPC/protobuf contracts, domain DTOs, and redaction helpers.
- `daemon`: daemon library plus `doric-daemon` binary, responsible for worker liveness, in-memory registry, event routing, and human-input routing.
- `worker`: worker library plus `doric-worker` binary, responsible for host-root repo preparation, prompt/requirements-agent execution, agent/tool composition, transcript/internal state, and event emission.

`packages/cli` remains the `doric` binary and composition root. It adds raw lifecycle commands that talk to the daemon over gRPC. Existing chat behavior remains untouched unless decomposition explicitly chooses to retire it in a later feature; v1 lifecycle commands do not depend on the TUI.

Tool ownership is resolved for this slice: `packages/tools` remains the stable owner for existing chat tools and the shared lifecycle tool implementation. The new `worker` package depends on `tools` and registers the same built-ins with the cloned repo root as their workspace root/default plus a worker trace directory; this is not confinement. Moving chat tools into `worker` or retiring `packages/tools` is out of scope for this TDD because preserving current chat is a hard compatibility constraint.

Worker provider/runtime composition is resolved for this slice: `worker` composes the prompt-agent provider locally from `config` and `llms` APIs. No lower package extraction is required for v1 because the existing chat composition is `cli`-private, worker is the first concrete second consumer, and duplicating a small boundary keeps chat stable while proving the lifecycle path.

Resolved v1 PRD decisions:

| Question | v1 decision |
| --- | --- |
| Registry persistence/restart behavior | The daemon registry is in-memory only. Workers are daemon-owned child processes. A daemon restart starts with an empty registry, must not report stale workers as live, and unknown prior worker ids return a clear untracked/unknown message. |
| Human-in-loop answer command syntax | Use `doric answer <worker-id> <request-id> --text "<answer>"`. The command is daemon-mediated and never talks directly to a worker. |
| Host worker root convention | The daemon uses `--worker-root <path>` when supplied, otherwise `config::config_dir()/workers`. Each job lives under `<worker-root>/<worker-id>/` with `repo/`, `state/`, `artifacts/`, and `tool-output/` children. |
| Feature/debug behavior | `feature` and `debug` share the same prompt/requirements workflow in v1. The requested mode is preserved in ids, status, events, and output. |
| Stream replay behavior | `doric worker <id>` replays the daemon's retained in-memory event history from sequence `0`, then follows live events. Events carry monotonic sequence numbers. If retained history was truncated, the stream starts with a history-truncated event. |

No blocking architecture fact was found. The implementation will change workspace membership and package boundaries, so `docs/architecture-and-packages.md` must be updated during implementation, but this authoring task is scoped to `TDD.md` only.

## Current architecture context

Current verified facts:

| Fact | Evidence | Design impact |
| --- | --- | --- |
| The workspace is Cargo resolver 2 with members `cli`, `agent`, `commands`, `llms`, `config`, `tools`, and `tui`. | `Cargo.toml` | Add `lifecycle`, `daemon`, and `worker` as new members. Keep `tools` as a workspace member and shared dependency for chat and worker lifecycle execution. |
| Nx uses `packages/*` workspaces and `@monodon/rust` targets. | `package.json`, `packages/*/project.json`, `.agents/skills/coding-conventions/references/monodon-rust.md` | New packages need `Cargo.toml` and `project.json` with `build`, `test`, `lint`, and binary `run` targets where applicable. |
| `cli` owns the user-facing `doric` binary and currently exposes only `config` as a top-level subcommand; bare invocation starts chat. | `packages/cli/src/main/cli_types.rs`, `packages/cli/src/main/entry.rs` | Lifecycle commands are added to `cli_types.rs` and routed in `entry.rs`; dispatch/list/stream/answer use daemon clients. |
| `agent` already owns agent runtime, event stream, store, tool trait, queueing, cancellation, provider streaming, and tool execution contracts. | `packages/agent/src/lib.rs`, `packages/agent/src/agent.rs`, `packages/agent/src/event.rs`, `packages/agent/src/store.rs`, `packages/agent/src/tool.rs` | Worker should reuse `agent` rather than invent a second agent loop. |
| `tools` currently owns built-in host tools and registers them against a workspace root, with optional terminal trace storage. | `packages/tools/src/lib.rs`; `packages/cli/src/chat/runner.rs` | `tools` remains the canonical owner for built-in host tools. Worker reuses it by passing `<worker-root>/<id>/repo` and `<worker-root>/<id>/tool-output`; chat keeps using the current `tools` crate path. |
| `config` owns `config_dir()`, config/model-cache IO, provider credentials, model slots, and API-key masking. | `packages/config/src/lib.rs`, `packages/config/src/schema.rs`, `packages/config/src/persistence.rs` | Worker root defaults can use `config::config_dir()`. Provider/model runtime selection should reuse existing config concepts instead of new secret storage. |
| Chat provider/runtime composition is not reusable by `worker` because it is currently local to `cli`. | `packages/cli/src/chat/runtime_selection.rs`, `packages/cli/src/chat/provider.rs`, `packages/cli/src/chat/auth.rs`, `packages/cli/src/chat/runner.rs` | Worker must not depend on `cli`; v1 adds worker-local composition that uses public `config`, `llms`, and `agent` APIs and temporarily duplicates only the small chat-local selection/provider/auth adapter layer. |
| Existing session persistence is chat/TUI-owned JSONL under `cli`. | `packages/cli/src/chat/session.rs`, `packages/cli/src/chat/session/store.rs` | Worker lifecycle event history should not reuse chat session storage in v1. Worker internal state belongs under the worker root. |

Target package direction after this slice:

```text
cli
 |-- lifecycle ----> generated gRPC contracts and shared DTOs
 |-- daemon -------> lifecycle, config
 |-- tools --------> existing chat built-ins
 |-- config
 |-- existing chat path remains as-is until separately retired

daemon
 |-- lifecycle
 |-- config

worker
 |-- lifecycle
 |-- agent -----> llms
 |-- config
 |-- llms
 |-- tools -----> agent
 |-- local provider/runtime composition from config + llms

agent
 |-- llms

tools
 |-- agent
```

`lifecycle` must not depend on `cli`, `daemon`, `worker`, or `tools`. `daemon` must not depend on worker internals beyond the worker process contract and lifecycle protocol. `worker` may depend on `tools`; `tools` must not depend on `worker`, `daemon`, or `cli`.

## Technical persona debate

| Disputed point | Database Administrator | Frontend Lead | SecOps Specialist | Decision | Tradeoff accepted |
| --- | --- | --- | --- | --- | --- |
| Registry persistence | Pushed for durable worker rows and replayable audit tables. | Favored fast visible status and predictable CLI output. | Warned that half-restored child processes after restart can be misleading. | Use in-memory registry and explicit restart loss in v1. | No restart recovery, but no false liveness claims or migration work in the first slice. |
| Event retention | Wanted append-only event storage from day one. | Wanted late attachment to show useful recent history. | Wanted bounded memory and redacted output. | Keep bounded in-memory event ring with sequence numbers and replay-before-live. | Replay is useful within one daemon lifetime; durable audit waits for a later persistence design. |
| Human input routing | Wanted a normalized pending-input table. | Wanted a simple command users can type from docs and stream output. | Required unambiguous correlation and no direct worker bypass. | Use pending-input records keyed by `(worker_id, request_id)` and `doric answer <worker-id> <request-id> --text`. | The syntax is less compact than an interactive prompt, but it is testable and daemon-mediated. |
| Worker process topology | No strong database concern. | Wanted minimal startup complexity. | Rejected direct CLI-to-worker access and unauthenticated worker spoofing. | Worker initiates a bidirectional gRPC session to the daemon using a one-time token. Daemon sends commands back over that stream. | Avoids worker port discovery while preserving daemon-to-worker gRPC messages. |
| Worker provider/runtime composition | Wanted one provider/runtime composition source eventually. | Wanted no chat regression or large migration before lifecycle proves itself. | Required provider secrets to stay in `config` and not be copied into daemon state. | Compose providers locally in `worker` from `config` + `llms` and keep chat-local composition untouched in v1. | Small temporary duplication is accepted. A lower package extraction waits until worker and chat prove the shared API shape. |
| Tool ownership | Wanted one canonical source of tool behavior. | Wanted to avoid regressing current chat while lifecycle lands. | Wanted tool registration to bias operations toward the clone and warned that root defaults are not containment. | `packages/tools` remains the canonical owner for built-in host tools; worker depends on it and supplies the cloned repo as the workspace root/default plus a worker trace directory. | This keeps the first slice decomposable and preserves chat. V1 does not add tool confinement; Docker or a future explicit scoped-tools mode can revisit containment with its own TDD. |
| Host execution | Wanted a clear root schema. | Wanted dispatch to fail before a confusing worker crash. | Warned that host execution is not sandboxing. | Use explicit worker root validation before dispatch and register tools with `<worker-root>/<id>/repo` as their default workspace root. | V1 is operationally useful but must not imply isolation. |

## Proposed design

### Package additions

`packages/lifecycle`

- Library package.
- Owns `proto/doric/lifecycle/v1.proto` and generated Rust modules.
- Owns small hand-written domain wrappers where they reduce call-site burden: `WorkerId`, `RequestId`, `RepoIdentity`, and redaction helpers.
- Uses a `build.rs` codegen step to compile `proto/doric/lifecycle/v1.proto` into a checked-at-build Rust module exposed as `lifecycle::proto::doric::lifecycle::v1`.
- Codegen dependencies are isolated to this package: runtime dependencies are `tonic`, `tonic-prost`, `prost`, `prost-types`, `serde`, and `serde_json` as needed for gRPC services, prost-backed codecs, contracts, DTO wrappers, and fixtures; build dependencies are `tonic-prost-build` and `protoc-bin-vendored`.
- `build.rs` should prefer an explicit `PROTOC` if one is set, otherwise use `protoc-bin-vendored` so Windows developers and CI do not need a system `protoc` install.
- Has no runtime process ownership.

`packages/daemon`

- Library plus binary package.
- Binary: `doric-daemon`.
- Library modules:
  - `server`: binds local gRPC services.
  - `registry`: in-memory worker records, event rings, pending-input records, subscriber channels.
  - `process`: worker binary resolution, spawn, child liveness, terminal-state detection.
  - `service`: CLI-facing lifecycle service implementation.
  - `worker_session`: worker-initiated bidirectional stream handling and daemon-to-worker command delivery.
  - `root`: worker-root resolution and validation.
- Depends on `lifecycle`, `config`, `tokio`, `tonic` runtime APIs, and standard process/fs APIs. It consumes generated contracts from `lifecycle` and does not run its own proto codegen.
- Does not depend on `agent`, `llms`, worker internals, TUI, or chat session code.

`packages/worker`

- Library plus binary package.
- Binary: `doric-worker`.
- Library modules:
  - `runtime`: starts a worker session, connects to daemon, receives `StartJob`, emits status/events, waits for daemon commands.
  - `repo`: validates and clones the requested repo into `<worker-root>/<id>/repo`.
  - `provider`: resolves the configured coding model, builds provider implementations, and owns worker-local OpenAI OAuth persistence.
  - `agents/prompt`: prompt/requirements agent only.
  - `tooling`: calls `tools::built_in_tools_with_trace_dir(repo_dir, tool_output_dir)` and owns any worker-specific filtering or trace-dir policy.
  - `state`: worker-local transcript/internal state under `<worker-root>/<id>/state`.
  - `event`: maps internal events to `lifecycle::WorkerEvent` values.
- Depends on `lifecycle`, `agent`, `llms`, `config`, `tools`, `tokio`, and `tonic` runtime APIs as needed for the worker session client. It consumes generated contracts from `lifecycle` and does not run its own proto codegen.
- Does not depend on `daemon` or `cli`.

### CLI command surface

Add top-level clap commands in `packages/cli/src/main/cli_types.rs`:

```text
doric daemon [--addr <host:port>] [--worker-root <path>]
doric feature --prompt <prompt> --repo <repo> [--addr <host:port>]
doric debug --prompt <prompt> --repo <repo> [--addr <host:port>]
doric list [--addr <host:port>]
doric worker <id> [--addr <host:port>]
doric answer <worker-id> <request-id> --text <answer> [--addr <host:port>]
```

Defaults:

- `--addr` defaults to `127.0.0.1:47821`.
- `doric daemon` starts the daemon in the foreground. This command may call `daemon::run` in-process; `doric-daemon` remains available as the package binary for supervisors and tests.
- `feature` and `debug` both require non-empty `--prompt` and `--repo`.
- Lifecycle commands fail nonzero when the daemon is unavailable and must not fall back to direct worker access.
- Existing `config` command remains.
- Bare `doric` chat behavior remains unless a separate accepted effort removes it.

Entrypoint rewrite:

- `packages/cli/src/main/entry.rs` currently creates `LlmDebugLogger` before dispatch, so even non-chat commands can fail on provider debug-log startup.
- Lifecycle command routing must parse `Cli` and dispatch non-chat commands before creating chat/provider debug infrastructure.
- `LlmDebugLogger::create_for_current_exe()` should move into the bare-chat branch immediately before `chat::run(debug_logger).await`.
- Provider/debug-log errors remain chat-scoped. `config`, `daemon`, `feature`, `debug`, `list`, `worker`, and `answer` must use their own command/service error mapping and must not require provider debug logging to start.

### Daemon lifecycle flow

Dispatch:

1. CLI sends `DispatchRequest` to the daemon.
2. Daemon validates non-empty prompt/repo and worker root accessibility.
3. Daemon creates a worker id, redacts repo identity, allocates a one-time worker token, creates `<worker-root>/<id>/`, and inserts `WorkerRecord { status: accepted }`.
4. Daemon spawns `doric-worker` with daemon address, worker id, token, and worker root. Job payload is sent over gRPC after the worker attaches, not trusted from process args.
5. Daemon returns `DispatchResponse` with id, mode, redacted repo identity, and initial status.
6. Worker attaches to daemon through a bidirectional gRPC stream and sends `WorkerHello`.
7. Daemon validates token and sends `StartJob`.
8. Worker prepares repo, runs prompt agent, emits lifecycle events, and sends terminal status.

List:

1. CLI sends `ListWorkersRequest`.
2. Daemon returns summaries for all registry records retained in memory, including recently terminal records.
3. Empty registry returns a successful empty state.

Stream:

1. CLI sends `StreamWorkerRequest { worker_id, from_sequence: 0 }`.
2. Daemon validates known id.
3. Daemon replays retained events in sequence, then subscribes the caller to live events.
4. If history starts after requested sequence due to retention, daemon sends `history_truncated` before available events.
5. Unknown worker id returns a nonzero CLI error.

Human input:

1. Worker emits `InputRequested { request_id, prompt, choices? }`.
2. Daemon marks status `waiting_for_input`, records pending input by worker/request id, stores the event, and fans it out to stream/list subscribers.
3. CLI sends `AnswerInputRequest { worker_id, request_id, text }`.
4. Daemon validates pending input, sends `AnswerProvided` over the worker command stream, clears the pending record, updates status, stores a continuation event, and returns success.
5. Worker resumes only after receiving the daemon command.

Restart:

- The registry is not restored.
- Previous worker ids are unknown after restart.
- Worker child processes should be terminated when the daemon exits normally. If an orphan worker reconnects to a restarted daemon, token validation fails and the worker exits.

### Worker prompt-agent execution

V1 worker scope is intentionally narrow:

- The only workflow agent is `agents/prompt`.
- It creates or updates a prompt/requirements artifact under `<worker-root>/<id>/artifacts/PROMPT.md`.
- It may request human input when required facts are missing.
- It must not emit events that imply PRD, technical design, decomposition, implementation, tests, or handover ran.
- It uses existing provider/runtime selection through `config` and `llms` rather than adding a new provider model.
- It uses the existing `agent` runtime and event stream instead of a bespoke model loop.

The prompt agent should be built behind a small runner trait:

```rust
pub trait PromptAgentRunner: Send + Sync {
    fn run<'a>(&'a self, job: PromptJob, sink: EventSink) -> PromptAgentFuture<'a>;
}
```

Production runner uses `agent::Agent` and worker tools. Tests use fake runners that emit deterministic lifecycle events and input requests.

### Worker provider/runtime composition

Decision: `worker` performs local provider/runtime composition from public `config`, `llms`, and `agent` APIs. Do not extract a new lower package in v1. A package such as `runtime` or `provider-runtime` would be premature because the current chat composition is still a `cli` implementation detail and the worker lifecycle is the first real pressure for a shared API. The implementation should keep the worker module small, behavior-tested, and explicitly marked as temporary duplication that can be extracted after chat and worker both use the shape.

Worker-local `provider` module responsibilities:

1. Resolve a `WorkerRuntimeSelection` from persisted config:
   - `config::read_config_or_default()`
   - `config::read_model_cache_or_default()`
   - `DoricConfig::model_for_task(TaskModelSlot::Coding)`
   - `DoricConfig::provider_for_model(model_key)`
   - `ProviderConfig::auth_mode()`
   - `ProviderConfig::api_key()`
   - `ModelCache::model(provider_name, model_id)` for optional cached `context_window_tokens`
   - `ReasoningLevel::non_none()` and `ReasoningLevel::as_str()` for agent reasoning settings
2. Build concrete providers from `llms`:
   - `OPENROUTER_PROVIDER_ID` -> `OpenRouterProvider::with_debug_logger(api_key, debug_logger)`
   - `OPENAI_PROVIDER_ID` with API-key auth -> `OpenAiProvider::with_api_key_and_debug_logger(api_key, debug_logger)`
   - `OPENAI_PROVIDER_ID` with OAuth auth -> `OpenAiProvider::with_debug_logger(Arc<WorkerOpenAiAuthStore>, debug_logger)`
   - unsupported provider types fail the worker before prompt-agent execution and emit a redacted lifecycle failure event.
3. Implement `WorkerOpenAiAuthStore` locally in `worker` using:
   - `OpenAiAuthStore`, `OpenAiAuthRecord`, and `ProviderError` from `llms`
   - `ProviderConfig::oauth_config()` to load persisted ChatGPT OAuth credentials
   - `config::read_config_or_default()`, `DoricConfig::set_provider_oauth(provider_name, auth)`, and `config::write_config(&config)` to persist refreshed credentials
   - local conversion helpers between `config::ChatGptAuthConfig` and `llms::OpenAiAuthRecord`
4. Build the prompt agent through public `agent` APIs:
   - `AgentConfig { model, include_reasoning, reasoning_effort, context_policy, system_prompt, ..Default::default() }`
   - `ContextPolicy { model_context_window_tokens, reasoning_reserve_tokens, max_summary_passes_per_request, ..Default::default() }`
   - `Agent::with_config_and_store(provider, config, Store::default()).with_tools(worker_tools)`
   - `Agent::run_stream` or `Agent::run_stream_with_prompt_event_id` so worker can map `AgentEvent`s to lifecycle events.

Debug logging:

- Worker provider composition may use `LlmDebugLogger::disabled()` by default to avoid coupling lifecycle worker startup to executable-directory debug-log creation.
- If implementation adds an explicit worker debug-log flag later, create the logger inside `worker`, not in `cli`, and keep failures worker-scoped.

Temporary duplicated chat-local code:

- Runtime selection shape and resolution equivalent to `packages/cli/src/chat/runtime_selection.rs`.
- Provider enum/wrapper and factory dispatch equivalent to `packages/cli/src/chat/provider.rs`.
- OpenAI auth-store conversion and config persistence equivalent to `packages/cli/src/chat/auth.rs`.
- Agent reasoning/context-policy helpers equivalent to `packages/cli/src/chat/runner.rs`.

Do not duplicate:

- `packages/tools` implementations.
- Chat session persistence.
- TUI command/runtime behavior.
- CLI `ChatBootstrap` or `ChatModel`.
- Model-cache refresh behavior from `packages/cli/src/chat/model_cache.rs`; worker reads the existing cache and falls back to provider-owned context-window defaults through `LlmProvider::context_window_tokens`.

Extraction trigger after v1: if both chat and worker need the same provider/runtime code after lifecycle is green, create a lower library package named `runtime` with no `cli`, `daemon`, `worker`, `tui`, or `tools` dependency. Its migration scope would be only provider/runtime selection, provider factory dispatch, OpenAI config-backed auth store, and agent config/context-policy helpers. It would depend on `config`, `llms`, and `agent`, and both `cli` chat and `worker` would consume it. That extraction is explicitly deferred from this slice.

### Root and repo handling

Root resolution:

1. `doric daemon --worker-root <path>` wins.
2. Otherwise use `config::config_dir()?.join("workers")`.
3. Daemon creates the root when possible and verifies it is a directory and writable before accepting dispatch.

Per-worker layout:

```text
<worker-root>/
  <worker-id>/
    repo/
    state/
    artifacts/
      PROMPT.md
    tool-output/
```

Repo preparation:

- V1 accepts remote clone URLs.
- Worker clones into `repo/` before starting the prompt agent.
- Clone implementation uses the external `git` CLI through an injected command runner, not `git2` or another Rust Git library in this slice. This matches local developer expectations, avoids adding libgit2/OpenSSL build complexity on Windows, and keeps authentication behavior aligned with the user's installed Git credential helpers.
- Production command shape: `git clone -- <repo> <repo_dir>` from the worker root, with timeout/cancellation owned by worker runtime. The implementation must not shell-concatenate the repo URL.
- Failure mapping:
  - missing `git` executable -> `RepoError::GitUnavailable` and worker status `failed`;
  - nonzero clone exit -> `RepoError::CloneFailed` with a redacted, bounded stderr excerpt;
  - timeout or cancellation -> `RepoError::CloneTimedOut` or `RepoError::Cancelled`;
  - destination already exists or escapes worker root -> validation error before invoking Git.
- Tests use a fake command runner or fake `RepoPreparer`; automated tests must not require network access or a real remote repository.
- Repo identity rendered to users is redacted and may omit URL credentials.
- Clone failure sets worker status to `failed`, emits a failure event, and is visible in `list` and `worker <id>`.

### Tool ownership graph

The final package graph for this slice keeps `packages/tools` as the stable owner for built-in host tools:

```text
cli chat path -> tools -> agent::Tool
worker path   -> tools -> agent::Tool
```

Implementation notes:

1. Do not move tool modules into `packages/worker` in this slice.
2. Add `tools = { path = "../tools" }` to `packages/worker/Cargo.toml`.
3. Worker registers tools through `tools::built_in_tools_with_trace_dir(repo_dir, tool_output_dir)`.
4. In v1, `repo_dir` is the shared tools workspace root/default, not a sandbox boundary. Current `packages/tools` intentionally allows absolute paths and `..` traversal after lexical normalization.
5. The worker-owned `tooling` module may validate worker paths, provide worker-specific trace-dir policy, and expose a narrow helper to prompt-agent code, but it must not duplicate tool implementations or claim confinement that `packages/tools` does not provide.
6. Existing `packages/cli/src/chat/runner.rs` keeps using `tools::built_in_tools_with_trace_dir(workspace_root, trace_dir)`.

This satisfies the preferred constraint from the gap report: existing chat remains stable, `packages/tools` remains the shared lifecycle tool owner, and worker depends on `tools`. A future design may retire chat, run workers inside Docker, or add an explicit scoped-tools mode with containment tests, but those are explicitly out of this slice.

## Data model

### Shared lifecycle DTOs

| Type | Fields | Notes |
| --- | --- | --- |
| `JobMode` | `feature`, `debug` | Mode label only in v1; execution path is shared. |
| `WorkerStatus` | `accepted`, `starting`, `running`, `waiting_for_input`, `succeeded`, `failed`, `stopped`, `unavailable` | Covers PRD-required liveness and terminal states. |
| `DispatchRequest` | `mode`, `prompt`, `repo` | CLI-to-daemon only. Prompt and raw repo are not stored in daemon-visible output without redaction. |
| `DispatchResponse` | `worker_id`, `mode`, `repo_identity`, `status` | Printed by dispatch commands. |
| `WorkerSummary` | `worker_id`, `mode`, `repo_identity`, `status`, `activity`, `created_at_ms`, `updated_at_ms`, `terminal_reason`, `waiting_request` | Used by `list`. |
| `WorkerEvent` | `worker_id`, `sequence`, `timestamp_ms`, `kind`, `status`, `activity`, `message`, `repo_identity`, `input_request`, `terminal_reason` | Used by stream replay and live tail. |
| `InputRequest` | `request_id`, `prompt`, `choices`, `allow_free_text` | Correlation handle for HITL. |
| `AnswerInputRequest` | `worker_id`, `request_id`, `text` | Daemon-mediated answer. |

### Daemon internal state

```rust
struct DaemonState {
    workers: HashMap<WorkerId, WorkerRecord>,
}

struct WorkerRecord {
    id: WorkerId,
    mode: JobMode,
    repo_identity: RepoIdentity,
    status: WorkerStatus,
    activity: String,
    created_at: SystemTime,
    updated_at: SystemTime,
    terminal_reason: Option<String>,
    child: Option<tokio::process::Child>,
    token: WorkerToken,
    events: EventRing,
    subscribers: broadcast::Sender<WorkerEvent>,
    command_sender: Option<mpsc::Sender<DaemonFrame>>,
    pending_inputs: HashMap<RequestId, PendingInput>,
}
```

`EventRing` is bounded by count and/or bytes. Initial target: retain at least the last 1,000 lifecycle events per worker, with truncation surfaced through a synthetic `history_truncated` event.

### Worker internal state

```rust
struct WorkerJob {
    id: WorkerId,
    mode: JobMode,
    prompt: String,
    raw_repo: String,
    redacted_repo: String,
    root: PathBuf,
    repo_dir: PathBuf,
    state_dir: PathBuf,
    artifact_dir: PathBuf,
    tool_output_dir: PathBuf,
}
```

Worker state is local to the worker root. The daemon registry stores summaries and stream events only, not full transcript internals.

## API or interface contracts

### gRPC services

`packages/lifecycle/proto/doric/lifecycle/v1.proto` should define two services.

CLI-facing daemon service:

```proto
service LifecycleService {
  rpc Dispatch(DispatchRequest) returns (DispatchResponse);
  rpc ListWorkers(ListWorkersRequest) returns (ListWorkersResponse);
  rpc StreamWorker(StreamWorkerRequest) returns (stream WorkerEvent);
  rpc AnswerInput(AnswerInputRequest) returns (AnswerInputResponse);
}
```

Worker-facing session service:

```proto
service WorkerSessionService {
  rpc Attach(stream WorkerFrame) returns (stream DaemonFrame);
}
```

Frame shape:

```proto
message WorkerFrame {
  oneof frame {
    WorkerHello hello = 1;
    WorkerStatusUpdate status = 2;
    WorkerEvent event = 3;
    InputRequest input_request = 4;
    WorkerTerminal terminal = 5;
  }
}

message DaemonFrame {
  oneof frame {
    StartJob start_job = 1;
    AnswerInputCommand answer = 2;
    ShutdownWorker shutdown = 3;
  }
}
```

`WorkerHello` includes `worker_id` and the one-time token. The token is never printed in CLI output or worker events.

Generated Rust contract:

- `packages/lifecycle/src/lib.rs` exposes generated gRPC code under `pub mod proto` and hand-written wrappers/redaction helpers outside the generated namespace.
- Generated files are produced during `cargo build` by `packages/lifecycle/build.rs`; do not commit generated Rust into `src/` unless implementation discovers a hard CI constraint that prevents build-time generation.
- `build.rs` uses `tonic-prost-build` to compile prost-backed tonic services, watches `proto/doric/lifecycle/v1.proto`, and fails fast with an actionable message when codegen fails.
- Generated service code depends on `tonic_prost::ProstCodec`, so `tonic-prost` is a required runtime dependency for crates that compile the generated service module.
- Windows handling is part of the contract: use `protoc-bin-vendored` as the fallback `protoc` provider and keep Cargo/Nx validation commands capable of running on a clean Windows checkout without manual Protocol Buffers installation.

### CLI output contracts

Dispatch success:

```text
Worker accepted
id: <worker-id>
mode: feature|debug
repo: <redacted-repo>
status: accepted
```

List empty:

```text
No workers.
```

List row content must include id, mode, repo identity, status, current activity or terminal reason, and updated time/order signal. Exact table styling can follow existing `main/output.rs` conventions.

Stream output:

- One readable line per lifecycle event.
- Waiting input events must include `request_id` and the answer command shape.
- Failure events must include concise reason.
- Event text must not include credentials from repo URLs or environment secrets.

Answer success:

```text
Answer accepted
worker: <worker-id>
request: <request-id>
```

### Rust interfaces

Daemon registry:

```rust
pub trait Registry {
    fn insert_accepted(&self, job: AcceptedJob) -> Result<WorkerSummary, RegistryError>;
    fn update_status(&self, id: &WorkerId, status: StatusUpdate) -> Result<(), RegistryError>;
    fn append_event(&self, id: &WorkerId, event: WorkerEvent) -> Result<(), RegistryError>;
    fn list(&self) -> Vec<WorkerSummary>;
    fn subscribe(&self, id: &WorkerId, from: u64) -> Result<EventSubscription, RegistryError>;
    async fn answer(&self, id: &WorkerId, request: &RequestId, text: String) -> Result<(), RegistryError>;
}
```

Worker process spawner:

```rust
pub trait WorkerSpawner {
    async fn spawn(&self, launch: WorkerLaunch) -> Result<SpawnedWorker, SpawnError>;
}
```

Repo preparation:

```rust
pub trait RepoPreparer {
    async fn prepare(&self, job: &WorkerJob) -> Result<PreparedRepo, RepoError>;
}

pub trait GitCommandRunner {
    async fn clone(&self, repo: &str, destination: &Path) -> Result<GitOutput, GitError>;
}
```

These traits are test seams, not framework abstractions. Production implementations should stay package-private unless tests require visibility. The production `RepoPreparer` composes root validation plus `GitCommandRunner`; unit tests can fake either layer.

## Dependency Hops

| Hop | Decision or component | Assumption | Dependency/evidence | Failure mode | Fallback or mitigation |
| --- | --- | --- | --- | --- | --- |
| CLI command surface | Add `daemon`, `feature`, `debug`, `list`, `worker`, and `answer` to `cli`. | `cli` is still the right user-facing composition root. | `packages/cli/src/main/cli_types.rs` has only `Config`; `entry.rs` already centralizes routing and exit codes. | Commands scatter into separate binaries and users lose one raw CLI surface. | Keep `doric-daemon` for supervisors, but expose user workflow through `doric`. |
| CLI entrypoint dispatch | Move `LlmDebugLogger` creation into the bare-chat branch before `chat::run`. | Lifecycle commands should not depend on chat/provider debug-log startup. | `packages/cli/src/main/entry.rs` currently creates `LlmDebugLogger` before matching `cli.command`; lifecycle commands are non-chat commands. | `doric list` or `doric feature` can fail before contacting the daemon because provider debug logging cannot initialize. | Parse and dispatch first; create debug logger only for `None` command/chat. Add CLI tests proving lifecycle/config command paths do not touch debug-log setup. |
| gRPC/protobuf build | `lifecycle` owns proto files and build-time Rust codegen. | Cargo and Nx can build generated prost-backed tonic contracts on Windows without a manually installed `protoc`. | New package can isolate `build.rs`; current protobuf generation uses build-time `tonic-prost-build` plus `protoc-bin-vendored`, and generated services require runtime `tonic`, `tonic-prost`, `prost`, and `prost-types` as needed. | Developers or CI fail before tests because `protoc` is missing, generated modules are stale, or the generated `tonic_prost::ProstCodec` dependency is absent. | Use explicit `PROTOC` when supplied; otherwise use vendored protoc. Validate with `cargo build -p lifecycle`, `cargo test -p lifecycle`, `npx nx run lifecycle:build`, and `npx nx run lifecycle:test` on Windows. |
| Daemon registry | Use in-memory registry for v1. | Restart recovery is not required if behavior is explicit and stale workers are never reported live. | PRD AC-14 allows in-memory if restart behavior is explicit. | Users expect old ids to survive daemon restart. | Return unknown/untracked messages after restart; add durable registry later if required. |
| Worker process/runtime | Daemon spawns `doric-worker`; worker owns prompt execution and connects back over gRPC. | Worker-initiated bidirectional stream satisfies daemon-to-worker command routing without worker port discovery. | Prompt requires daemon liveness/routing only; daemon and worker packages already require Tokio for gRPC/runtime work and can isolate process spawning behind `WorkerSpawner`. | Worker never attaches or token mismatch leaves accepted worker stuck. | Add attach timeout; transition to failed with reason if attach deadline expires. |
| Event stream | Bounded in-memory event ring with replay then live tail. | V1 users need late attachment within daemon lifetime, not durable event audit. | PRD asks whether replay should exist; selected replay supports AC-7 and AC-11. | Long worker runs exceed retention and lose early context. | Emit `history_truncated`; later add worker or daemon durable event log. |
| Human input routing | `doric answer <worker-id> <request-id> --text` routes through daemon pending input map. | A request id is enough to prevent answer misrouting. | PRD AC-11 requires visible correlation and daemon-mediated answer. | Answer reaches wrong worker/request or hangs. | Validate `(worker_id, request_id)` is pending; reject stale/duplicate answers. |
| Root/repo handling | Host workers run under daemon worker root with per-worker `repo/`, `state/`, `artifacts/`, and `tool-output/`; shared tools are registered with `repo/` as their workspace root/default. | Host-root execution is acceptable before Docker because root layout and non-containment semantics are explicit. | Prompt and PRD allow host root; `config::config_dir()` already exists; `packages/tools/src/path.rs` resolves relative paths against the workspace root but preserves absolute paths and `..` traversal after lexical normalization; `write` and `terminal` manifests document that behavior as intentional. | Users or implementers treat repo registration as sandboxing, or clone/artifact paths escape the worker root. | Validate worker root, repo destination, state/artifact/tool-output directories before dispatch/runtime; register tools with repo default and trace dir; document that containment waits for Docker or a future scoped-tools mode. |
| Repo clone mechanism | Worker uses the external `git` CLI through an injected command runner. | The user's installed Git handles remote transports and credentials more reliably than adding a Rust Git library in v1. | Windows build risk is lower than `git2`/libgit2/OpenSSL; tests can fake command output. | `git` is missing, authentication fails, clone hangs, or stderr leaks credentials. | Map missing Git/nonzero/timeout/cancel into redacted `RepoError` variants; use fake `GitCommandRunner`/`RepoPreparer` in tests and no network in automated tests. |
| Tool ownership graph | Keep `packages/tools` as shared owner; worker depends on `tools`. | Preserving chat is more important than moving tool code in this slice. | `packages/cli/Cargo.toml` depends on `tools`; `packages/cli/src/chat/runner.rs` calls `tools::built_in_tools_with_trace_dir`; `packages/tools/src/lib.rs` owns built-ins. | Moving tools to worker breaks existing chat or creates circular/package churn; adding confinement inside shared tools changes chat semantics. | Add worker dependency on `tools`; register tools with worker repo/trace dirs as defaults; leave chat imports and current tool semantics untouched. Defer any tool migration or scoped-tools containment mode to a separate accepted design. |
| Worker provider/runtime composition | Worker composes its prompt-agent provider locally from `config`, `llms`, and `agent` instead of depending on `cli` or extracting a lower package now. | Public `config`, `llms`, and `agent` APIs are enough to reproduce the small chat-local runtime/provider/auth seam for prompt-agent execution. | `config::read_config_or_default`, `read_model_cache_or_default`, `DoricConfig::model_for_task`, `DoricConfig::provider_for_model`, `ProviderConfig::{auth_mode,api_key,oauth_config}`, `DoricConfig::set_provider_oauth`, `config::write_config`; `llms::{OpenRouterProvider,OpenAiProvider,LlmDebugLogger,LlmProvider,OpenAiAuthStore,OpenAiAuthRecord,OPENROUTER_PROVIDER_ID,OPENAI_PROVIDER_ID}`; `agent::{Agent,AgentConfig,ContextPolicy,Store}`. Chat equivalents are `packages/cli/src/chat/runtime_selection.rs`, `provider.rs`, `auth.rs`, and `runner.rs`, but those are `cli`-owned. | Worker cannot run prompt-agent provider calls without illegally depending on `cli`, or a premature shared package forces chat migration before lifecycle is proven. | Add `packages/worker/src/provider.rs` with focused tests for config selection, provider-type dispatch, OAuth load/save, unsupported providers, reasoning/context policy, and cache fallback. Defer a lower `runtime` package extraction until both chat and worker use the same shape after v1. |
| Redaction/secrets | Shared redaction helpers strip repo URL credentials and avoid env secret printing. | Repo URLs and provider errors are the main v1 visible secret vectors. | `config::mask_api_key` and `llms` redacted excerpts prove redaction is already a repo concern. | Secrets leak through dispatch/list/stream/failure text. | Centralize repo redaction in `lifecycle`; add tests for URL credentials and API-key-like tokens. |
| Prompt-agent-only scope | Worker runs only prompt/requirements agent and writes only `PROMPT.md` artifact. | Prompt-agent-only still proves lifecycle value without false SDLC claims. | PRD AC-10 and AC-15 require no implication of later phases. | Output implies PRD/TDD/decomposition/code ran. | Use explicit event names: `prompt_agent_started`, `prompt_artifact_written`, `prompt_agent_completed`. |
| Testability | Use traits for registry, spawner, repo preparation, prompt runner, and gRPC client wrappers. | Focused tests can prove behavior without live LLMs or network repos. | Coding conventions require behavior tests, package `tests/` directories, dependency injection at boundaries. | Integration tests become slow/flaky or require network. | Fake daemon/worker streams, fake repo preparer, fake prompt runner, temp roots under `dist/test`. |

## Technical alternatives and tournament

Serious alternatives existed for package boundaries, registry persistence, and worker transport. The selected path is intentionally small but not merely easy.

### Package boundary tournament

| Candidate | Rating | Result |
| --- | ---: | --- |
| A. Add `daemon` and `worker`, put gRPC types inside `daemon` | 1450 | Rejected. Smaller package count, but makes worker or CLI depend on daemon-owned implementation details. |
| B. Add `lifecycle`, `daemon`, and `worker` | 1600 | Selected. One extra library, but clean dependency direction and shared contracts for all three processes. |
| C. Put daemon and worker modules inside `cli` | 1300 | Rejected. Conflicts with prompt package boundaries and bloats the composition root. |

### Registry persistence tournament

| Candidate | Rating | Result |
| --- | ---: | --- |
| A. In-memory registry with explicit restart loss | 1600 | Selected. Satisfies PRD AC-14 with the least migration and consistency risk. |
| B. JSONL registry under config dir | 1450 | Rejected for v1. Useful later, but process liveness recovery is hard to make honest without more supervision semantics. |
| C. SQLite registry | 1375 | Rejected for v1. Correct for richer history/query needs, but premature for first lifecycle slice. |

### Worker transport tournament

| Candidate | Rating | Result |
| --- | ---: | --- |
| A. Worker exposes its own gRPC server on an ephemeral port | 1425 | Rejected. Harder port discovery and startup coordination. |
| B. Worker-initiated bidirectional gRPC stream to daemon | 1600 | Selected. Keeps all daemon-to-worker commands on gRPC and avoids worker port exposure. |
| C. Worker stdin/stdout JSON protocol | 1250 | Rejected. Easier spawn path but violates the gRPC architecture requirement. |

QUIC remains a later transport option. The first slice should use normal local gRPC over loopback because the product requirement is lifecycle correctness, not transport optimization.

### Provider/runtime composition tournament

| Candidate | Rating | Result |
| --- | ---: | --- |
| A. Worker depends on `cli` chat modules | 900 | Rejected. Violates package direction and turns the application composition root into a lower-level dependency. |
| B. Extract a new lower `runtime` package before worker implementation | 1425 | Rejected for v1. It may be the right follow-up, but it forces chat migration before worker proves the second-consumer shape. |
| C. Worker-local composition from `config` + `llms` + `agent` with explicit temporary duplication | 1600 | Selected. Keeps chat stable, avoids illegal dependency direction, and remains decomposable with focused worker tests. |

## Security and privacy

- Bind daemon gRPC to loopback by default: `127.0.0.1:47821`.
- Do not add remote auth/multi-user authorization in v1; local-only binding and explicit non-goals keep the blast radius honest.
- Use a one-time worker token for worker attach. Reject unknown worker ids, wrong tokens, duplicate attaches, and attaches after terminal status.
- Never print worker tokens.
- Redact credentials embedded in repo URLs before storing `repo_identity` in daemon summaries/events.
- Do not print provider API keys, OAuth tokens, or environment values in stream/list output.
- Worker provider setup must not include raw API keys, OAuth tokens, refresh errors with token payloads, or full config records in lifecycle events.
- Host execution is not sandboxing. CLI/stream output should not claim isolation.
- Worker tools are registered with the cloned repo root as the workspace root/default and `<worker-root>/<id>/tool-output` as terminal trace storage.
- Current `packages/tools` permits absolute paths and `..` traversal intentionally; v1 must not describe this registration as confinement.
- Clone failures and tool failures should be concise and redacted.
- Future Docker sandboxing or an explicit scoped-tools mode should replace the containment layer without changing CLI command shape or lifecycle API.

## Performance and operations

- The daemon keeps one event ring and one broadcast channel per worker. Retention must be bounded to prevent unbounded memory growth.
- `StreamWorker` must not block event ingestion when a CLI client is slow; use broadcast lag handling and report missed events.
- Worker attach has a deadline. If the worker process starts but never attaches, status becomes `failed`.
- Child exit monitoring updates status to `failed`, `succeeded`, or `stopped` based on terminal frame and process status.
- Repo clone happens in the worker so daemon remains a lightweight lifecycle router.
- `list` reads from in-memory summaries and should stay cheap even with many terminal records. A retention cap for terminal records can be introduced after v1 if needed.
- `doric daemon` should handle Ctrl-C by sending shutdown commands to attached workers, then best-effort killing remaining child processes.
- Default worker root under `config_dir()/workers` keeps lifecycle artifacts out of the source tree.

## Testing strategy

Follow repository test-location rules: tests live under each package `tests/` directory, with only minimal `include!` harnesses in source when needed.

Unit tests:

- `packages/cli/tests/unit/main_cli.rs`: parse lifecycle commands, reject missing prompt/repo, preserve existing `config` and bare invocation behavior until intentionally changed.
- `packages/cli/tests/unit/entry.rs`: lifecycle and config command dispatch do not create `LlmDebugLogger`; bare chat still creates it before `chat::run`.
- `packages/cli/tests/unit/lifecycle_output.rs`: dispatch/list/stream/answer rendering, daemon unavailable errors, unknown worker errors, redacted repo output.
- `packages/lifecycle/tests/unit/proto_contract.rs`: generated message/service modules are reachable from the public `lifecycle::proto` namespace and build from the checked-in proto.
- `packages/lifecycle/tests/unit/redaction.rs`: URL credential redaction and API-key-like token redaction.
- `packages/daemon/tests/unit/registry.rs`: insert/list, status transitions, event sequence allocation, replay from sequence, history truncation, pending input validation, duplicate/stale answer rejection.
- `packages/daemon/tests/unit/process.rs`: worker attach timeout, child exit mapping, invalid worker root failures using temp roots.
- `packages/daemon/tests/unit/service.rs`: fake lifecycle service dispatch/list/stream/answer without spawning real workers.
- `packages/worker/tests/unit/provider.rs`: coding-model selection from `config`, cached context-window lookup, OpenRouter/OpenAI provider dispatch, OAuth load/save through `WorkerOpenAiAuthStore`, unsupported-provider errors, and disabled debug-logger default.
- `packages/worker/tests/unit/repo.rs`: repo-root layout, external Git command construction, missing Git/nonzero/timeout failure mapping through fake `GitCommandRunner` or fake `RepoPreparer`, invalid root handling.
- `packages/worker/tests/unit/runtime.rs`: fake daemon command stream, prompt runner events, input wait/resume, terminal status.
- `packages/worker/tests/unit/tooling.rs`: worker registers `packages/tools` with cloned `repo/` as the workspace root/default and `tool-output/` as trace storage, without changing chat registration behavior or adding confinement filters.
- `packages/tools/tests/unit/path.rs`, `write.rs`, and `terminal/mod.rs`: preserve current shared-tool semantics where relative paths default to the registered workspace root while absolute paths and `..` traversal remain allowed.
- `packages/worker/tests/unit/prompt_agent.rs`: prompt-agent-only event names, agent config/reasoning/context-policy mapping, and artifact write behavior using fake provider/tool seams.

Integration tests:

- In-process daemon service with fake worker session proves CLI client flow without live LLM or network repo.
- Spawn `doric-worker` with fake prompt runner feature/test mode if decomposition adds a test-only binary switch; otherwise keep process tests at daemon spawner seam.
- End-to-end smoke: daemon service accepts `feature`, list shows worker, stream replays accepted/start events, fake input request can be answered, terminal success appears.

Validation commands for implementation:

```text
cargo fmt --all -- --check
cargo build -p lifecycle
cargo test -p lifecycle --no-fail-fast
cargo test -p daemon --no-fail-fast
cargo test -p tools --no-fail-fast
cargo test -p worker --no-fail-fast
cargo test -p cli --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p cli --bin doric
cargo build -p daemon --bin doric-daemon
cargo build -p worker --bin doric-worker
```

Nx target validation:

```text
npx nx run lifecycle:build
npx nx run lifecycle:test
npx nx run daemon:build
npx nx run daemon:test
npx nx run tools:test
npx nx run worker:build
npx nx run worker:test
npx nx run cli:build
npx nx run cli:test
```

`cargo build -p lifecycle` and `npx nx run lifecycle:build` are the explicit codegen gates. They must compile the `tonic-prost-build` output and resolve the generated `tonic_prost::ProstCodec` runtime dependency. On Windows they must succeed from a clean checkout through vendored `protoc` unless `PROTOC` is deliberately set. If local Nx or Clippy tooling is unavailable, record the tool failure and run the Cargo fallback commands with an isolated target dir when Windows file locks appear.

## Rollout and migration

1. Add `lifecycle`, `daemon`, and `worker` package skeletons with Cargo/Nx metadata.
2. Add lifecycle proto/contracts, `build.rs` using `tonic-prost-build`, vendored-protoc fallback, runtime `tonic-prost` wiring, and redaction/codegen tests.
3. Add daemon registry and service tests before process spawning.
4. Add worker repo preparation around the external Git command seam, with fake command-runner tests before any network clone smoke.
5. Add worker provider/runtime composition tests and implementation using public `config`, `llms`, and `agent` APIs; keep chat composition untouched.
6. Add worker runtime and fake prompt runner tests before live agent/provider integration.
7. Add CLI parse/routing/output tests, including the entrypoint rewrite that keeps debug-log startup chat-scoped.
8. Wire daemon gRPC service and CLI clients.
9. Wire worker process spawn and worker attach stream.
10. Register shared `packages/tools` from worker with the cloned repo as workspace root/default and `tool-output/` as trace dir; keep existing chat registration untouched and do not add confinement in v1.
11. Add prompt/requirements agent runner and artifact output.
12. Update `docs/architecture-and-packages.md` to reflect the new package map, dependency direction, lifecycle flow, shared tool ownership, and worker-local provider/runtime composition.
13. Run package and workspace validation, including lifecycle build/codegen gates through Cargo and Nx.

Migration notes:

- Existing user config schema does not need a worker-root field in v1 because `doric daemon --worker-root` and `config_dir()/workers` cover the first slice. A future config field can be added if operators need persistent daemon settings.
- Existing chat sessions remain separate from lifecycle worker state.
- Existing chat continues to depend on `packages/tools`; worker adds a second consumer of that same package, not a second implementation or a scoped fork.
- Existing chat provider/runtime modules remain in `packages/cli/src/chat/*` for v1. Worker adds local provider/runtime composition and may be reconciled into a lower `runtime` package only after this lifecycle slice is validated.
- No `git2`/Rust-Git migration is planned in v1. The external Git seam can be replaced later only if a separate design accepts the build, credential, and platform tradeoffs.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| gRPC/protobuf dependency integration adds build complexity. | New crates can fail before lifecycle code is tested, especially on Windows without system `protoc` or if generated service runtime crates are incomplete. | Keep generated contracts isolated in `lifecycle`; use `tonic-prost-build`, `tonic-prost`, and vendored protoc fallback; validate `cargo build -p lifecycle` and Nx lifecycle build first. |
| Worker attach race or timeout leaves accepted workers stuck. | Users see accepted jobs that never run. | Add attach deadline and failed transition with reason. |
| In-memory registry surprises users after daemon restart. | Prior worker ids disappear. | Make restart behavior explicit in stream/list errors and tests. |
| Host execution is mistaken for sandboxing. | Users may run untrusted repos with false confidence. | State host-root behavior plainly; no isolation claims. Treat Docker or a future explicit scoped-tools mode as the containment boundary, not shared tool registration. |
| Tool workspace root/default is mistaken for confinement. | Decomposition may plan false path-safety guarantees or tests that current `packages/tools` cannot satisfy. | Say worker tools are registered with a repo root default, not sandboxed; preserve existing absolute-path and traversal behavior in `tools` tests; defer confinement to a separate design. |
| External Git clone depends on local Git installation and credentials. | Workers can fail before prompt-agent execution. | Detect missing Git, map clone failures to redacted lifecycle events, and keep automated tests on fake command runners. |
| Shared `packages/tools` changes break existing chat. | Wider blast radius than lifecycle work. | Do not move tool modules or change their path semantics in this slice; add worker as a consumer and run `tools`, `cli`, and `worker` tests when touching tool registration. |
| Worker-local provider/runtime composition drifts from chat behavior. | Worker and chat may select providers, reasoning, OAuth refresh, or context policy differently. | Keep duplication narrow and tested; name the duplicated chat-local modules in the worker provider tests; defer extraction until both consumers prove the shared shape. |
| Worker provider setup leaks secrets through lifecycle errors. | API keys or OAuth tokens could appear in daemon events or CLI streams. | Map provider/config errors into redacted worker failure events and unit-test OAuth/API-key redaction around provider composition failures. |
| Secret-bearing repo URLs leak in events. | Credential exposure. | Central redaction helper and required tests for dispatch/list/stream/failure rendering. |
| Prompt agent accidentally implies full Doric SDLC. | Product overclaim and acceptance failure. | Constrain event names and artifact output to prompt/requirements only. |
| Live provider tests become flaky or costly. | CI instability. | Use fake prompt runner/provider seams for automated tests; keep live-provider smoke manual unless explicitly requested. |

## Open questions

All PRD open questions have v1 decisions above. Remaining non-blocking technical questions for decomposition:

1. Should the daemon service and `doric daemon` share one library entry point exactly, or should `doric daemon` exec `doric-daemon`? Default: share `daemon::run` to keep tests simple.
2. Should event ring retention be count-only or count-plus-byte bounded? Default: count-only at 1,000 events for first implementation, with a byte cap added if tests expose large event pressure.
3. Should worker root be added to persisted config after v1? Default: no persisted schema change until operators need daemon settings beyond CLI flags.
4. Should the old bare chat path be retired in the same product direction? Default: no, because this TDD covers lifecycle commands only and preserving it lowers migration risk.

No open question blocks decomposition.
