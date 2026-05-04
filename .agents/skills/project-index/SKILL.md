---
name: project-index
description: Spectacular repository map and file index. Use when Codex needs quick context about this repo, its Nx/Cargo workspace layout, local agent skills, Rust CLI crates, source-controlled files, planning, coding, reviewing, or debugging.
---

# Project Index

Last updated: 2026-05-01

This skill gives agents a compact, source-of-truth map of the current Spectacular repository. It indexes files returned by `git ls-files --cached --others --exclude-standard` and keeps generated, dependency, and local-only paths out of startup context.

Omitted intentionally:

- `.git/` - Git object database and local repository metadata.
- `.nx/cache/`, `.nx/workspace-data/`, `dist/`, `tmp/`, `out-tsc/`, `coverage/` - generated task outputs and caches.
- `node_modules/` - installed npm dependencies.
- `.journal/` - ignored local planning and delivery history; read it only when a task explicitly asks for journal context.

## Repository Summary

Spectacular is an Nx workspace backed by a Rust Cargo workspace. The current indexed source contains the `spectacular` Rust CLI, an Aider-style terminal chat surface, a reusable slash-command registry, an agent runtime crate, LLM provider abstractions, built-in agent tools, configuration persistence, and placeholder planning behavior. The repo also includes local `.agents/skills` playbooks for Nx operations, coding conventions, debugging, CI monitoring, journaling, and repository indexing.

## Tree

