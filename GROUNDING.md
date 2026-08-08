# Doric Grounding

Last reviewed: 2026-08-08

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
tools. `packages/bundle` owns strict manifest validation and runtime loading.
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
factory accepts an injected provider, logger, default and reranker model IDs,
explicit candidate and selected-skill limits, a required non-negative integer
localized-revision limit, a required positive safe-integer per-node model-turn
limit, bundle skills and executable tools, and both retrievers; it has no
session option. Doric configures eight model turns per node and three localized
revisions. The tool retriever remains part of this composition contract, but
the bundle state does not query it or run an independent tool router. Model
graph output owns only `id`,
`goal`, `doneWhen`, `dependsOn`, and `deliver`; Mosaic deterministically assigns
array-order indices, pending status, empty runtime-owned `candidates`, `tools`,
and `artifacts` arrays, plus null `bundle`, `outcome`, and `termination` fields. Plans
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
requests, over a run-local LIFO graph array where `graphs[n]` is plan revision
`n`. The `plan` state rejects every re-entry after P1. P0 is
catalog-independent and P1 is exactly one body-aware revision. Candidate limits
apply as both hint and retrieval bounds, including a defensive post-filter hint
bound over canonical, unique, non-required skills. Bundle routing uses the original request, current
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
are rejected. When executable tools are present, the agent exposes that
decision schema as its reserved terminal tool rather than combining the tools
with provider-native structured output. The terminal structured-output tool is
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
observations. Completed nodes store their Markdown result as a `text/markdown`
artifact, append additional artifacts, and return the resolved wave to
scheduling. Model-authored `blocked` and `failed` decisions also resolve their
wave normally. When every node is terminal, `delivery` performs stable
topological assembly using original node-array position as its tie-breaker and
finishes with `MosaicResult`.
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
receives `callId`. The successful local revision count is derived from graph
history, and retired IDs are the IDs found in older snapshots but absent from
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
MOSAIC 0.2 sections 4.7-4.9 and the `NodeContext`, `NodeDecision`,
`Observation`, runtime `NodeOutcome`, and `RevisionRequest` contracts in
Appendix A. The paper defines the semantic lifecycle but leaves tool-calling
protocol and dispatch mechanics open; concurrent ready waves,
`Promise.allSettled`, one isolated agent per node, terminal structured-output
tools, explicit criterion proof entries, and `text/markdown` artifact promotion
are Doric runtime choices.
Planning and revision append a graph, the last graph is active, and
scheduling mutates that graph when marking nodes ready. It succeeds for an
already-terminal graph and otherwise preserves the intentional missing-ready
domain failure when progress cannot continue.

`models/skillrouter-embedding` is the user-approved Nx/uv conversion utility
for the pinned SkillRouter checkpoint. Only its export target may fetch
upstream model bytes. Its `artifact/` directory and ONNX sidecars are ignored,
non-committed generated output; the converter source, pinned revision, and
`uv.lock` provide provenance. It remains a standalone utility. Doric generates
embeddings through LM Studio's local OpenAI-compatible
`http://localhost:1234/v1/embeddings` endpoint using the
`text-embedding-qwen3-embedding-0.6b` model. Doric fails when that endpoint is
unavailable, rejects the request, or returns no embedding.

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

`agents/doric` receives first-message config and later config replacements from
`requestContext.userMessage.metadata.configuration`. Sandbox creation, Git
setup, repository cloning, and initial workdir state are tied to session
creation; later config replacement updates only the stored session config.
GitHub repository config may include an optional `github.repo.branch` string,
which is used only when initially cloning a sandbox repository.

`packages/sandpool` owns process-local `SandboxSession` capacity, FIFO leasing,
background warming, replacement, and disposal. It accepts an injected sandbox
factory, depends only on the public `sandbox` contract at runtime, never reuses
released sessions, and has no provider-specific creation policy or persistence.
Its capacity option is `maxSandboxes`, and a lease guards SSH access exactly as
it guards other operations. `packages/sandbox` owns the provider-neutral
`SandboxProvider` and `SandboxRuntime` boundary plus workspace, Git, file, diff,
network-policy normalization, and disposed-session behavior. Every sandbox has
explicit CPU, memory, and writable-layer disk resources; networking is disabled
by default, optional SSH is key-only and loopback-bound by default, and effective
egress requires IP-literal DNS plus protected-destination filtering.

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
Use a quota-capable Docker host when disk isolation is required. Effective
Docker egress requires a local Unix daemon, host-network-namespace access,
nftables `CAP_NET_ADMIN`, and
provider-owned rules keyed to the inspected container address. Both providers
block new sandbox-to-host traffic and protected public-egress destinations,
with only exact private CIDR/protocol/port exceptions. SSH is disabled by
default, uses per-sandbox Ed25519 user and host keys, is key-only, and binds to
loopback unless an advertised remote binding is explicit.

