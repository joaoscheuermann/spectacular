# Doric Grounding

Last reviewed: 2026-07-13

This is Doric's repository validity contract. Every agent working in this
repository must read it before non-trivial planning, reviewing, artifact
generation, architecture discussion, code editing, or workflow execution.

## Authority

Use this authority order:

1. System and runtime safety instructions.
2. Hard Constraints in this document.
3. Explicit user-approved constraints for the current task.
4. Current repository code, manifests, tests, and generated contracts.
5. Project agent instructions and task-specific skills.
6. The current task prompt.
7. Convention Parameters in this document.
8. Agent preference or local judgment.

Hard Constraints are gates. If a prompt conflicts with one, stop, cite the HC
ID, explain the conflict, and ask for user direction only when the constraint
allows a scoped human decision.

Convention Parameters are defaults. Follow them unless the task has a clear
reason to do otherwise, and record meaningful deviations where useful.

## Product Direction

Doric is a TypeScript-first agent-core project.

Current product work should focus on the agent runtime and its direct support
surfaces:

- prompt, message, and context handling;
- model/provider abstraction;
- tool definition, invocation, and result handling;
- agent loop control and structured events;
- error, cancellation, retry, and minimal state interfaces when justified.

The repository is not currently grounded in the former Rust CLI, daemon,
worker, lifecycle, TUI, slash-command, service, or multi-process architecture.
Those terms are out-of-scope markers unless the user explicitly expands the
product scope and current files support the change.

`agents/doric` hosts the A2A protocol scaffold for the Doric agent, including
Doric-owned JSON-RPC session management methods under the `doric/*` namespace.
Standard A2A JSON-RPC methods remain delegated to the A2A SDK transport
handler.

`apps/cli` is the explicitly requested Node.js command-line host surface for
interacting with the Doric A2A agent. The CLI is scoped to A2A message
submission, session listing, session replay/connection, and best-effort session
kill behavior. This does not by itself reintroduce the former Rust CLI, daemon,
worker, lifecycle service, TUI, slash-command, or multi-process architecture.

`packages/okf` is the explicitly requested embeddable TypeScript library for
generating local Open Knowledge Format bundles. Its public `generate` API
accepts an injected provider/model configuration, a repository root, and an
optional output path. The default output is
`<root>/.agents/bundles/project`; every explicit output must remain below
`<root>/.agents/bundles`, including after resolving existing symbolic links or
junctions. The generator discovers a complete regular-file snapshot, respects
scoped target `.gitignore` rules, excludes symbolic links and binary content,
and processes readable text in configurable concurrent batches.
Production OKF retains raw source bodies for SHA-256 cache identity while
redacting slash-delimited regex literals from model input. It preserves the
classification, frontmatter, and classification-routed analysis stages, using
the caller's provider instead of a package-selected provider. Complete
classification and frontmatter system prompts live under
`packages/okf/prompts/<stage>/<target>/SYSTEM_PROMPT.md`; analysis has one
complete prompt per supported file kind under
`packages/okf/prompts/analyze/<kind>/<target>/SYSTEM_PROMPT.md`. The
prompt-evolution CLI can evolve each prompt independently, and callers select
a common prompt target. Generated concepts mirror source paths without using
reserved `index.md` or `log.md` concept names, and one deterministic project
index is written to the bundle root.

`tools/okf` owns the provider-neutral, read-only `okf_search` tool for consuming
bundles below `<workspace>/.agents/bundles`. It performs bounded deterministic
lexical search over permissively parsed OKF frontmatter and Markdown bodies,
tolerates unknown producer fields and concept types, skips malformed concepts
and reserved index/log files, does not follow symbolic links, and does not
write files or access the network. The tool is a standalone package and is not
registered with `workflow-prompt` by this scope.

