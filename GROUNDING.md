# Doric Grounding

Last reviewed: 2026-08-09

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

Repository-owned executable bundles live as individual Nx packages immediately
below `/bundles`; each bundle owns its package metadata, TypeScript build, and
isolated output below `agents/doric/dist/bundles/<name>`. `/bundles/core` owns
the built-in `edit`, `find`, `grep`, `terminal`, `tree`, `web`, and `write`
tools. `/bundles/git` owns the routable `git` tool plus focused skills for
cloning, commit preparation, conflict resolution, rebasing, remote
synchronization, and linked worktrees. The Git tool executes structured argv
directly without shell interpretation, forces non-interactive Git behavior,
bounds stdout and stderr, and exposes no dedicated credential input.
`packages/bundle` owns strict manifest validation and runtime loading.
Doric loads only immediate bundle directories, in lexical order, from its
built `dist/bundles` artifact. Manifests explicitly order every resource and
carry `alwaysAvailable` flags for tools and skills. Runtime tools are compiled
ESM `.js` default exports created through `packages/tool`; each export is an
inspectable `ToolFactory` that Doric binds to a sandbox in its composition
root. Runtime TypeScript is rejected. Skill `allowed-tools` references resolve
only within their declaring bundle. Bundle, skill, and tool-factory names are
globally unique, and duplicates are rejected rather than aliased or
deduplicated. `packages/bundle` publicly owns the JSON-Schema-compatible
`SkillSchema`, whose input is normalized into a canonical `SkillRecord` with
trimmed fields, stable unique `allowedTools`, and a recalculated `indexText`.
Doric indexes that canonical text directly. `packages/tool` publicly owns the
JSON-Schema-compatible `ToolDefinitionSchema`, whose runtime descriptors carry
both `inputSchema` and `outputSchema`, plus strict-output-compatible
`ToolMetadataSchema` and JSON value schemas used by structured consumers.
Executable tool factories expose Zod `input` and `output` schemas; handler
results are output-validated before execution resolves, with sanitized
`invalid_output` failures. Providers transmit only their supported tool fields
and use `inputSchema` as function parameters. Model-generated graph nodes use
tool metadata rather than executable tools or arbitrary tool input schemas.

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
and processes every readable text file in configurable concurrent batches.
OKF derives type from the lowercase final extension and strictly validates TS,
TSX, JS, JSX, and JSON with the official Tree-sitter binding and grammars.
TS/JS concepts include deterministically sorted imports, resolved relative
targets, exports/re-exports, CommonJS relationships, and public members of
exported classes; JSON is syntax-validation-only, while unsupported extensions
remain ordinary text. Supported parsing uses a Tree-sitter buffer derived from
the JavaScript UTF-16 source length plus one, with a 32 KiB minimum rounded up
to the next power of two and bounded by the parser's unsigned 32-bit limit.
Parser buffer calculation, construction, language setup, and parsing failures
become a curated source-scoped `OKF_SOURCE_PARSE_FAILED` error at the `parse`
stage, while a parsed tree with syntax errors remains
`OKF_SOURCE_SYNTAX_INVALID` at the `syntax` stage. Each cache miss makes exactly three sequential
temperature-zero, tool-free plain-text completions using
`prompts/summarize/<target>/SYSTEM_PROMPT.md`,
`prompts/describe/<target>/SYSTEM_PROMPT.md`, and
`prompts/tags/<target>/SYSTEM_PROMPT.md`. The first request is
collision-safe Markdown containing Path, Type, an extracted Module Interface
when available, and the exact raw source with UTF-8 byte-length and
terminal-newline framing, so readable source is intentionally sent unchanged
to the caller-configured provider. Valid JSON uses a labeled `json` fence; all
other content uses `text`. Its non-empty trimmed Markdown analysis must not
begin with YAML frontmatter. The second and third calls each receive only that
analysis under `# Source Summary` in a collision-safe Markdown fence, without
path, type, interface, or raw content. The second prompt guides the model to
return one plain-text sentence of 1 to 240 characters ending in punctuation,
but runtime acceptance requires only the non-empty trimmed text guaranteed by
the completion boundary. Every such description is preserved without
normalization, truncation, repair, or restrictions on length, punctuation,
sentence count, or internal newlines. The third accepts
comma- or newline-separated plain text with an optional leading `Tags:`, a JSON
string array, an ordinary JSON object whose only own key is a `tags` string
array, or one complete matching Markdown fence containing one of those forms.
Plain items may have one whitespace-delimited bullet or decimal-list prefix
and one matching pair of single quotes, double quotes, or backticks. The prompt
guides the model toward concise tags, but runtime acceptance permits every
non-empty trimmed response. Recognized items preserve case, punctuation,
order, duplicates, and count after structural cleanup. Multiple non-empty
plain-text lines split by line; a single line splits by comma only with an
explicit `Tags:` label or when every comma candidate lacks sentence
punctuation. Malformed or unsupported JSON, non-string values, extra object
keys, mismatched or partial fences, content outside a fence, and recognized
forms with no remaining items fall back to the original trimmed response as
one tag. No call requests structured output or schema injection. All calls are
flagged as sensitive output, without
logging, response excerpts, or diagnostic/cause propagation. Generated
concepts persist the first result as analysis with the later description and
tags in deterministic YAML metadata plus Markdown. YAML preserves the complete
trimmed description, including internal newlines; the deterministic project
index folds whitespace only in its one-line tree entry and does not alter
concept metadata. The recipe includes normalized source identity, extension
type, sorted relationships,
and a recipe-aware SHA-256 covering source, relationships, parser/extractor,
concept-schema, YAML and plain-text validator versions (with
`plain-text-fields-v4` invalidating earlier description and tag-validation
caches), an explicit three-stage pipeline version, a versioned
next-power-of-two parser-buffer policy, all three exact prompts, provider ID,
model, and effort.
Cache hits make zero provider calls and parse
YAML for an exact hash match and still strictly parse supported source files.
Concepts mirror source paths without using reserved `index.md` or `log.md`
names, and one deterministic project index is written to the bundle root. The
public generator accepts an optional provider-neutral progress observer for
generation, per-file inspection, cache outcome, summary, description, and tags
stage start/completion, concept completion, index, and
final-summary events. Concurrent file events may interleave; each file is
identified only by its validated, normalized repository-relative source path,
and terminal file events include the synchronously updated processed count.
Host applications own stderr rendering; the root runner uses
`pino`/`pino-pretty`. Progress and failure logs exclude absolute root, output,
and filesystem paths; source bodies; prompts; provider/model identity;
responses; credentials; diagnostics; causes; thrown values; and
error names, messages, and stacks.
Known OKF operational failures use package-constructed, publicly inspectable
but non-constructible `OkfError` details: a fixed message, closed code and
stage, fixed hint, and, only for source-scoped stages, the validated normalized
repository-relative source. The exported `isOkfError` package-authenticity
guard is the sole trust check for logging; `instanceof`, prototypes, and field
shape are not authorization. Representative syntax,
parse, summary, description, and tags failures identify their safe stage and source
without retaining provider, parser, or filesystem diagnostics. Sanitized `AbortError`
values and progress-observer exceptions preserve their behavior and identity;
unknown failures may escape for hosts to replace with fixed output. Neither
the error nor host logging may include absolute paths, provider or filesystem
diagnostics, source bodies, prompts, responses, credentials, or caught
messages, names, stacks, codes, or causes.

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
any raw provider integration exported by `packages/llms`: OpenAI, OpenRouter,
LM Studio native, LM Studio OpenAI compatibility, or Codex. The unified
OpenRouter provider is a Doric composition policy rather than an Evolution
config variant.
Non-secret provider and model settings live in the passed JSON config;
credential values are resolved only at runtime from configured
environment-variable names and are never persisted or logged. Each target
model evolves independently from the original prompt; target failures do not
prevent other targets from running. Scenarios carry an explicit training or
validation split and combine config-level binary assertions with optional
scenario-local assertions. An incomplete suite, including a missing split or
a scenario without an effective assertion, fails before provider calls; the
CLI never generates, accepts, snapshots, merges, or writes scenarios.

