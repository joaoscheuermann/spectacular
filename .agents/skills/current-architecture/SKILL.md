---
name: current-architecture
description: Tracks and maintains the repository's current architecture. Use when answering architecture questions, changing package boundaries, adding providers/tools/commands/TUI flows, or updating architecture documentation.
---

# Current Architecture

Use this skill to keep the repository architecture accurate, discoverable, and synchronized with implementation changes.

## Source of truth

The current architecture overview lives at:

- `docs/architecture-and-packages.md`

Treat this document as the repo-level architecture map. It should describe observed implementation, not future intent.

Repository code and manifests are authoritative when they disagree with documentation. If the architecture document conflicts with implementation, verify against source, report the discrepancy, and update the smallest affected section when the task allows it.

## Success criteria

- **Accuracy:** Architecture notes reflect current code, package manifests, and public APIs.
- **Boundaries:** Package responsibilities and dependencies remain explicit.
- **Traceability:** Architecture updates point to concrete source paths, package names, or symbols.
- **Freshness:** Architecture documentation is updated when structural changes make it stale.
- **No speculation:** Future ideas, uncertain behavior, and unverified assumptions are not presented as current architecture.

## When to use this skill

Use this skill when the task involves any of these areas:

- Explaining repository architecture or package responsibilities.
- Changing crate/package dependencies or workspace membership.
- Adding, removing, or moving major modules.
- Adding a provider, built-in tool, slash command, TUI runtime behavior, config surface, or session format.
- Changing event flows between provider, agent, session, TUI, or tools.
- Updating `Cargo.toml`, package `project.json`, `nx.json`, or architecture-relevant docs.
- Reviewing whether a proposed change fits existing boundaries.

## Retrieval budget

Read only enough context to answer or update the architecture accurately:

1. Start with `docs/architecture-and-packages.md`.
2. Read package manifests only if package membership, dependencies, targets, or workspace shape matter.
3. Read source modules only for the boundary or flow being changed.
4. Stop reading once you can cite the affected package, module, responsibility, and dependency direction.

Do not scan the whole repository unless the requested change truly cuts across all architecture layers.

## Canonical architecture files

Use these files as the first verification points when their concern is relevant:

- `Cargo.toml`: Rust workspace membership, exclusions, release settings, and crate patches.
- `package.json`: Nx workspace metadata, JS tooling, and package workspaces.
- `nx.json`: Nx plugin and target inference configuration.
- `packages/*/Cargo.toml`: package dependencies and crate metadata.
- `packages/*/project.json`: package Nx targets.
- `packages/cli/src/chat/provider.rs`: application-level provider composition.
- `packages/cli/src/chat/runner.rs`: main agent and built-in tool assembly.
- `packages/cli/src/chat/commands/registry.rs`: chat slash-command registration.
- `packages/llms/src/registry.rs`: enabled provider metadata.
- `packages/tools/src/lib.rs`: built-in tool registration and exports.

Use this skill together with `coding-conventions` when proposing or changing architecture. This skill tracks current structure; `coding-conventions` supplies design standards for simplicity, boundaries, tests, and implementation quality.

## Maintenance rules

When a code change affects architecture, update `docs/architecture-and-packages.md` in the same task if the user's request allows documentation updates.

Update the architecture document when any of these change:

- Workspace members or package names.
- Internal package dependency direction.
- Package responsibility or ownership of a major concern.
- Provider registry or provider composition model.
- Built-in tool list or tool registration flow.
- Slash-command registration or command execution model.
- TUI state/action/reducer/runtime boundaries.
- Session persistence format or storage locations.
- Agent event, provider event, or tool-call flow.
- Config schema concepts or config file locations.
- Vendored dependency strategy.

Do not update the architecture document for purely local implementation details that do not affect package responsibilities, public extension points, or cross-package flows.

Do not track changes such as:

- Private helper functions.
- Small refactors contained inside one package boundary.
- Test-only helpers or fixtures.
- Formatting-only changes.
- Implementation details that do not affect public extension points or runtime flows.

## Architecture update process

When updating the architecture map:

1. Verify the current implementation from source, manifests, or existing docs.
2. Identify the smallest section of `docs/architecture-and-packages.md` that must change.
3. Edit only that section unless the change has multiple explicit architectural impacts.
4. Keep language factual and current-tense.
5. Preserve package boundaries and avoid adding speculative abstractions.
6. Mention discrepancies only as observed discrepancies, not resolved facts.

## High-level architecture

Use this high-level model as the starting point for architecture reasoning, then verify against code before making claims:

```text
User terminal
   |
   v
doric binary (`cli` package)
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

## Current boundary model

Use this as the default package ownership model, then verify against code before making claims:

- `cli` is the application composition root and builds the `doric` binary.
- `agent` owns agent runtime, event flow, context assembly, tool loop, queueing, retries, and cancellation.
- `llms` owns provider traits, provider request/stream types, provider registry, and provider implementations.
- `tools` owns built-in host/file/web tools that implement the agent `Tool` trait.
- `tui` owns UI state, actions, reducers, rendering, and IOCraft runtime glue.
- `commands` owns generic slash-command parsing, registration, metadata, and dispatch primitives.
- `config` owns persisted configuration schema, validation, provider credentials, and config IO.
- `packages/vendor/iocraft` is a vendored UI dependency patched through Cargo.

## Reporting expectations

When answering architecture questions:

- Prefer concise package-level explanations first.
- Include source paths when useful.
- Distinguish current implementation from future ideas.
- Mark unknowns as unknown instead of guessing.
- If the architecture document appears stale, say what is stale and update it when appropriate.

When reviewing architecture-impacting changes:

- State which package owns the concern.
- State whether the dependency direction remains valid.
- State whether `docs/architecture-and-packages.md` needs an update.
- Keep recommendations simple and aligned with existing package seams.

## Skill validation

When validating or maintaining this skill, run:

```sh
skills-ref validate .agents/skills/current-architecture
```
