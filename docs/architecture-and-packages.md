# Architecture And Packages

Doric is an Nx workspace backed by a Rust Cargo workspace. The user-facing
binary is `doric`; the Cargo package and Nx project that build it are named
`cli`.

## Workspace Shape

The Rust workspace is declared in `Cargo.toml` and contains these packages:

| Package | Path | Responsibility |
| ------- | ---- | -------------- |
| `cli` | `packages/cli` | `doric` CLI entry point, application composition, config command output, chat orchestration, session persistence, lifecycle client commands, daemon startup command, and adapters between command, agent, TUI, provider, tool, daemon, and lifecycle packages. |
| `agent` | `packages/agent` | Agent runtime, event stream, context assembly, request lifecycle, queueing, cancellation, retries, provider streaming, tool loop, and store contracts. |
| `lifecycle` | `packages/lifecycle` | Shared lifecycle domain types, worker identity/status/event models, redaction helpers, repository identity parsing, and generated gRPC/protobuf client/server contracts. |
| `daemon` | `packages/daemon` | `doric-daemon` application, lifecycle gRPC server, worker registry, worker root/layout preparation, worker process launch, authenticated worker-session attachment, command routing, and event replay/streaming. |
| `worker` | `packages/worker` | `doric-worker` application, daemon-controlled worker runtime, repository preparation, prompt-agent execution, worker-local provider/runtime selection, shared tool registration for prepared repos, state models, and lifecycle event/status reporting. |
| `llms` | `packages/llms` | Provider traits and types, provider registry, OpenAI/OpenRouter integrations, model metadata, streaming DTOs, auth flow, and provider debug logging. |
| `tools` | `packages/tools` | Built-in host tools for file search, grep, tree, terminal execution, edit/write operations, diff previews, and web search/open/find. |
| `tui` | `packages/tui` | IOCraft terminal UI state, actions, reducers, layout, rendering model, prompt editing, selection, transcript components, and runtime glue. |
| `commands` | `packages/commands` | Generic slash-command parsing, command metadata, fuzzy search, and command dispatch primitives. |
| `config` | `packages/config` | Persisted Doric config schema, provider/model/task validation, model cache, config path resolution, and config IO. |

`package.json` declares the JavaScript workspace as `packages/*`; package
`project.json` files expose the Rust crates to Nx through `@monodon/rust`
build, test, lint, and run targets.

`packages/vendor/iocraft` is excluded from the workspace and patched through
`[patch.crates-io]` so workspace packages use the vendored IOCraft source.

## Dependency Direction

The current package dependency direction is:

```text
cli
 |-- agent -----> llms
 |-- daemon ----> lifecycle
 |      |-------> config
 |-- lifecycle
 |-- tools -----> agent
 |-- tui -------> commands
 |-- commands
 |-- config
 |-- llms
 |
 v
doric binary

daemon --process launch--> doric-worker binary

worker
 |-- agent -----> llms
 |-- config
 |-- lifecycle
 |-- llms
 `-- tools -----> agent
```

`cli` is the composition root. It wires configuration, provider selection,
agent execution, built-in tool registration, slash-command handling, session
storage, lifecycle client commands, daemon startup, and the IOCraft TUI
together. Lower-level packages do not depend on `cli`.

`lifecycle` sits below `cli`, `daemon`, and `worker` as the shared lifecycle
contract package. It uses `build.rs` plus the v1 protobuf stack
`tonic-prost-build`, `tonic-prost`, and `prost` to generate the
`doric.lifecycle.v1` gRPC types from `proto/doric/lifecycle/v1.proto`.

`daemon` depends on `lifecycle` and `config`, but not on `worker` as a Rust
crate. It launches the `doric-worker` binary as an external sibling process and
passes a worker id, daemon address, one-time token, and worker root over the
process command line.

`worker` depends on `agent`, `config`, `lifecycle`, `llms`, and `tools`.
Provider and agent runtime composition for lifecycle jobs is worker-local in
v1, while normal interactive chat provider/runtime composition remains in
`cli`.

## Runtime Flow

```text
User terminal
   |
   v
doric binary (`cli`)
   |
   +-- config commands ----------> config
   |
   +-- daemon command -----------> daemon server (`doric-daemon`)
   |
   +-- lifecycle commands -------> lifecycle gRPC client
   |                                  |
   |                                  v
   |                              daemon lifecycle service
   |                                  |
   |                                  v
   |                              worker process (`doric-worker`)
   |                                  |
   |                                  +-- repo preparation using external Git
   |                                  +-- worker-local provider/runtime composition
   |                                  +-- shared tools registered against worker repo
   |
   +-- chat/TUI controller ------> tui reducer/runtime/view
   |                                  |
   |                                  v
   |                              rendered terminal UI
   |
   +-- agent turn runner --------> agent
                                      |
                                      +-- provider calls ----> llms
                                      |
                                      +-- tool calls --------> tools
                                      |
                                      +-- event stream ------> session persistence + TUI actions
```

Config, daemon, and lifecycle subcommands are dispatched before chat startup.
Lifecycle commands bypass chat debug-log creation because they route through the
daemon/lifecycle command handlers instead of the bare chat branch. Bare `doric`
chat behavior still creates the provider debug-log logger before entering the
chat runner.

Daemon-managed workers prepare a per-worker layout under the daemon worker root:
`repo/`, `state/`, `artifacts/`, and `tool-output/`. In v1 the worker clones the
requested repository by invoking external Git (`git clone -- <repo> <repo-dir>`)
through the worker repository boundary.

## Config And Local Data

Doric stores local configuration under an OS config directory named `doric`.
The config package owns `config.json`, `model-cache.json`, provider credentials,
model slots, task assignments, validation, and schema handling.

Chat sessions are persisted as JSONL records by the `cli` package under the
Doric config directory. Session records contain user prompts, assistant deltas,
reasoning deltas, tool calls/results, command lifecycle events, usage metadata,
errors, cancellations, and finish events.

## Extension Points

Provider additions start in `packages/llms` with provider types, registry
metadata, streaming/parsing code, and debug logging. `packages/cli` composes the
enabled providers for chat and model refresh flows. `packages/worker` separately
selects and composes worker-local providers for lifecycle jobs from persisted
config and the model cache.

Built-in tool additions start in `packages/tools` and implement the agent
`Tool` trait from `packages/agent`. `packages/cli/src/chat/runner.rs` registers
the tool set used by chat runs. `packages/worker/src/tooling.rs` registers the
same shared tools against a prepared worker repo and trace-output directory; the
worker registration is not confinement, and `packages/tools` remains the shared
owner of built-in tool implementations.

Slash-command additions use generic parsing and metadata from `packages/commands`
and are registered in `packages/cli/src/chat/commands/registry.rs`.

TUI behavior belongs in `packages/tui`. New visible transcript, prompt, footer,
selection, or status behavior should be expressed through IOCraft components
under `packages/tui/src/components` rather than new flattened line formatters.