For each scenario, the target is sampled with exactly three independent text
calls. Every effective assertion/sample pair is evaluated by its own structured
judge call, which returns one binary verdict; scenario accuracy covers all
pair verdicts, and overall accuracy is the mean of scenario accuracies so
scenarios are equally weighted. Evaluation defaults to one complete scenario
at a time and one global judge request at a time. Positive-integer concurrency
settings may raise either sliding limit; each active scenario still starts its
three target samples concurrently, while one evaluation-wide judge pool caps
judgments across all active scenarios. Scheduling settings do not change the
history fingerprint. Only training scenarios, training failures, and matching
bounded history inform optimization. Normal
optimizer calls use temperature `0.2`; after the configured
`evolution.patience.epochs` unsuccessful normal epochs, one `0.8`
plateau-escape attempt runs. Codex optimizer requests
omit unsupported temperature and emit one stderr warning. Candidates are
accepted only on strict training improvement, subject to a hard epoch cap.

Application-generated optimizer, compression, and judge user messages are
deterministic Markdown documents. Every authored or historical text value is
preserved inside a collision-safe Markdown fence. Optimizer messages exclude
application-injected target identity entirely: target ID, provider, and model
remain available for routing, progress, history, and fingerprints but are not
sent to the optimizer. Raw target scenario inputs remain unchanged, and all
structured optimizer and judge responses remain schema-validated JSON.

After training reaches the configured accuracy, one `0.2` compression attempt
may replace the prompt only when it is 20–30 percent shorter by trimmed
character count and still meets training accuracy. The selected prompt is then
evaluated once against the isolated validation split; validation inputs,
outputs, reasoning, and failures are never exposed to optimization or
compression. Prompt files are written only for approved targets, and existing
prompts survive failed runs. Applied runs append fingerprinted attempt and
terminal records to each target's history; only the newest configured number of
records matching the versioned evaluation mode, original prompt, training
contract, global assertions, accuracy threshold, target, and judge are reused.
A dry run may read matching history but makes no filesystem writes. Runtime
progress is rendered through `pino`/`pino-pretty` on stderr with credential
redaction and without prompt, scenario, model-output, judge-reasoning,
assertion, strategy, or failure bodies, preserving stdout for the final JSON
result.

`packages/prompt-kit` owns Doric's command-line prompt abstraction for the
Node.js CLI host. It provides Doric-owned text, select, and queued prompt APIs
instead of coupling CLI user-input handling to Inquirer-shaped contracts.

`packages/victor` owns Node.js-compatible, process-local in-memory retrieval.
It exposes incremental lexical and vector indexes plus a read-only hybrid
search. The lexical index applies deterministic Unicode tokenization and BM25;
the vector index accepts caller-injected embedding generation and ranks by
cosine similarity. Both indexes store caller values through per-add text
transformation. Hybrid search queries its injected lexical and semantic sources
concurrently, fuses their bounded ranks with equal-weight reciprocal rank fusion
using a fixed rank constant of 60, deduplicates by a caller-provided canonical
key, and resolves fused-score ties by that key. It has no persistence or
provider integration.

`packages/state-machine` owns reusable, process-local typed transition
execution. A definition stores only its exhaustive handler map and infers its
available handler names from that map. Each run keeps its context, current
handler, and current state local and resolves with a finished, domain-failed,
or engine-error result. Every handler receives the same state object type but
explicitly supplies the next state to a transition. A transition accepts only
an available handler name and passes that handler a shallow copy of the
supplied state object. It owns no workflow policy, persistence, listeners,
recovery hooks, or external side effects.