`apps/evolution` is the explicitly requested Node.js prompt-evolution CLI. Its
installed root command is `evolution`, with `init [directory]` and
`evolve <config-path> [--dry-run]` subcommands. Init resolves its directory from
the caller's current working directory, defaults to `.`, and non-destructively
ensures a real `scenarios` directory plus a credential-free default
`evolution.config.json`; it does not create a default prompt. Relative evolve
config paths resolve from the caller's current working directory, allowing
invocation from anywhere. The config file's parent directory is the workspace
root for all input and output: manually authored scenario definitions under
`scenarios`, exactly one immutable original `SYSTEM_PROMPT.md` or
`SYSTEM_PROMPT.txt` under `default`, and model-specific prompts and append-only
`evolution.history.jsonl` files under each target model ID. The config, default
prompt, and scenario definitions are read-only during evolution. The CLI
composes configured target models, one optimizer, and one binary judge through
any provider integration exported by `packages/llms`: OpenAI, OpenRouter, LM
Studio native, LM Studio OpenAI compatibility, or Codex.
Non-secret provider and model settings live in the passed JSON config;
credential values are resolved only at runtime from configured
environment-variable names and are never persisted or logged. Each target
model evolves independently from the original prompt; target failures do not
prevent other targets from running. Scenarios carry an explicit training or
validation split and combine config-level binary assertions with optional
scenario-local assertions. An incomplete suite, including a missing split or
a scenario without an effective assertion, fails before provider calls; the
CLI never generates, accepts, snapshots, merges, or writes scenarios.

For each scenario, the target is sampled exactly three times and one structured
judge call evaluates every sample against every effective assertion. Scenario
accuracy covers its complete result matrix, and overall accuracy is the mean of
scenario accuracies so scenarios are equally weighted. Only training scenarios,
training failures, and matching bounded history inform optimization. Normal
optimizer calls use temperature `0.2`; after the configured
`evolution.patience.epochs` unsuccessful normal epochs, one `0.8`
plateau-escape attempt runs. Codex optimizer requests
omit unsupported temperature and emit one stderr warning. Candidates are
accepted only on strict training improvement, subject to a hard epoch cap.

After training reaches the configured accuracy, one `0.2` compression attempt
may replace the prompt only when it is 20–30 percent shorter by trimmed
character count and still meets training accuracy. The selected prompt is then
evaluated once against the isolated validation split; validation inputs,
outputs, reasoning, and failures are never exposed to optimization or
compression. Prompt files are written only for approved targets, and existing
prompts survive failed runs. Applied runs append fingerprinted attempt and
terminal records to each target's history; only the newest configured number of
records matching the original prompt, training contract, global assertions,
accuracy threshold, target, and judge are reused. A dry run may read matching
history but makes no filesystem writes. Runtime progress is rendered through
`pino`/`pino-pretty` on stderr with credential redaction and without prompt,
scenario, model-output, judge-reasoning, assertion, strategy, or failure bodies,
preserving stdout for the final JSON result.

`packages/prompt-kit` owns Doric's command-line prompt abstraction for the
Node.js CLI host. It provides Doric-owned text, select, and queued prompt APIs
instead of coupling CLI user-input handling to Inquirer-shaped contracts.

CLI streamed A2A event output is visible console rendering through
`pino`/`pino-pretty`. Redaction must be applied to message text and structured
fields before events are handed to the logger, and this rendering is not
durable structured log storage.

Prompt open-question artifacts carry concrete selectable solution options.
Doric input-required A2A events expose those options directly instead of
synthesizing a recommendation-only choice.

`agents/doric` receives first-message config and later config replacements from
`requestContext.userMessage.metadata.configuration`. Sandbox creation, Git
setup, repository cloning, and initial workdir state are tied to session
creation; later config replacement updates only the stored session config.
GitHub repository config may include an optional `github.repo.branch` string,
which is used only when initially cloning a sandbox repository.