`agents/doric/.Dockerfile` reproducibly builds the agent, pinned Firecracker and
jailer, Linux 6.18 guest kernel, static BusyBox and Dropbear bootstrap,
initramfs, OCI/ext4 tooling, networking tools, and OpenSSH client. Its Linux-only
Compose profiles provide either the Docker socket plus host-network firewall
access or KVM/TUN/cgroup/state/cache access without a Docker socket. The
privileged Firecracker profile is a development and e2e harness, not a
production isolation boundary. Doric constructs one selected provider for its
pool, acquires one pool lease per prompt, binds bundle tools to it, releases it
after success or failure, and disposes the pool on shutdown.

Doric initializes an otherwise unconfigured Express application and attaches
a Socket.IO server to the same HTTP listener. The listener binds to
`DORIC_HOST` and `DORIC_PORT`, defaulting to `0.0.0.0:3000`; the Doric image
exposes port 3000. Its modular Express router exposes `GET /vms`, which returns
the IDs and selected provider names of runtimes successfully provisioned by
this Doric process and not yet successfully disposed. The registry wraps the
provider at the composition boundary and does not expose keys, networking, or
provider internals. No other HTTP routes, middleware, Socket.IO events, or
connection handlers are defined.

Sandpool, each repository-owned LLM provider, and Victor require an injected
`pino.Logger` and create their own component child logger. They emit only safe,
structured `debug` operation logs: lifecycle and allowlisted counts/identifiers,
never prompts, model inputs or outputs, stored values, vectors, credentials,
URLs, headers, diagnostics, causes, or thrown values. A provider request with
`flags.sensitiveOutput` emits no provider operational logs. Composition roots
pass their existing logger to these dependencies; private OKF provider calls
use a disabled Pino logger.

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
The OpenAI, OpenRouter, and LM Studio OpenAI-compatible integrations support
single-text embeddings through their OpenAI-compatible `/embeddings` endpoint
and text-document reranking through `/rerank` below the configured base URL.
Embedding requests may include optional positive-integer `dimensions`, which
the compatible providers forward unchanged to the endpoint.
Rerank requests carry a model, query, non-empty document list, and optional
positive `topN`; successful results expose each original document index and
finite relevance score. Codex and LM Studio native support neither embeddings
nor reranking.
Provider configs may include an optional `baseUrl` string to
override provider endpoints that support it. Model configs may include an
optional provider-neutral `effort` value of `none`, `minimal`, `low`,
`medium`, `high`, or `xhigh`; legacy model `reasoning` remains supported as
the same effort alias. Config parsing rejects models that provide conflicting
`effort` and `reasoning` values. Provider requests may include top-level
`effort`, which takes precedence over legacy `flags.reasoning.effort`.
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
The agent runtime does not send a provider request that combines executable
tools with a native structured-output schema. When an agent run requests both,
it appends one collision-free strict terminal tool derived from the output
schema, adds a deterministic system instruction naming that tool, and omits the
native schema from provider requests in that run. Ordinary tool calls continue
through the existing loop. The terminal tool call must be the only call in its
response; the agent parses and validates its JSON arguments with the original
schema, converts it to a tool-free structured finish, and never executes it or
stores a tool result for it. Missing, malformed, schema-invalid, duplicate, or
mixed terminal submissions are repairable. The agent discards each invalid
response without persisting it or executing any included call, then may make
three correction attempts after the initial invalid submission. Each next
request receives one transient system correction naming the terminal tool,
explaining the failure, and including at most ten normalized Zod issue paths
and messages when available; rejected arguments are never copied into the
correction. The retry budget is cumulative across the run, and ordinary tool
turns neither consume nor reset it. The fourth invalid submission throws the
latest `invalid_structured_output` `AgentErrorObject`, whose existing
`diagnostic` field contains the same safe validation details when available.
Runs with a schema and no executable tools retain provider-native structured
output unchanged. This terminal behavior applies equally to complete and
stream agent runs. Streaming preserves already-emitted provider deltas,
suppresses an invalid `response.finished`, and adds no repair-specific public
event.
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