`packages/mosaic` depends on `packages/state-machine` and owns the Doric
goal-workflow policy: planning and revision, scheduling, skill retrieval and
reranking, and per-node skill/tool menu composition. Its public
factory accepts injected planning, localized-revision, execution, and reranker
providers, a logger, independent planning, localized revision, and execution
model profiles with explicit reasoning effort, plus reranker and embedder model
IDs without reasoning effort,
independent required positive safe-integer hint and execution candidate limits,
a selected-skill limit, a required non-negative integer
localized-revision limit, a required positive safe-integer per-node model-turn
limit, bundle skills and executable tools, and both retrievers. Run options may
carry a caller-owned UUID run ID and cancellation signal. Doric configures 32
model turns per node and three localized revisions. The tool retriever remains
part of this composition contract, but
the bundle state does not query it or run an independent tool router. Model
graph output owns only `id`,
`goal`, `doneWhen`, `dependsOn`, and `deliver`; Mosaic deterministically assigns
array-order indices, pending status, empty runtime-owned `candidates`, `tools`,
and `artifacts` arrays, plus null `bundle`, `outcome`, and `termination` fields.
Each materialized graph also receives a runtime-owned safe non-negative integer
`revision`: P0 is `0`, P1 is `1`, and every localized revision increments it
contiguously. The model-facing planning schema does not expose this field. Plans
require non-empty nodes, IDs, goals, and criteria;
unique existing dependencies; acyclicity; and at least one terminal deliverable,
while every deliverable must be terminal. A routed node stores a complete
`SkillCandidate` trace with canonical name, exact finite reranker score,
contiguous one-based rank, and non-empty selector rationale for every accepted
or rejected candidate. Its non-null `OrderedBundle` stores the matching goal
ID, unique selected names in candidate order, and a non-empty global rationale.
`bundle: null` means the node has never passed routing; routed empty bundles are
non-null. Full skill definitions remain in the catalog and are resolved only
from bundle names when composing tools and the execution prompt.
`agents/doric` remains the composition root that loads bundles, constructs and
populates lexical and vector indexes for routable skills and executable tools,
wraps each pair in hybrid search, and invokes Mosaic. Mosaic runs the fixed
`plan(P0) -> plan(P1) -> schedule -> bundle -> execution -> schedule -> delivery -> finish(MosaicResult)`
lifecycle, with `schedule -> revision -> schedule` for localized runtime
requests, over a run-local ordered graph snapshot array. Each graph's explicit
`revision`, rather than its array position or array length, is authoritative;
the history must be contiguous and ordered. The `plan` state rejects every
re-entry after P1. P0 is catalog-independent and P1 is exactly one body-aware
revision. `routing.maxHintCandidates` implements `K_hint`, while the independent
`routing.maxRetrievedCandidates` implements `K_retrieve`; each is also applied
as a defensive post-filter bound over canonical, unique, non-required skills.
`routing.maxSkills` is bounded only by `maxRetrievedCandidates`. Bundle routing uses the original request, current
goal and completion criteria, and only transitive-ancestor artifacts. It
retrieves a bounded routable-skill candidate set, reranks complete skill bodies,
validates one node-bound structured evaluation for every candidate plus a
global selection rationale, and locally normalizes the result by descending
relevance score with canonical-name tie-breaking. The exact provider scores are
preserved. The selected bundle is unique, bounded, may be empty, and remains
observable in that order on the node and its public result. No additional model
call is made. No-candidate routing and a zero selected-skill limit materialize
deterministic empty traces; the zero limit bypasses retrieval, reranking, and
selection. The exact node tool menu is the stable union of base tools and tools
declared by selected skills, with duplicate names removed by first occurrence.
Always-available skills are excluded from hints and routing, do not count toward
the selected-skill limit, and are injected as universal execution instructions;
they may reference only base tools and do not expand the node tool menu.
Every model-authored structured object is validated locally against its complete
Zod contract before changing Mosaic state. P0, P1, each concurrent hint
candidate, bundle selection, and each localized revision create a fresh agent
with isolated empty in-memory message and tool storages, preserve the authored
system prompt and Markdown user input, and terminate through the agent-owned
reserved structured-output tool using the operation's direct object schema
without sending a provider-native schema. Invalid submissions cannot
mutate Mosaic state, are retained only for provider replay, and may receive
bounded diagnostic feedback for two correction attempts; the next invalid submission propagates
`invalid_structured_output` through the owning workflow state. The reserved
tool is never executed, registered in Mosaic, or materialized as an
`Observation`. Bundle selection preserves `sensitiveOutput`. Reranking remains
a direct provider operation, and provider or transport failures receive no
Mosaic infrastructure retry.
Execution creates one agent per ready node with isolated in-memory message
storage and executable tools resolved from the catalog. Each node makes one
`agent.complete` call with tools and a node-bound strict semantic-decision
schema plus the required Mosaic turn limit. One turn is one provider
invocation, including direct and terminal responses, tool-call responses, and
structured-output repair attempts. Tools requested on the final permitted turn
execute and their results are stored; exhaustion is raised before another
provider invocation. The model-facing decision owns only `status`, ordered criterion
evaluations, `result`, `revisionRequest`, and `reason`; criterion `evidence` is
model-authored prose, and the revision request owns only `goalId`,
`invalidatedAssumption`, and `requestedEffect`. Unknown legacy reference fields
are rejected. The agent exposes that decision schema as its reserved terminal
tool, whether or not executable tools are present, rather than using
provider-native structured output. The terminal structured-output tool is
never an observation. Its collision-safe Markdown execution context contains
the original request, current goal and ordered `doneWhen` criteria, only
transitive-ancestor artifacts, selected skill bodies in bundle order, and tool
names and descriptions without duplicating tool schemas.