Doric supports OpenAI, OpenRouter, LM Studio native, LM Studio OpenAI
compatibility, and Codex as separate provider integrations. LM Studio native
uses its native REST API at `http://localhost:1234` by default. LM Studio
OpenAI compatibility uses the OpenAI-compatible API at
`http://localhost:1234/v1` by default and sends structured-output requests
through chat completions `response_format` rather than OpenAI Responses
`text.format` when no tools are present. When tools and structured output are
both requested, LM Studio OpenAI compatibility rejects the request with a
provider error before sending HTTP because LM Studio rejects `tools` and
`response_format` together.
Provider configs may include an optional `baseUrl` string to
override provider endpoints that support it. Model configs may include an
optional provider-neutral `effort` value of `none`, `minimal`, `low`,
`medium`, `high`, or `xhigh`; legacy model `reasoning` remains supported as
the same effort alias. Config parsing rejects models that provide conflicting
`effort` and `reasoning` values. Provider requests may include top-level
`effort`, which takes precedence over legacy `flags.reasoning.effort`.
OpenAI, Codex, and OpenRouter send resolved effort through `reasoning.effort`;
LM Studio OpenAI compatibility sends `reasoning_effort`; LM Studio native sends
its native `reasoning` value with `none` mapped to `off`, `minimal` to `low`,
and `xhigh` to `high`. The Codex provider uses the existing provider token
field for the Codex authorization value and derives Codex-compatible account
headers from that credential when available. OpenAI and LM Studio provider
configs may omit or blank the token for compatible local endpoints; in that
case the auth header is omitted. Codex and OpenRouter provider configs still
require configured tokens. Credential values must remain private runtime
inputs: do not persist, log, or echo resolved tokens.

The Codex OAuth profile is a package-owned default for the Codex browser
authorization flow. Host scripts should require only `CODEX_AUTHORIZATION` for
normal runtime use and let the OAuth package provide Codex's client id,
localhost callback route, connector scopes, and authorize-request parameters
unless an explicit override is needed for testing or a provider change. Codex
model requests use ChatGPT's Codex Responses backend with the ChatGPT access
token; they do not request the public OpenAI `api.responses.write` OAuth
scope. The ChatGPT Codex backend requires streaming requests with
`store: false` and non-empty instructions; non-streaming provider calls should
be fulfilled by consuming the streaming backend response. Codex requests must
not forward unsupported public Responses API controls such as `temperature`.

## Repository Shape

Doric is an Nx-managed TypeScript workspace with npm workspaces for
`apps/*`, `agents/*`, `packages/*`, `tools/*`, and `workflows/*`.

Use current manifests and source as the package inventory. Do not treat this
file as the source of truth for every package responsibility.

Durable boundaries:

- product packages live under `packages/*`;
- application host surfaces live under `apps/*`;
- agent entry surfaces live under `agents/*`;
- standalone tool packages live under `tools/*`;
- workflow packages live under `workflows/*`;
- package APIs should be exported through public entrypoints;
- sibling packages should not deep-import another package's private source.

Before adding a package or boundary, identify the responsibility that makes it
deeper than a folder: a stable public contract, separate test boundary,
dependency-direction boundary, or proven second consumer.

## Runtime Boundaries

The target runtime is an embeddable TypeScript agent core usable from tests and
future host surfaces without coupling the agent to a specific CLI, daemon,
worker, UI, provider, or process model.

Keep provider integration behind TypeScript interfaces. Do not make one
provider the only runtime path unless the task is intentionally a narrow first
slice.

Keep tool behavior behind explicit, typed, testable interfaces. Do not add host
command execution, network access, filesystem mutation, credential handling,
persistence, or session behavior without explicit scope and validation.

Credentials and secrets must not be persisted, printed, logged, or committed.
Prefer dependency injection and explicit configuration objects for sensitive
runtime inputs.

Doric runtime session replay state is process-local and in-memory. It records
context IDs, latest task IDs, initial prompt text, visible task state, ordered
A2A event history, and live subscribers for the custom
`doric/sessions/list`, `doric/sessions/connect`, and `doric/sessions/kill`
JSON-RPC methods. This replay state is not durable persistence. Session kill is
best-effort: it cancels active A2A tasks when the SDK/runtime can do so,
disposes known or in-flight sandbox sessions, removes in-memory session and
runtime replay state, and suppresses late events from the killed context in the
current process.

## Hard Constraints

### HC-001 Grounding Is Mandatory Context

