# Spectacular

Spectacular is a terminal AI assistant for working inside a local codebase.
It runs as a native chat loop, streams model output, keeps session history, and
lets the model use built-in tools to inspect, edit, search, and run commands in
the current workspace.

The main product surface today is `spectacular chat`. The longer-term direction
is spec-driven development, but the current working functionality is centered on
chat, tool use, sessions, provider configuration, and repository workflows.

## Functionality

### Terminal Chat

Start a fresh terminal chat session:

```sh
npx nx run spectacular:run --args='chat'
```

The chat experience is transcript-first: no fullscreen TUI, no fixed panels,
and normal terminal scrollback. User prompts and assistant responses stay in the
terminal output, while the prompt editor handles interactive input.

The prompt supports:

- Multiline input.
- Bracketed paste and fallback paste-burst handling.
- Slash-command suggestions.
- Tab completion for command names.
- Quoted command arguments.
- `Enter` to submit.
- `Shift+Enter`, `Alt+Enter`, `Ctrl+Enter`, or `Ctrl+J` to insert a newline.
- `Ctrl+C` to clear the current prompt, or exit when the prompt is empty.

### Slash Commands

Slash commands are strict. Empty commands, uppercase command names, unknown
commands, and unterminated quotes are errors.

| Command | What it does |
| ------- | ------------ |
| `/new` | Starts a new chat session. |
| `/history [page|start-end]` | Lists saved sessions. |
| `/resume <session-id>` | Resumes a saved session by id or unique prefix. |
| `/clear` | Clears the visible terminal output. |
| `/exit` | Exits chat. |
| `/provider [configured-provider-id]` | Shows or switches the active provider. |
| `/model [model-id none|low|medium|high]` | Shows or updates the coding model and reasoning level. |
| `/reasoning [none|low|medium|high]` | Shows or updates coding reasoning. |
| `/retry` | Replays the latest prompt after truncating the previous response. |
| `/git-status` | Shows working tree status and staged diff stats. |
| `/git-commit` | Generates a conventional commit message for staged changes and commits them. |

### Built-In Tools

The main chat agent exposes these tools to the model:

| Tool | What it does |
| ---- | ------------ |
| `find` | Finds files by glob, respecting `.gitignore`. |
| `grep` | Searches file contents with regex or literal matching. |
| `tree` | Prints a gitignore-aware ASCII directory tree. |
| `terminal` | Runs shell commands and returns stdout, stderr, and exit code. |
| `edit` | Applies exact text replacements to existing files. |
| `write` | Creates or overwrites files, including parent directories. |
| `web` | Searches the web, opens pages, and finds text in pages. |

Tool calls are model-facing and currently run without an approval prompt. Use
Spectacular in workspaces where file writes and command execution are intended.

### Sessions

Chat sessions are persisted as structured JSONL records. A session can include:

- The session id and title.
- Provider and model changes.
- User prompts and assistant deltas.
- Reasoning deltas.
- Tool calls and tool results.
- Usage metadata.
- Errors, cancellations, and finish reasons.

Useful session behavior:

- `spectacular chat` starts a fresh session by default.
- `/history` lists recent saved sessions.
- `/resume <session-id>` restores a previous session.
- `/retry` truncates after the latest user prompt and reruns it.
- Titles are generated in the background after the first assistant response.

### Provider And Model Configuration

OpenRouter is the enabled provider implementation in this checkout.

Spectacular stores provider settings locally and supports three model slots:

- `coding`: used by `spectacular chat`.
- `labeling`: used for background session titles when configured.
- `planning`: reserved for the planning route.

Reasoning levels:

- `none`
- `low`
- `medium`
- `high`

Show current configuration:

```sh
npx nx run spectacular:run --args='config'
```

Configure OpenRouter:

```sh
npx nx run spectacular:run --args='config --provider openrouter --key sk-or-v1-your-key'
npx nx run spectacular:run --args='config --use openrouter'
npx nx run spectacular:run --args='config --provider openrouter --task coding --model openrouter/your-model --reasoning medium'
```

Optional title model:

```sh
npx nx run spectacular:run --args='config --provider openrouter --task labeling --model openrouter/title-model --reasoning none'
```

### Planning Command

`spectacular plan <prompt>` exists, but it is not a real planning workflow yet.
It validates a non-empty prompt and complete config, then returns:

```text
Hello World
```

## Quick Start

Prerequisites:

- Node.js 20+
- npm
- Rust stable
- OpenRouter API key

Install dependencies:

```sh
npm ci
```

Build:

```sh
cargo build -p spectacular
```

Configure the chat model:

```sh
npx nx run spectacular:run --args='config --provider openrouter --key sk-or-v1-your-key'
npx nx run spectacular:run --args='config --use openrouter'
npx nx run spectacular:run --args='config --provider openrouter --task coding --model openrouter/your-model --reasoning medium'
```

Start chat:

```sh
npx nx run spectacular:run --args='chat'
```

## Local Data

Configuration and sessions are stored outside the repo.

Windows:

```text
%APPDATA%\spectacular\config.json
%APPDATA%\spectacular\sessions\*.jsonl
```

macOS:

```text
~/Library/Application Support/spectacular/config.json
~/Library/Application Support/spectacular/sessions/*.jsonl
```

Linux:

```text
$XDG_CONFIG_HOME/spectacular/config.json
$XDG_CONFIG_HOME/spectacular/sessions/*.jsonl
```

If `XDG_CONFIG_HOME` is not set, Linux uses `~/.config/spectacular`.

API keys are stored as plain text in `config.json`.

## Development

Spectacular is an Nx workspace backed by a Rust Cargo workspace.

| Package | Purpose |
| ------- | ------- |
| `spectacular` | CLI, chat loop, prompt editor, renderer, sessions, and chat commands. |
| `spectacular-agent` | Agent runtime, streaming, retries, continuation, tool loop, and store. |
| `spectacular-llms` | Provider traits, provider types, registry, and OpenRouter. |
| `spectacular-tools` | Built-in file, terminal, web, search, edit, and write tools. |
| `spectacular-commands` | Slash-command parsing, metadata, fuzzy search, and errors. |
| `spectacular-config` | Config schema, persistence, validation, and migration. |
| `spectacular-plan` | Placeholder planning command. |

Common commands:

```sh
npx nx show projects
npx nx test spectacular
npx nx run-many -t lint build test
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

CI runs `cargo nextest run --workspace --all-features` and Nx `lint`, `build`,
and `typecheck`.