After the model decision terminates as `completed`, `needs_revision`,
`blocked`, or `failed`, the runtime correlates every successfully returned
executable-tool call from the node's isolated history. It creates one ordered
`Observation` per tool result and materializes the internal node outcome from
the decision and the complete observation sequence. `callId` exists only in an
`Observation` for runtime correlation and is not exposed in execution or
revision prompts. Missing, duplicate, or uncorrelated call/result data is a
runtime failure. A completed decision is valid with zero, one, or multiple
observations. Completed nodes store their Markdown result as an inline
`text/markdown` artifact, append additional artifacts, and return the resolved wave to
scheduling. Model-authored `blocked` and `failed` decisions also resolve their
wave normally. When every node is terminal, `delivery` performs stable
topological assembly using original node-array position as its tie-breaker and
finishes with `MosaicResult`.
Artifacts use one strict public discriminated union: inline artifacts contain
`kind: 'inline'`, a normalized non-empty MIME type, and string data that may be
empty; reference artifacts contain `kind: 'reference'`, a normalized non-empty
MIME type, and a normalized non-empty opaque reference. Its provider-facing
JSON Schema represents the two strict variants with `anyOf`, avoiding the
unsupported `oneOf` keyword in terminal function parameters. The runtime preserves
artifact order through decisions, outcomes, graph snapshots, causal projection,
and delivery, and renders references as references rather than content. The
MOSAIC core only validates and transports opaque references; storage, resolution,
existence checks, persistence, and authorization policy remain out of scope.
Delivery does not make provider, model, skill, or tool calls. Its public parts
preserve Markdown exactly, expose copied additional artifacts and complete
runtime observations (including call IDs and inputs/outputs), and join part
Markdown only with `\n\n`. A `needs_revision` decision requires at least one observation,
must target the current node, and does not promote partial results: execution
stores its semantic request and every observation produced by the node, then
returns normally to scheduling. Turn exhaustion materializes every correlated
executable-tool observation already stored, adds a runtime-owned `turn_limit`
termination, marks the node blocked, promotes no result artifacts, and resolves
the wave normally. Provider, tool, schema, correlation, and state-machine
failures retain their original identity and reject the workflow. Execution
emits safe logs with node IDs, terminal statuses, and selected skill and tool names only, never prompts, decisions, outcomes,
reasons, tool payloads, or error details.
The run-local state contains only the ordered graph snapshots. Each node owns a
complete candidate trace, `OrderedBundle | null`, `NodeOutcome | null`, and
`RuntimeTermination | null`. Outstanding
revision work is selected from node outcomes in deterministic node-wave order;
within each node, observations retain tool-result order. The localized planner receives every
queued observation as fenced tool name, input, and output evidence and never
receives `callId`. The successful local revision count and next revision are
derived from the active graph's explicit `revision`; retired IDs are the IDs found in older snapshots but absent from
the active graph. Each localized pass is owned by the dedicated `revision`
state, skips catalog hints, preserves completed and other non-pending nodes
exactly, permits changes only to the target and pending nodes, clears routing,
outcome, termination, and partial results from a retained target, and prevents
retired ID reuse. The prior graph snapshot retains its complete
`needs_revision` outcome. Only successfully appended localized graphs count
against the configured limit; exhaustion preserves the outcome, adds a
runtime-owned `revision_limit` termination, and blocks the target without a
provider call. Descendants of blocked or failed dependencies become blocked
with direct dependency IDs, while independent branches continue. A completed
result includes `FinalDelivery`; blocked and failed results omit partial
delivery, with `failed` taking precedence. This behavior implements
MOSAIC 0.2 sections 4.8-4.10 and the `NodeContext`, `NodeDecision`,
`Observation`, runtime `NodeOutcome`, and `RevisionRequest` contracts in
Appendix A. The paper recommends a reserved terminal tool for provider-neutral
structured output while permitting other protocols that preserve complete
validation, atomicity, bounded feedback, and bounded recovery. Concurrent ready
waves, `Promise.allSettled`, the exact isolated-agent composition, explicit
criterion proof entries, and `text/markdown` artifact promotion are Doric
runtime choices.
Planning and revision append a graph, the last graph is active, and
scheduling mutates that graph when marking nodes ready. It succeeds for an
already-terminal graph and otherwise preserves the intentional missing-ready
domain failure when progress cannot continue.

Agent structured runs expose an optional awaited `onStructuredAttempt`
callback on `AgentRunOptions`. It receives only schema version, one-based
structured-submission attempt, runtime acceptance, whether bounded feedback
will be sent, and an optional safe validation diagnostic. It never receives
rejected arguments, model reasoning, thrown values, or raw causes. A callback
failure retains its identity and aborts both complete and stream runs before
further provider activity.

Agent runs validate an entire provider tool-call batch before any handler
executes. Invalid JSON, unknown tools, invalid payloads, and invalid terminal
submissions share a default two-repair budget, append `incomplete` tool results,
and may be observed through the safe `onToolCallRepair` counter callback.
Handler failures are never retried by this protocol repair path. Structured
runs require a terminal tool call at the runtime validation boundary and
disable parallel tool calls. They do not force provider `tool_choice`, because
reasoning models may support tools and reasoning without supporting forced tool
selection in the same request.

The public Mosaic result contract remains unchanged. `MosaicOptions.models`
separates planning, revision, and execution profiles and keeps reranking and
embedding as non-reasoning model IDs. Each
`MosaicAgent.prompt` additionally accepts run-local observation options with a
serial awaited observer, caller-supplied UUID run ID, cancellation signal, and
either `structure` capture, the default, or `io` capture. Version-two Mosaic
events carry one run ID and contiguous sequence; model events also carry the
configured provider ID. They cover lifecycle, planning, retrieval/reranking,
bundle/menu, scheduling waves,
node execution and tools, decisions and runtime observations, revision, and
delivery. Every delivered event is a detached deeply frozen copy. Observer
latency is excluded from reported durations; the first observer failure aborts
the run and suppresses later events without reverting effects already
executed. Structure capture contains only allowlisted identifiers, statuses,
counts, ranks, scores, and durations. IO capture may additionally contain
model-visible messages and tool definitions, model-emitted visible content,
and tool inputs and outputs; it excludes credentials, provider controls,
private reasoning, provider replay, encrypted reasoning content, usage
payloads, diagnostics from caught failures, and raw causes. Existing Mosaic
logs remain allowlisted structural projections. Safe `tool.repair` events carry
only stage identity and attempt counters.