Agents must read this document before non-trivial planning, reviewing,
artifact generation, architecture discussion, code editing, or workflow
execution.

Enforcement: if this document cannot be read, stop and report that the
repository grounding contract cannot be satisfied.

### HC-002 Current Repository State Is Authoritative

Current code, manifests, tests, and generated contracts beat memory,
assumptions, stale documentation, older summaries, and generic model knowledge.

Enforcement: verify current files before making architecture claims or changing
architecture-relevant behavior. Mark uncertain or future behavior as uncertain.

### HC-003 Keep Scope To The TypeScript Agent

New product work belongs in the TypeScript agent core unless the user
explicitly expands the scope.

Enforcement: do not add or design CLI, daemon, worker, lifecycle service, TUI,
slash-command, Rust product packages, or multi-process behavior without
explicit user approval for that scope.

### HC-004 Respect Minimal Boundaries

Do not add package splits, host/runtime boundaries, provider coupling, tool
privileges, persistence layers, or process orchestration before they are needed
by the agent core.

Enforcement: inspect current manifests and source before changing package
layout, public APIs, provider composition, tool registration, event streaming,
session persistence, config schema, or runtime behavior.

### HC-005 Preserve Unrelated Worktree Changes

Do not revert, overwrite, delete, stage, or commit unrelated user, maintainer,
or sub-agent changes.

Enforcement: inspect status before edits and commits. Keep write scope small.
Use destructive recovery only with explicit user approval and verified target
paths.

### HC-006 Validate Before Claiming Completion

Do not claim code, docs, workflows, or generated artifacts are complete or
working without appropriate evidence.

Enforcement: run relevant checks or state clearly why they could not be run and
what risk remains.

### HC-007 Protect Sensitive And Boundary Surfaces

Do not weaken security, privacy, permission, sandbox, command-execution,
network, authentication, secret-storage, config, or session boundaries without
explicit approval and validation.

Enforcement: stop for review before expanding command execution, network
access, credential handling, repository mutation, or persisted config/session
behavior.

### HC-008 Generated, Vendored, Lock, And Build Artifacts Need Provenance

Do not casually hand-edit generated contracts, vendored code, lockfiles,
schemas, or build outputs.

Enforcement: use the repository-approved generation, formatting, or validation
path when one exists, and explain why the artifact changed.

### HC-009 Human Gates Must Be Explicit

When scope, product tradeoffs, safety exceptions, destructive actions, missing
approval, or validation substitutions require human judgment, ask the user and
record the answer where useful.

Enforcement: do not treat silence, model confidence, or relative ranking as
user approval.

## Convention Parameters

| ID     | Convention                                     | Default                                                                    | Deviation rule                                                            |
| ------ | ---------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| CP-001 | Prefer the agent-only direction.               | Keep product work focused on the TypeScript agent core.                    | Explain why the task needs a larger product surface.                      |
| CP-002 | Prefer narrow, evidence-backed changes.        | Edit only files needed for the task and validate the touched behavior.     | Explain why a broader change is required.                                 |
| CP-003 | Prefer TypeScript and Nx-native commands.      | Use configured TypeScript, Nx, and formatter commands before substitutes.  | Explain tool absence, sandbox limits, or why a fallback proves the claim. |
| CP-004 | Prefer small interfaces over early frameworks. | Add minimal TypeScript contracts that can be tested directly.              | Explain why a larger abstraction is justified now.                        |
| CP-005 | Prefer progressive discovery.                  | Read only the grounding, instructions, manifests, and source needed.       | Broaden search when the task crosses boundaries or evidence is missing.   |
| CP-006 | Prefer durable provenance.                     | Name changed files, validation commands, decisions, and generated outputs. | Explain why provenance cannot be recorded.                                |
| CP-007 | Prefer implementation over chat-only advice.   | Land requested repository deliverables in files and verify them.           | Explain any blocker that prevents file changes.                           |
| CP-008 | Prefer Nx-managed package boundaries.          | Use Nx-visible workspace packages and public package entrypoints.          | Explain why a folder, manual scaffold, or direct source import is safer.  |