```text
.
|-- .agents/ - Agent-facing repository context and reusable skill instructions.
|   `-- skills/ - Local skill playbooks used by coding agents.
|       |-- architect/
|       |   `-- SKILL.md - Planning-only architecture workflow for Nx monorepo changes.
|       |-- coding-conventions/
|       |   |-- SKILL.md - Shared design principles and implementation conventions.
|       |   `-- references/
|       |       |-- dependency-injection.md - Dependency injection guidance and examples.
|       |       |-- dip.md - Dependency Inversion Principle guidance.
|       |       |-- dry.md - DRY principle guidance.
|       |       |-- early-returns.md - Early return and control-flow style guidance.
|       |       |-- functional-programming.md - Functional programming guidance for implementation work.
|       |       |-- isp.md - Interface Segregation Principle guidance.
|       |       |-- kiss.md - KISS principle guidance.
|       |       |-- monodon-rust.md - Rust project guidance for `@monodon/rust` in Nx.
|       |       |-- nxlv-python.md - Python project guidance for `@nxlv/python` in Nx.
|       |       |-- ocp.md - Open/Closed Principle guidance.
|       |       `-- srp.md - Single Responsibility Principle guidance.
|       |-- debug-coordinator/
|       |   `-- SKILL.md - Coordinates journal-backed debugging from triage to verification.
|       |-- debugger/
|       |   `-- SKILL.md - Hypothesis-driven bug investigation workflow.
|       |-- decomposer/
|       |   `-- SKILL.md - Breaks approved architecture into ordered effort files.
|       |-- developer/
|       |   `-- SKILL.md - Implements approved efforts while following repo conventions.
|       |-- discuss/
|       |   `-- SKILL.md - One-question-at-a-time plan interrogation and decision clarification.
|       |-- effort-executor/
|       |   `-- SKILL.md - Orchestrates effort execution and journal status lifecycle.
|       |-- generate-project-index/
|       |   |-- SKILL.md - Workflow and reusable prompt for regenerating this repository index skill.
|       |   `-- agents/
|       |       `-- openai.yaml - UI metadata for the project-index generator skill.
|       |-- journal-manager/
|       |   `-- SKILL.md - File protocol for creating and updating `.journal` entries.
|       |-- link-workspace-packages/
|       |   `-- SKILL.md - Adds workspace package dependencies through package-manager commands.
|       |-- monitor-ci/
|       |   |-- SKILL.md - Watches Nx Cloud CI and coordinates self-healing fixes.
|       |   |-- references/
|       |   |   `-- fix-flows.md - CI failure remediation flow reference.
|       |   `-- scripts/
|       |       |-- ci-poll-decide.mjs - Polls CI state and decides the next monitoring action.
|       |       `-- ci-state-update.mjs - Updates persisted CI monitoring state.
|       |-- nx-generate/
|       |   `-- SKILL.md - Nx generator workflow for scaffolding apps, libs, and structure.
|       |-- nx-import/
|       |   |-- SKILL.md - Nx repository import workflow.
|       |   `-- references/
|       |       |-- ESLINT.md - Import notes for ESLint-based repositories.
|       |       |-- GRADLE.md - Import notes for Gradle-based repositories.
|       |       |-- JEST.md - Import notes for Jest-based repositories.
|       |       |-- NEXT.md - Import notes for Next.js repositories.
|       |       |-- TURBOREPO.md - Import notes for Turborepo migrations.
|       |       `-- VITE.md - Import notes for Vite repositories.
|       |-- nx-plugins/
|       |   `-- SKILL.md - Discovers and installs Nx plugins.
|       |-- nx-run-tasks/
|       |   `-- SKILL.md - Runs Nx build, test, lint, serve, affected, and run-many tasks.
|       |-- nx-workspace/
|       |   |-- SKILL.md - Read-only Nx workspace exploration and task discovery.
|       |   `-- references/
|       |       `-- AFFECTED.md - Guidance for Nx affected commands.
|       |-- project-index/
|       |   |-- SKILL.md - This repository map for quick agent startup context.
|       |   `-- agents/
|       |       `-- openai.yaml - UI metadata for this project-index skill.
|       |-- spec/
|       |   `-- SKILL.md - Requirements elicitation, user stories, acceptance criteria, and edge cases.
|       `-- tester/
|           `-- SKILL.md - Verification workflow for efforts and bug fixes.
|-- .cargo/
|   `-- config.toml - Cargo configuration that places build outputs under `dist/target`.
|-- .github/
|   `-- workflows/
|       `-- ci.yml - GitHub Actions workflow for Nx CI, install, format, run-many, and fix-ci.
|-- .vscode/
|   `-- extensions.json - Recommended VS Code extensions for the workspace.
|-- packages/
|   |-- .gitkeep - Keeps the packages directory present when no packages are generated.
|   |-- spectacular/ - Rust CLI application crate with config, plan, and terminal chat commands.
|   |   |-- Cargo.toml - Application manifest with Clap, terminal I/O, Tokio, and local crate dependencies.
|   |   |-- project.json - Nx targets for Rust build, test, lint, and run via `@monodon/rust`.
|   |   `-- src/
|   |       |-- main.rs - CLI entry point for `chat`, `config`, and `plan`, plus config output, errors, and tests.
|   |       `-- chat/
|   |           |-- mod.rs - Chat context, loop, runtime selection, provider/model commands, and terminal fallback I/O.
|   |           |-- prompt.rs - Interactive prompt editor used for terminal chat input and command suggestions.
|   |           |-- provider.rs - Runtime provider construction from selected provider, API key, model, and reasoning state.
|   |           |-- renderer.rs - Terminal rendering helpers for prompts, assistant output, tool display, commands, warnings, and status.
|   |           |-- runner.rs - Agent-backed chat run orchestration, built-in tool wiring, Ctrl-C cancellation, event rendering, persistence, and title generation.
|   |           |-- commands/
|   |           |   |-- mod.rs - Registers chat slash commands into the shared command registry.
|   |           |   |-- config/
|   |           |   |   |-- mod.rs - Chat configuration command module boundary.
|   |           |   |   |-- model.rs - `/model` command for showing or updating the coding model.
|   |           |   |   |-- provider.rs - `/provider` command for showing or switching the active provider.
|   |           |   |   `-- reasoning.rs - `/reasoning` command for showing or updating coding reasoning effort.
|   |           |   |-- runtime/
|   |           |   |   |-- mod.rs - Runtime command module boundary.
|   |           |   |   `-- retry.rs - `/retry` command for rerunning the latest prompt with current runtime settings.
|   |           |   `-- session/
|   |           |       |-- mod.rs - Session command module boundary.
|   |           |       |-- clear.rs - `/clear` command for clearing visible terminal output.
|   |           |       |-- exit.rs - `/exit` command for leaving the chat loop.
|   |           |       |-- history.rs - `/history` command for listing saved chat sessions.
|   |           |       |-- new.rs - `/new` command for starting a new chat session.
|   |           |       `-- resume.rs - `/resume` command for restoring an earlier session.
|   |           |-- session.rs - Session manager, record parsing, session creation, title handling, schema version, and tests.
|   |           `-- session/
|   |               |-- event.rs - Persisted chat event model and conversion between chat events and agent events.
|   |               |-- index.rs - Session index file model and persistence helpers.
|   |               `-- store.rs - Filesystem-backed session storage and JSONL record access.
|   |-- spectacular-agent/ - Agent orchestration library crate.
|   |   |-- Cargo.toml - Agent manifest with JSON schema, Tokio, and LLM crate dependencies.
|   |   |-- project.json - Nx targets for Rust check, test, and lint.
|   |   |-- examples/
|   |   |   |-- cancellation.rs - Example for cancelling an active agent run.
|   |   |   |-- context_filtering.rs - Example for converting stored events into provider context.
|   |   |   |-- error_paths.rs - Example for provider and agent error handling.
|   |   |   |-- no_tool_run.rs - Example for a basic agent run without tools.
|   |   |   |-- queued_runs.rs - Example for queued agent run behavior.
|   |   |   |-- structured_output.rs - Example for JSON-schema-backed response validation.
|   |   |   `-- tool_loop.rs - Example for provider-requested tool execution loops.
|   |   `-- src/
|   |       |-- agent.rs - Core agent runtime, queue integration, provider streaming, tools, validation, cancellation, and tests.
|   |       |-- context.rs - Provider context construction and context-limit validation.
|   |       |-- error.rs - Agent error enum and provider error mapping.
|   |       |-- event.rs - Agent event model for prompts, deltas, tools, validation, cancellation, and finishes.
|   |       |-- lib.rs - Public module declarations and re-exports for the agent crate.
|   |       |-- queue.rs - FIFO run queue with manual queueing, waiters, and cancellation behavior.
|   |       |-- schema.rs - JSON schema wrapper for structured assistant response validation.
|   |       |-- store.rs - Append-only event store with checkpoints and rollback.
|   |       `-- tool.rs - Tool trait, registry, execution, error formatting, and provider-visible tool calls.
|   |-- spectacular-commands/ - Reusable slash-command parser and registry crate.
|   |   |-- Cargo.toml - Commands crate manifest.
|   |   |-- project.json - Nx targets for Rust check, test, and lint.
|   |   `-- src/
|   |       `-- lib.rs - Command metadata, registry, slash parser, quoted-argument parser, fuzzy search, and command errors.
|   |-- spectacular-config/ - Configuration persistence crate.
|   |   |-- Cargo.toml - Config crate manifest with Serde dependencies.
|   |   |-- project.json - Nx targets for Rust check, test, and lint.
|   |   `-- src/
|   |       `-- lib.rs - Config path resolution, read/write/migration logic, provider API keys, task models, validation, and tests.
|   |-- spectacular-llms/ - LLM provider abstraction and OpenRouter implementation crate.
|   |   |-- Cargo.toml - LLM crate manifest with reqwest, Serde, JSON, Tokio, futures, and test dependencies.
|   |   |-- project.json - Nx targets for Rust check, test, and lint.
|   |   |-- examples/
|   |   |   `-- provider_capabilities.rs - Example for inspecting enabled provider capabilities.
|   |   `-- src/
|   |       `-- lib.rs - Provider registry, OpenRouter validation/models/chat streaming, provider stream types, capabilities, and errors.
|   |-- spectacular-tools/ - Built-in agent tool implementations crate.
|   |   |-- Cargo.toml - Tools crate manifest with anstyle, glob, ignore, regex, serde, similar, agent, and Tokio dependencies.
|   |   |-- project.json - Nx targets for Rust check, test, and lint.
|   |   `-- src/
|   |       |-- display.rs - Shared display styling helpers for built-in tool formatters.
|   |       |-- edit.rs - `edit` built-in tool with normalized matching, diff output, BOM and line-ending preservation, and tests.
|   |       |-- find.rs - `find` built-in tool for gitignore-aware filename discovery and output truncation.
|   |       |-- grep.rs - `grep` built-in tool for regex search with context, glob filtering, and error payloads.
|   |       |-- lib.rs - Built-in tool exports and factory that registers edit, find, grep, terminal, tree, and write tools.
|   |       |-- path.rs - Workspace-root path resolution helper shared by built-in tools.
|   |       |-- terminal.rs - `terminal` built-in tool with shell selection, timeout clamp, cancellation, and process output contract.
|   |       |-- test_support.rs - Temporary workspace helpers shared by tools crate tests.
|   |       |-- tree.rs - `tree` built-in tool for gitignore-aware ASCII directory trees.
|   |       `-- write.rs - `write` built-in tool for creating parent directories and writing file content.
|   `-- spectacular-plan/ - Placeholder planning route crate.
|       |-- Cargo.toml - Plan crate manifest depending on `spectacular-config`.
|       |-- project.json - Nx targets for Rust check, test, and lint.
|       `-- src/
|           `-- lib.rs - Validates non-empty plan prompts and complete config before returning placeholder output.
|-- .gitignore - Ignore rules for generated outputs, dependency directories, editor files, Nx cache, and local journals.
|-- .prettierignore - Files excluded from Prettier formatting.
|-- .prettierrc - Prettier configuration.
|-- Cargo.lock - Locked Rust dependency graph for reproducible builds.
|-- Cargo.toml - Root Cargo workspace manifest and release profile.
|-- README.md - Nx template overview, included Rust/Python/TS tooling, agent skill catalog, and everyday commands.
|-- nx.json - Nx workspace configuration, plugins, named inputs, and disabled cloud analytics.
|-- opencode.json - Local opencode configuration file.
|-- package-lock.json - Locked npm dependency graph for reproducible installs.
|-- package.json - npm workspace manifest and Nx/TypeScript/Rust/Python dev dependencies.
|-- tsconfig.base.json - Root TypeScript compiler options shared by workspace projects.
`-- tsconfig.json - Root TypeScript project reference container.
```

## Working Notes For Agents

- Use `git ls-files --cached --others --exclude-standard` as the file inventory source when refreshing this index.
- Prefer Nx targets when available: `npx nx <target> <project>`, for example `npx nx test spectacular-agent`.
- Rust crates are members of the root Cargo workspace, so `cargo test -p <crate>` is also a useful direct verification path.
- The main CLI now exposes `chat`, `config`, and `plan`. Use `npx nx run spectacular:run --args='chat'` or direct Cargo runs when interactive terminal behavior matters.
- Chat runtime behavior flows through `spectacular-agent` plus `spectacular-llms`; slash-command parsing and fuzzy command search live in `spectacular-commands`.
- OpenRouter is the enabled provider implementation; OpenAI and Google Gemini remain disabled metadata placeholders unless current code changes that registry.
- `spectacular-plan` still returns placeholder planning output after prompt and config validation.
- The repo has local agent skills under `.agents/skills`; load the relevant skill before following a specialized workflow.
- Do not treat ignored `.journal` files as normal source unless the task explicitly asks for journal planning or effort execution.