`mosaic/evaluation` is the benchmark-only interception entrypoint over the
same validated engine. Its optional initial-plan, feedback-plan, skill-view,
retrieval, routing, menu, execution, and localized-revision hooks receive
detached readonly snapshots and may delegate through `next`. Hook results pass
the normal complete schemas and runtime invariants before state mutation. An
adapter with no hooks is behaviorally neutral. A feedback hook may return
`unchanged`; Mosaic then performs no hint or P1 provider call and materializes
revision 1 as a validated plan identical to P0.

`models/skillrouter-embedding` is the user-approved Nx/uv conversion utility
for the pinned SkillRouter checkpoint. Only its export target may fetch
upstream model bytes. Its `artifact/` directory and ONNX sidecars are ignored,
non-committed generated output; the converter source, pinned revision, and
`uv.lock` provide provenance. It remains a standalone utility. Doric's default
persisted configuration composes an OpenAI Responses-compatible OpenRouter
provider with `qwen/qwen3.7-flash` at `low` for
planning, `google/gemini-3.6-flash` at `low` for localized revision, and
`deepseek/deepseek-v4-flash-0731` at `low` for node execution. Its per-node
model-turn limit is 32. It uses
`voyageai/rerank-2.5-lite` for reranking and
`voyageai/voyage-4-large` for 2,048-dimensional embeddings. Doric fails when
the configured endpoint is unavailable, rejects the request, or returns no
embedding.

`benchmarks/mosaic` is the user-approved private Nx application for the minimal
public-benchmark comparison of MOSAIC. The former custom empirical study is
archived at the annotated tag and GitHub release
`mosaic-validation-v0.2-archive`; it is no longer an active repository surface.
The application exposes only Nx-managed build, run, typecheck, test, and
release targets. Its sole `.mjs` file is a generated standalone ACP bundle for
benchmark containers that do not contain this workspace or its dependencies.

The benchmark publishes two BenchFlow ACP agents: a direct agent and a MOSAIC
agent. Both receive the same task prompt, mounted task skills, terminal tool,
OpenRouter-backed OpenAI Completions-compatible proxy, model, and low reasoning
effort. Both compose the shared `createUnifiedProvider`; BenchFlow selects
OpenRouter with `openrouter/openai/gpt-5.6-luna` and resolves the host's
`OPENROUTER_API_KEY`, while the agents receive only the proxy endpoint, alias,
and ephemeral proxy credential. The adapter sends that alias to LiteLLM while
the unified provider uses the fixed original `openai/gpt-5.6-luna` identifier
for curated capability policy; it does not treat LiteLLM's compatibility-only
model listing as an OpenRouter capability catalog. The generated adapter owns
the fixed `low` effort for both arms and the campaign omits BenchFlow's ACP
reasoning-effort option because the external manifest contract cannot declare
its required config-option identifier. The persisted BenchFlow run
configuration must record that harness-owned effort as null. The MOSAIC arm
uses the ordinary public `mosaic` entrypoint and observes runs through
`MosaicRunOptions.observer` with `capture: 'io'`; it does not use benchmark
interception hooks from `mosaic/evaluation`. IO events may expose model-visible
content and terminal inputs and outputs to the benchmark trajectory, but never
private reasoning or credentials. The terminal is benchmark-only authority
inside the task sandbox and does not add command execution to a product package
or Doric host surface. Its strict input accepts an optional positive
`max_output_chars` bounded by the fixed 12 KiB combined stdout/stderr ceiling,
allowing the model to request a smaller capture without invalidating the tool
call. Loaded skill bodies identify their canonical mounted
directory so references to bundled scripts and supporting files resolve the
same way in both arms. ACP prompt failures write only an authentic provider
error code when available, or a fixed generic line otherwise; provider
messages, diagnostics, causes, and thrown values are never written.

The BenchFlow launcher transfers its ephemeral proxy credential through a
mode-0600 `OPENROUTER_API_KEY_FILE`, removes credential values from the Node
process environment, and the provider reads and unlinks that file while
constructing the ACP app, before any prompt or terminal exists. Terminal
subprocesses additionally drop credential-shaped environment names. The host's
actual `OPENROUTER_API_KEY` never enters the agent container. The generated
bundle, its published
checksum sidecar, and both launch manifests are fixed campaign inputs. Both
manifests pin the bundle SHA-256 literally and pin the official Node archive
SHA-256 per supported architecture; a paid campaign repeats the free preflight
and refuses any local, manifest, or published bundle hash mismatch.

SkillsBench v1.1, pinned to commit
`b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af`, is the primary benchmark.
A diagnostic SkillsBench `pilot` action runs a predeclared, varied ten-task
subset through both arms by using BenchFlow's repeated `--include` selection.
The fixed set is `data-to-d3`, `earthquake-phase-association`, `edit-pdf`,
`jax-computing-basics`, `organize-messy-files`,
`pptx-reference-formatting`, `sec-financial-report`,
`spring-boot-jakarta-migration`, `travel-planning`, and `xlsx-recover-data`.
The comparator requires that exact selection in the recorded run config.
Pilot evidence is diagnostic only: it does not replace the full 87-task
primary campaign and cannot satisfy the SkillsBench report gate for
Terminal-Bench.
The Nx benchmark host may resume an existing SkillsBench pilot after a host or
sandbox failure. Resume is not a new campaign action: it preserves the original
`pilot` action and `campaignId`, validates the direct-child `results/` path,
closed metadata, artifact digests, exact task selection, released ACP bundle,
and per-arm manifest, then reuses the existing BenchFlow `jobs/` directory.
BenchFlow may reuse scored rollouts and reruns unscored tasks; MOSAIC starts or
resumes only after Direct has no runtime errors. Resume requires explicit paid
confirmation, an exclusive campaign lock, and Docker capacity of at least 8
CPUs and 8 GiB. Before and after every returned arm attempt, duplicate and
incomplete rollout directories are preserved under `attempts/`, while `jobs/`
retains only the newest result per task; a successful arm additionally requires
all ten tasks. Resume never crosses adapter releases, so every rollout in a
campaign uses the same recorded bundle and manifests. The command mirrors
BenchFlow stdout and stderr plus explicit host stages to host stderr while
reserving stdout for the final JSON result. Host-only resume code is built into
a separate generated Nx dispatcher.
Terminal-Bench 2, pinned to
`2fd12b88aafdd04a52c298e3940bcb189f9766d6`, is a secondary confirmation only
after a valid SkillsBench comparison report, which is an explicit input to a
paid Terminal-Bench campaign. A comparison is valid only when both arms match
the closed benchmark source, task manifests, model, skill policy, and adapter
assets; their recorded BenchFlow run configuration and health artifacts must
match the campaign contract and their recorded hashes. Every task is scored
without runtime or verifier errors and has trusted positive usage and cost
telemetry. The single decision gate is a strict Pareto win: MOSAIC must have
both a higher mean public-benchmark reward and a lower total model cost than the
direct agent. Paid smoke, pilot, and full campaigns require explicit
`--yes-paid-run`; credentials remain runtime-only and are never persisted or
logged.

