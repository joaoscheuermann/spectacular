# Architecture And Packages

Doric is an Nx workspace backed by a Rust Cargo workspace. The user-facing
binary is `doric`; the Cargo package and Nx project that build it are named
`cli`.

## Workspace Shape

The Rust workspace is declared in `Cargo.toml` and contains these packages:

| Package | Path | Responsibility |
| ------- | ---- | -------------- |
| `cli` | `packages/cli` | CLI entry point, application composition, config command output, chat orchestration, session persistence, and adapters between command, agent, TUI, provider, and tool packages. |
| `agent` | `packages/agent` | Agent runtime, event stream, context assembly, request lifecycle, queueing, cancellation, retries, provider streaming, tool loop, and store contracts. |
| `llms` | `packages/llms` | Provider traits and types, provider registry, OpenAI/OpenRouter integrations, model metadata, streaming DTOs, auth flow, and provider debug logging. |
| `tools` | `packages/tools` | Built-in host tools for file search, grep, tree, terminal execution, edit/write operations, diff previews, and web search/open/find. |
| `tui` | `packages/tui` | IOCraft terminal UI state, actions, reducers, layout, rendering model, prompt editing, selection, transcript components, and runtime glue. |
| `commands` | `packages/commands` | Generic slash-command parsing, command metadata, fuzzy search, and command dispatch primitives. |
| `config` | `packages/config` | Persisted Doric config schema, provider/model/task validation, model cache, config path resolution, and config IO. |

`packages/vendor/iocraft` is excluded from the workspace and patched through
`[patch.crates-io]` so workspace packages use the vendored IOCraft source.

## Dependency Direction

The intended package dependency direction is:

```text
cli
 |-- agent -----> llms
 |-- tools -----> agent
 |-- tui -------> commands
 |-- commands
 |-- config
 |-- llms
 |
 v
doric binary
```

`cli` is the composition root. It wires configuration, provider selection,
agent execution, built-in tool registration, slash-command handling, session
storage, and the IOCraft TUI together. Lower-level packages do not depend on
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
enabled providers for chat and model refresh flows.

Built-in tool additions start in `packages/tools` and implement the agent
`Tool` trait from `packages/agent`. `packages/cli/src/chat/runner.rs` registers
the tool set used by chat runs.

Slash-command additions use generic parsing and metadata from `packages/commands`
and are registered in `packages/cli/src/chat/commands/registry.rs`.

TUI behavior belongs in `packages/tui`. New visible transcript, prompt, footer,
selection, or status behavior should be expressed through IOCraft components
under `packages/tui/src/components` rather than new flattened line formatters.
