---
name: project-index
description: Spectacular repository map and file index. Use when Codex needs quick context about this repo, its Nx/Cargo workspace layout, local agent skills, Rust crates, GPUI app structure, config/theme files, or source-controlled files before planning, coding, reviewing, or debugging.
---

# Project Index

Last updated: 2026-04-28

This skill gives agents a compact map of the current Spectacular repository. It indexes source-controlled and unignored project files, with generated/dependency folders omitted so startup context stays useful.

Omitted intentionally:

- `.git/` - Git object database and local repository metadata.
- `.nx/cache/`, `.nx/workspace-data/`, `dist/`, `tmp/`, `out-tsc/`, `coverage/` - generated task outputs and caches.
- `node_modules/` - installed npm dependencies.
- `.journal/` - ignored local planning and delivery history; read it only when a task explicitly asks for journal context.

## Repository Summary

Spectacular is an Nx workspace backed by a Rust Cargo workspace. It currently contains a GPUI desktop application (`spectacular`) plus supporting Rust crates for agent orchestration, provider abstractions, config persistence, and a placeholder planning route. The repo also contains `.agents/skills` playbooks that steer coding, Nx, debugging, CI, and journal workflows.

## Tree

```text
.
|-- .agents/ - Agent-facing repository context and reusable skill instructions.
|   `-- skills/ - Local skill playbooks used by coding agents.
|       |-- architect/
|       |   `-- SKILL.md - Planning-only architecture workflow for Nx monorepo changes.
|       |-- coding-conventions/
|       |   |-- SKILL.md - Shared implementation conventions and references.
|       |   `-- references/
|       |       |-- dependency-injection.md - Dependency injection guidance and examples.
|       |       |-- dip.md - Dependency Inversion Principle guidance.
|       |       |-- dry.md - DRY principle guidance.
|       |       |-- early-returns.md - Early return and control-flow style guidance.
|       |       |-- functional-programming.md - Functional style guidance for implementation work.
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
|       |-- effort-executor/
|       |   `-- SKILL.md - Orchestrates effort execution and status lifecycle.
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
|       |   `-- SKILL.md - Requirements elicitation, stories, acceptance criteria, and edge cases.
|       `-- tester/
|           `-- SKILL.md - Verification workflow for efforts and bug fixes.
|-- .cargo/
|   `-- config.toml - Cargo configuration for the workspace.
|-- .github/
|   `-- workflows/
|       `-- ci.yml - GitHub Actions workflow for Nx CI, install, format, lint, test, build, and fix-ci.
|-- .vscode/
|   `-- extensions.json - Recommended VS Code extensions for the workspace.
|-- packages/
|   |-- .gitkeep - Keeps the packages directory present when empty.
|   |-- spectacular/ - GPUI desktop application crate.
|   |   |-- Cargo.toml - Application crate manifest with GPUI, config, agent, and LLM dependencies.
|   |   |-- project.json - Nx project definition for build, test, lint, and run targets.
|   |   |-- assets/
|   |   |   |-- fonts/
|   |   |   |   `-- UbuntoSansMono/
|   |   |   |       |-- UbuntuSansMono-Italic-VariableFont_wght.ttf - Bundled italic monospace font asset.
|   |   |   |       `-- UbuntuSansMono-VariableFont_wght.ttf - Bundled regular monospace font asset.
|   |   |   |-- footer/
|   |   |   |   |-- branch.svg - Footer branch-status icon.
|   |   |   |   `-- config.svg - Footer configuration icon.
|   |   |   `-- window-controls/
|   |   |       |-- close.svg - Close window control icon.
|   |   |       |-- maximize.svg - Maximize window control icon.
|   |   |       |-- minimize.svg - Minimize window control icon.
|   |   |       `-- restore.svg - Restore window control icon.
|   |   `-- src/
|   |       |-- main.rs - Application entry point; resolves project directory args and launches the GUI.
|   |       `-- gui/
|   |           |-- fake_agent.rs - Produces fake agent events for the current shell prototype.
|   |           |-- mod.rs - GUI module root; binds actions, loads assets/fonts, configures the GPUI window.
|   |           |-- prompt_input.rs - Custom multiline prompt input with selection, cursor, clipboard, and scrolling behavior.
|   |           |-- shell.rs - Main Spectacular shell UI: header, transcript, sticky prompt, footer, timer, branch status, and fake work loop.
|   |           |-- theme.rs - Theme token structs and loading/fallback logic for JSON themes.
|   |           |-- transcript.rs - Converts agent events into user and assistant transcript groups.
|   |           `-- window_controls.rs - Cross-platform custom close/minimize/maximize controls and window-drag helpers.
|   |-- spectacular-agent/ - Agent orchestration library crate.
|   |   |-- Cargo.toml - Agent crate manifest with JSON schema, Tokio, and LLM crate dependencies.
|   |   |-- project.json - Nx project definition for Rust check, test, and lint targets.
|   |   |-- examples/
|   |   |   |-- cancellation.rs - Example for cancelling an active agent run.
|   |   |   |-- context_filtering.rs - Example for converting stored events into provider context.
|   |   |   |-- error_paths.rs - Example for provider and agent error handling.
|   |   |   |-- no_tool_run.rs - Example for a basic agent run without tools.
|   |   |   |-- queued_runs.rs - Example for queued agent run behavior.
|   |   |   |-- structured_output.rs - Example for JSON-schema-backed response validation.
|   |   |   `-- tool_loop.rs - Example for provider-requested tool execution loops.
|   |   `-- src/
|   |       |-- agent.rs - Core Agent implementation: run queue integration, provider streaming, tool loops, validation, cancellation, and tests.
|   |       |-- context.rs - Builds provider messages from stored events and validates provider context limits.
|   |       |-- error.rs - Agent error enum and mapping from provider errors.
|   |       |-- event.rs - Agent event model for prompts, deltas, metadata, tools, validation, errors, cancellation, and finish reasons.
|   |       |-- lib.rs - Public module declarations and re-exports for the agent crate.
|   |       |-- queue.rs - FIFO run queue with manual queueing, concurrent waiters, and cancellation handling.
|   |       |-- schema.rs - JSON schema wrapper for validating structured assistant responses.
|   |       |-- store.rs - Append-only event store with checkpoints and rollback.
|   |       `-- tool.rs - Tool trait, registry, execution, error formatting, and provider-visible tool call formatting.
|   |-- spectacular-config/ - Configuration persistence crate.
|   |   |-- Cargo.toml - Config crate manifest with Serde dependencies.
|   |   |-- project.json - Nx project definition for Rust check, test, and lint targets.
|   |   `-- src/
|   |       `-- lib.rs - Config path resolution, read/write/repair logic, provider API key storage, task model validation, and tests.
|   |-- spectacular-llms/ - LLM provider abstraction crate.
|   |   |-- Cargo.toml - LLM crate manifest with reqwest, Serde, and JSON dependencies.
|   |   |-- project.json - Nx project definition for Rust check, test, and lint targets.
|   |   |-- examples/
|   |   |   `-- provider_capabilities.rs - Example for inspecting enabled provider capabilities.
|   |   `-- src/
|   |       `-- lib.rs - Provider registry, model metadata, chat/message types, capabilities, cancellation, OpenRouter validation/model fetching, and provider errors.
|   |-- spectacular-plan/ - Placeholder planning route crate.
|   |   |-- Cargo.toml - Plan crate manifest depending on `spectacular-config`.
|   |   |-- project.json - Nx project definition for Rust check, test, and lint targets.
|   |   `-- src/
|   |       `-- lib.rs - Validates non-empty plan prompts and complete config before returning placeholder output.
|-- themes/
|   `-- default.json - Default GUI theme token file.
|-- .gitignore - Ignore rules for generated outputs, dependency directories, editor files, Nx cache, and local journals.
|-- .prettierignore - Files excluded from Prettier formatting.
|-- .prettierrc - Prettier configuration.
|-- Cargo.lock - Locked Rust dependency graph for reproducible builds.
|-- Cargo.toml - Root Cargo workspace manifest and release profile.
|-- README.md - Repository overview, included Nx/Rust/Python tooling, agent skill catalog, and everyday commands.
|-- nx.json - Nx workspace configuration, plugins, named inputs, and disabled cloud analytics.
|-- opencode.json - Local opencode configuration file.
|-- package-lock.json - Locked npm dependency graph for reproducible installs.
|-- package.json - npm workspace manifest and Nx/TypeScript/Rust/Python dev dependencies.
|-- tsconfig.base.json - Root TypeScript compiler options shared by workspace projects.
`-- tsconfig.json - Root TypeScript project reference container.
```

## Working Notes For Agents

- Prefer Nx targets when available: `npx nx <target> <project>`, for example `npx nx test spectacular-agent`.
- Rust crates are members of the root Cargo workspace, so `cargo test -p <crate>` is also a useful direct verification path.
- The current GUI shell uses fake agent events; real agent-provider integration lives in `spectacular-agent` and `spectacular-llms`.
- The repo has local agent skills under `.agents/skills`; load the relevant skill before following a specialized workflow.
- Do not treat ignored `.journal` files as normal source unless the task explicitly asks for journal planning or effort execution.