CLI streamed A2A event output is visible console rendering through
`pino`/`pino-pretty`. Redaction must be applied to message text and structured
fields before events are handed to the logger, and this rendering is not
durable structured log storage.

Prompt open-question artifacts carry concrete selectable solution options.
Doric input-required A2A events expose those options directly instead of
synthesizing a recommendation-only choice.

Doric evaluates decomposition hints for candidate skills concurrently. The
hints method retains its existing inputs and `Set<SkillExtraction>` output,
with successful extractions ordered by their candidate input order.

`agents/doric` receives complete singleton Mosaic configuration replacements
through `PUT /mosaic/config`. It persists only provider IDs, HTTP(S) base URLs,
credential environment-variable names ending in `_API_KEY`, model profiles,
and bounded routing/execution/revision settings. Credential values remain
process environment inputs. Each session created by `POST /mosaic/sessions`
captures the active configuration generation and an immutable JSONB snapshot;
later replacements affect only new sessions.

`packages/sandpool` owns process-local `SandboxSession` capacity, FIFO leasing,
background warming, bounded factory-attempt batches, replacement, and disposal.
It accepts an injected sandbox factory, depends only on the public `sandbox`
contract at runtime, never reuses released sessions, and has no
provider-specific creation policy or persistence. A batch rejects pending FIFO
acquisitions and heat waiters after `maxCreateAttempts` consecutive factory
failures; the default is three, and later demand starts a fresh batch so a
recovered provider can serve new work. Doric explicitly uses that three-attempt
limit, allowing its existing execution failure path to move affected sessions
from `queued` to `failed`. Its capacity option is `maxSandboxes`, and a lease
guards SSH access exactly as it guards other operations. `packages/sandbox` owns
the provider-neutral
`SandboxProvider` and `SandboxRuntime` boundary plus workspace, Git, file, diff,
network-policy normalization, and disposed-session behavior. Every sandbox has
explicit CPU, memory, and writable-layer disk resources; networking is disabled
by default, optional SSH is key-only and loopback-bound by default, and
effective egress requires IP-literal DNS. Doric explicitly provisions its agent
sandboxes from the multi-architecture `node:22-bookworm` image, which includes
Git, with the `1.1.1.1` DNS resolver so selected Git skills can reach public
remotes. `DORIC_SANDBOX_SSH=true` adds loopback-bound, dynamically allocated
user SSH access so a trusted same-host user can inspect the active session
sandbox. Native source runs default it off because provider SSH requires pinned
host assets; the Doric runtime image and Compose profiles default it on and
include those assets. Remote authentication remains runtime-provided and must
never be placed in model-visible tool arguments.

`packages/docker` and `packages/firecracker` depend inward on Sandbox and expose
providers. Docker is Doric's default; `DORIC_SANDBOX_PROVIDER=firecracker`
selects the direct Linux x86_64 Firecracker/KVM boundary. Firecracker has no
Docker dependency or socket access. Firecracker resolves anonymous public
Linux/amd64 OCI images directly with skopeo and umoci, converts validated
rootfs trees into immutable ext4 base disks, and retains them in a persistent
manifest-and-converter-keyed 20 GiB LRU cache. Each sandbox receives a separate
sparse ext4 writable OverlayFS disk and a jailed Firecracker 1.16.1 process,
private `/30` TAP network, provider-only management SSH channel, and optional
proxied user SSH access. Startup reconciliation and disposal own the jail,
process, disks, cache-use markers, TAP, nftables tables, proxy, and generated
keys transactionally.

Docker maps CPU, memory, and writable-layer disk resources to daemon limits.
It retries once without the writable-layer disk daemon limit only when the
local storage driver explicitly rejects that option, so that fallback leaves
the Docker writable layer unmetered; CPU and memory limits remain enforced.
Use a quota-capable Docker host when disk isolation is required. On Linux,
effective Docker egress requires a local Unix daemon,
host-network-namespace access, nftables `CAP_NET_ADMIN`, and provider-owned
rules keyed to the inspected container address. Linux Docker and Firecracker
block new sandbox-to-host traffic and protected public-egress destinations,
with only exact private CIDR/protocol/port exceptions. On native macOS and
Windows, the Docker provider deliberately skips custom firewall configuration
and uses Docker Desktop bridge/NAT egress directly for local development;
protected destinations that Linux blocks remain reachable there. Other host
platforms reject Docker egress. SSH is disabled by default, uses per-sandbox
Ed25519 user and host keys, is key-only, and binds to loopback unless an
advertised remote binding is explicit.

`agents/doric/.Dockerfile` reproducibly builds the agent and both built-in
bundles, pinned Firecracker and jailer, Linux 6.18 guest kernel, static BusyBox
and Dropbear bootstrap, initramfs, OCI/ext4 tooling, networking tools, and
OpenSSH client. Its Linux-only Compose profiles provide either the Docker
socket plus host-network firewall access or KVM/TUN/cgroup/state/cache access
without a Docker socket. The privileged Firecracker profile is a development
and e2e harness, not a production isolation boundary. Doric acquires one pool
lease per persisted Mosaic session, binds that session's bundle tools to its
sandbox, propagates cancellation through acquisition and provider calls,
releases the lease after success or failure, and disposes the pool on shutdown.

Doric initializes an otherwise unconfigured Express application and attaches
a Socket.IO server to the same HTTP listener. The listener binds to
`DORIC_HOST` and `DORIC_PORT`, defaulting to `0.0.0.0:3000`; the Doric image
exposes port 3000. Its modular Express router exposes `GET /vms`, which returns
the IDs and selected provider names of runtimes successfully provisioned by
this Doric process and not yet successfully disposed. `GET /vms/:id/ssh`
returns the selected provider, owning live session ID, and complete
`SandboxSshAccess` only while that VM is leased to an active Mosaic session;
idle, releasing, and disposed VMs never expose access. The registry wraps the
provider at the composition boundary and the session service owns the
process-local lease association. `POST /mosaic/sessions` includes a stable
session SSH subresource link while preserving asynchronous queued creation.
That subresource reports pending acquisition, returns the active VM and SSH
access, or reports unavailable or expired access after release. Private keys
remain ephemeral provider-managed sandbox state and HTTP response data;
provider disposal owns their key-file cleanup. They are never persisted in
Doric's database, logged, included in lists, or emitted through Socket.IO. SSH
HTTP responses prohibit caching. REST additionally owns configuration reads and
replacements under `/mosaic/config`, session creation and cursor listing,
ordered event replay with an optional exclusive `afterSequence`, idempotent
termination, and terminal-only deletion under `/mosaic/sessions`. REST event
replay returns the original stored Mosaic event objects and the last observed
sequence, prohibits caching, and is a point-in-time read rather than a live
subscription. Socket.IO namespace
`/mosaic` owns subscription, snapshot, incremental replay, live `mosaic:event`,
state update, and deletion notifications. The PostgreSQL database is the event
source of truth: each event and the session's contiguous last sequence are
committed together before live emission.

`agents/doric` owns its Prisma ORM 7 schema, generated client configuration,
and versioned PostgreSQL migrations. Production uses one adapter-pg Prisma
client per process and never applies migrations implicitly during HTTP startup.
PostgreSQL stores the singleton configuration, normalized provider/model rows,
session snapshots and outcomes, and ordered JSONB events. Startup marks every
non-terminal session failed with the sanitized `process_interrupted` code;
events remain replayable and explicit terminal deletion cascades to events.
The local Compose surface pins PostgreSQL 18.4, mounts its PostgreSQL-18 volume
at `/var/lib/postgresql`, runs migrations as a one-shot dependency, and starts
either Doric sandbox profile only after the database is healthy and migrations
complete.
Doric startup emits safe structured `info` logs for its listener and sandbox
selection, PostgreSQL client initialization, sandbox limits, bundle resource
counts, active configuration revision and model profiles, session
reconciliation, mounted interfaces, and listener readiness. Startup failures
identify only the active bootstrap stage; they do not retain or emit database
or provider URLs, credential environment names or values, prompts, caught
diagnostics, causes, or thrown values.
Mosaic configuration and session routes remain unauthenticated on the existing
`0.0.0.0` listener. Provider base URLs and credential environment names are
intentionally configurable through the open PUT, so deployments must keep this
listener on an isolated trusted network. Sessions have no automatic expiry.

Sandpool, each repository-owned LLM provider, and Victor require an injected
`pino.Logger` and create their own component child logger. They emit only safe,
structured `debug` operation logs: lifecycle and allowlisted counts/identifiers,
never prompts, model inputs or outputs, stored values, vectors, credentials,
URLs, headers, diagnostics, causes, or thrown values. A provider request with
`flags.sensitiveOutput` emits no provider operational logs. Composition roots
pass their existing logger to these dependencies; private OKF provider calls
use a disabled Pino logger.

Doric supports OpenAI, raw OpenRouter, unified OpenRouter, LM Studio native,
LM Studio OpenAI compatibility, and Codex as provider integrations. The unified
provider uses stable OpenRouter Chat Completions, a 15-minute live model
capability cache with stale-on-error fallback, and curated profiles for OpenAI,
Anthropic, Gemini, Gemma, DeepSeek, Kimi, Mistral, Qwen, Llama, xAI, GLM,
Cohere, and MiniMax. It maps tool choice and sequential controls, forwards only
already-compatible strict tool schemas, and preserves ordered opaque
`reasoning_details` for replay. It sends `parallel_tool_calls` only when the
live model catalog advertises that parameter; otherwise it omits the transport
control, reinforces sequential requests with a model-facing instruction, and
relies on the Agent's atomic tool-batch validation before execution. Direct
proxy compositions may provide one original upstream model identifier while
sending a different request model alias; those requests use the original's
curated profile and skip live capability discovery because an OpenAI-compatible
proxy model listing is not an OpenRouter capability catalog. Direct
tool-free schemas select advertised JSON Schema, JSON object mode, or a
deterministic schema prompt, then validate with the original Zod schema and
allow at most two correction attempts. Structured streams emit only after
buffered validation. Tools plus a direct provider schema and other
non-emulatable combinations fail explicitly before completion.
The opt-in paid unified-provider conformance runner reserves stdout for its
final JSON report, permits up to 1,024 output tokens per request, and emits Pino
progress to stderr. Failures identify the exact structured-output, tool-call,
or tool-replay stage and expose only the sanitized provider error fields already
retained by the LLM boundary. An empty non-sensitive structured response
diagnostic identifies its finish reason and available output/reasoning token
counts instead of returning an empty string.
LM Studio native
uses its native REST API at `http://localhost:1234` by default. LM Studio
OpenAI compatibility uses the OpenAI-compatible API at
`http://localhost:1234/v1` by default and sends structured-output requests
through chat completions `response_format` rather than OpenAI Responses
`text.format` when no tools are present. When tools and structured output are
both requested, LM Studio OpenAI compatibility rejects the request with a
provider error before sending HTTP because LM Studio rejects `tools` and
`response_format` together.
The OpenAI, OpenRouter, and LM Studio OpenAI-compatible integrations support
single-text embeddings through their OpenAI-compatible `/embeddings` endpoint
and text-document reranking through `/rerank` below the configured base URL.
Embedding requests may include optional positive-integer `dimensions`, which
the compatible providers forward unchanged to the endpoint.
Rerank requests carry a model, query, non-empty document list, and optional
positive `topN`; successful results expose each original document index and
finite relevance score. Codex and LM Studio native support neither embeddings
nor reranking.
`packages/llms` also exposes a generic OpenAI Responses-compatible factory with
caller-configured provider identity and base URL. Its `/responses`, `/models`,
`/embeddings`, and `/rerank` operations preserve that identity in metadata,
safe logs, stream events, and provider errors.
Provider configs may include an optional `baseUrl` string to
override provider endpoints that support it. Model configs may include an
optional provider-neutral `effort` value of `none`, `minimal`, `low`,
`medium`, `high`, or `xhigh`; legacy model `reasoning` remains supported as
the same effort alias. Config parsing rejects models that provide conflicting
`effort` and `reasoning` values. Provider requests may include top-level
`effort`, which takes precedence over legacy `flags.reasoning.effort`.
Provider requests may also control tool selection and parallel tool calls.
The OpenAI Responses adapter sends `store: false`, preserves every opaque
response output item for exact replay, forwards incomplete tool results, and
marks schemas strict only when their unmodified JSON Schema is already
strict-compatible. Replay and encrypted reasoning never enter provider
operational logs.
Provider requests may also set `flags.sensitiveOutput`; repository-owned
providers then suppress operational logs for the call and omit model-response
bodies, excerpts, diagnostics, and causes from error surfaces while preserving
the normal completion result.
Provider requests may set
`flags.includeStructuredSchemaOnSystemPrompt: true` together with `schema` to
append one deterministic, collision-safe Markdown system message containing
the converted JSON Schema. Authored system messages retain their order before
that generated message, followed by all non-system messages in their original
order. The flag is additive to provider-native structured-output fields;
omitting it, setting it to false, or using it without a schema leaves messages
unchanged. `flags.sensitiveOutput`, independently, controls repository-provider
diagnostic suppression for private calls.
Every `provider.complete` request with a schema resolves only after a JSON
response has been parsed and validated by that schema. Refusals, tool calls,
missing or invalid JSON, and schema-validation failures reject with
`invalid_structured_output`; successful structured completions always include
the validated `structured` value.
Every agent run with an output schema appends one collision-free strict
terminal tool derived from that schema, including when its caller-owned tool
storage has no executable definitions. The agent adds a deterministic system
instruction naming the terminal tool and never sends the native schema in its
provider requests. Ordinary executable tool calls continue through the
existing loop. The terminal tool call must be the only call in its response;
the agent parses and validates its JSON arguments with the original schema,
converts it to a tool-free structured finish, and never executes it or stores a
tool result for it. Missing, malformed, schema-invalid, duplicate, or mixed
terminal submissions are repairable. The agent stores each invalid response
for provider replay without executing any included call, then may make two
correction attempts after the initial invalid submission. Each next request
receives one transient system correction naming the terminal tool, explaining
the failure, directing the model to correct every issue without stringifying
objects or arrays, and including at most ten normalized Zod issues with their
field path, issue kind, expected type when available, and safe message;
rejected arguments are never copied into the correction. The
retry budget is cumulative across the run, and ordinary tool turns neither
consume nor reset it. The third invalid submission throws the latest
`invalid_structured_output` `AgentErrorObject`, whose existing `diagnostic`
field contains the same safe validation details when available. This terminal
behavior applies equally to complete and stream agent runs. Streaming
preserves already-emitted provider deltas, suppresses an invalid
`response.finished`, and adds no repair-specific public event.
Agent runs may declare an optional positive safe-integer `maxTurns`; omission
keeps the loop unbounded. Invalid values fail with `TypeError` before message
storage or provider activity. The budget is checked immediately before every
provider invocation and counts ordinary responses, tool-call responses, and
structured-output repair attempts identically in `complete` and `stream`.
Tools from the last permitted turn execute and their results are stored before
the next invocation is rejected with `turn_limit_exceeded`. Exhausted streams
preserve emitted events and do not emit `agent.finished`.
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
`apps/*`, `agents/*`, `packages/*`, `tools/*`, `workflows/*`, and
`benchmarks/*`.

Use current manifests and source as the package inventory. Do not treat this
file as the source of truth for every package responsibility.

Durable boundaries:

- product packages live under `packages/*`;
- application host surfaces live under `apps/*`;
- agent entry surfaces live under `agents/*`;
- standalone tool packages live under `tools/*`;
- workflow packages live under `workflows/*`;
- private evaluation instruments live under `benchmarks/*`;
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

Doric Mosaic session replay is durable in PostgreSQL. Sessions transition
through queued, running, cancelling, completed, failed, and cancelled states;
termination is best-effort and idempotent. One process-local map owns only live
abort controllers and Socket.IO subscribers. It is not the replay source of
truth, does not resume work after restart, and requires no distributed
Socket.IO adapter because Doric currently supports one host instance.

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

### HC-010 Keep Completion And Stream Inputs Human-Readable

Newly written or materially revised user-message inputs passed to model
`complete` or `stream` calls must not use raw or serialized JSON objects or
arrays as their outer prompt envelope.

Enforcement: write the outer prompt as simple, direct, unambiguous,
evidence-grounded Markdown. Literal JSON source material is allowed only in a
labeled fenced `json` block within that Markdown. This constraint does not
apply to provider transports, JSON-RPC, configuration, storage or persistence,
tools or tool payloads, schemas, or other non-prompt JSON.

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
| CP-009 | Author model prompts for people first.         | Use simple, direct, unambiguous, evidence-grounded Markdown.               | Explain why another prompt format is necessary.                           |
