# MOSAIC E2E: separate acceptance criteria and completion judge

A small, single-request experiment using the existing `llms`, `victor`, `agent`,
`messages` and `tool` packages. No production MOSAIC state machine is involved.
The retrieval and P0-ablation labs supply the provider patterns and local skills.

The code follows the experiment's stages:

- `index.mjs`: command options, configuration and run lifecycle.
- `flow.mjs`: stage orchestration with explicit input contexts.
- `stages/prepare.mjs`: input loading, catalog composition and manifest creation.
- `stages/p0.mjs`: request-only draft goals.
- `stages/index.mjs`: shared catalog indexing and hybrid search with reranking.
- `stages/retrieval.mjs`: per-P0-goal retrieval and candidate union.
- `stages/gate.mjs`: global candidate keep/drop selection.
- `stages/p1.mjs`: fresh graph synthesis from the request and kept skills.
- `stages/criteria.mjs`: one criteria-generation call per node and topological ordering.
- `stages/schedule.mjs`: sequential node dispatch.
- `stages/route.mjs`: fresh node retrieval and keep/drop selection.
- `stages/execute.mjs`: executor session and bounded submission loop.
- `stages/judge.mjs`: independent completion judgment for one submission.
- `stages/feedback.mjs`: next executor message from judgment and observations.
- `stages/delivery.mjs`: accepted terminal results and execution metrics.
- `runtime.mjs`: provider calls, usage, agent sessions, core tool binding and traces.
- `environment.mjs`: Docker sandbox, input upload, workspace export and disposal.
- `validation.mjs`: shared membership checks and topological ordering.
- `context.mjs`: shared Markdown evidence rendering, without model instructions.

Every stage exports exactly one function taking an explicit context object and
starts with a purpose comment. Model instructions, private output schemas and stage-specific message
composition live in the corresponding stage file. Deterministic stages have no
model prompt. Source hashes in the manifest cover root modules and stage modules.

```text
request → P0 goals → hybrid retrieval + reranking → global keep/drop gate
        → P1 graph (request + kept bodies, no P0)
        → separate acceptance-criteria generation call for each node
        → sequential nodes in topological order:
            fresh retrieval + reranking + skill selection
            → executor with core tools → candidate result
            → independent completion judge with core tools for inspection
            → accept, feedback and continue, needs_revision, or blocked
        → assemble accepted terminal results
```

Criteria are generated sequentially in topological order, with the original request,
kept skill bodies, full P1 graph and current node supplied to each call. Each call
returns only that node's criteria and is traced as `criteria.<nodeId>`.

The judge runs after execution. Criteria are fixed across attempts. It judges
satisfaction of those criteria, not their quality. Every node needs at least
one criterion, and completion requires exactly one positive evaluation per
criterion. Each fresh judge receives only the current node ID, goal, fixed
criteria and candidate Markdown result in its user message. Its system prompt
contains evaluation instructions, the environment and always-available skills.
It receives no original request, ancestor results, executor observation ledger or
previous judgments. Observation references must come from that judge's own tool
results, resolved at validation time. This validates references, not semantic
proof; the judge independently inspects the shared workspace when needed.

The executor's initial user message is exactly the current node's goal. Its
system prompt contains execution instructions, the environment, selected skill
bodies and always-available skills. Judge feedback continues the executor's
existing conversation. Goals and criteria must carry the requirements needed
for their stage: neither executor nor judge receives the original request as
a fallback during execution.

## Run

From the repository root, with Docker running, dependencies installed and `OPENROUTER_API_KEY`
in `.env`:

```sh
# Build the runtime bundle (repeat after changing core tools or skills).
npx nx run bundle-core:build

# Free preparation: load core plus the local catalog and create the task workspace.
npm run llm:mosaic-e2e -- --prepare

# Paid execution of the bundled task in a fresh workspace.
npm run llm:mosaic-e2e

# Your own task, whose input files already exist in this directory.
npm run llm:mosaic-e2e -- --request /absolute/request.md --workspace /absolute/task

# Optional model overrides.
npm run llm:mosaic-e2e -- --model google/gemini-3.8-flash --execution-model openai/gpt-6-astra --criteria-model openai/gpt-5.6-sol --judge-model openai/gpt-5.6-sol
```

`--skills DIR` accepts a flat directory of Markdown skill bodies. The default
reads the catalog directly from `../skill-retrieval-gating/cases/skills`; gold
case labels are never loaded. The compiled `core` bundle is loaded from
`agents/doric/dist/bundles` through the public bundle loader. Only `core` is used;
other bundles are not exposed to the agents. Its 13 optional skills join the
37-skill default catalog for planning and node retrieval. Duplicate skill names
are rejected. Its always-available `goal-directed-tool-use` skill is included in
the P1 planning context with its complete body and injected into both executor
and judge instructions. Always-available skills bypass retrieval and selection.
Other settings are the small `config` object in
`index.mjs`: hybrid top 20, reranked top 10, two execution submissions per node,
20 model turns per agent call. P0, P1 and skill selection default to
`google/gemini-3.8-flash` with high reasoning effort. Node execution defaults to
`openai/gpt-6-astra` with low effort. Criteria use `openai/gpt-5.6-sol`
and completion judging on `openai/gpt-5.6-sol`, both with high effort.
`--model` controls planning and skill selection only; `--execution-model`,
`--criteria-model` and `--judge-model` independently override the other stages.

Each paid run creates one Docker container through `sandbox.createSandbox` and
the `docker` provider. The default image is `node:22-bookworm`, with one CPU,
512 MiB RAM and a requested 4096 MiB writable disk. Docker's provider may fall
back to an unmetered writable layer when the host does not support disk quotas.
The image is pulled only when missing; change `config.sandbox` for another
environment with `/bin/sh`, `timeout` and `tar` installed.

The runner copies regular files and directories from the local input workspace
into `/workspace`. Symlinks and special files are rejected. Nothing is bind-mounted,
and changes stay in the container. Executor and judge share this sandbox; every
terminal command starts a fresh shell there. Both executor and judge receive all
seven core tools: `edit`, `find`, `grep`, `terminal`, `tree`, `web`, and `write`.
The core terminal uses its existing compact output contract and a default
120-second timeout (configurable per call, up to 600 seconds).
Container networking is disabled and provider credentials stay in the host
process. The core `web` tool makes requests through the host, outside that
network restriction; task restrictions on external data still apply.
Models still run through the host's OpenRouter connection.
The judge is instructed to inspect without modifying files; that is a prompt
convention within the shared sandbox, not an enforced permission boundary.

Before removing the container, the runner exports `/workspace` into
`output/<uuid>/workspace.tar`, including when execution fails. Cleanup runs in
`finally`; forcibly killing the host process can still leave a container behind.
Custom input directories are not overwritten. Use workspace-relative paths in
your request. `--prepare` creates only local inputs and does not start Docker.

## Bundled task and evidence

For a harder shared task using the same container and tools, see
[Regional close](../e2e-scenarios/regional-close/README.md). It supplies identical
inputs for this runner and `direct-skills-e2e`, plus an external artifact scorer.

The default case recovers two missing CSV values and produces a Markdown report.
It needs no external data or extra dependencies. Inspect the generated files:
Q1 south must be 180, Q2 north 190; column sums must be north 510, south 640,
grand total 1150. `input.csv` must remain unchanged. These answers are documented
for manual checking and are not supplied to any model. The case exercises real
file creation and inspection; it does not guarantee the planner chooses multiple nodes.

Each invocation creates `output/<uuid>/`:

- `manifest.json`: configuration, request, complete skill snapshot, core tool definitions and skill flags, source hashes,
  workspace, timestamps and final status.
- `trace.jsonl`: stage inputs and outputs, per-call usage and tool observations,
  including partial progress if a later call fails.
- `stages/`: individual JSON stage results, written during execution with
  sequential filenames such as `000002-p0.json`. Each contains the same
  `at`, `stage` and `data` envelope as its JSONL entry. Repeated stages get
  separate files; stage names are sanitized for filenames. Inputs, usage and
  tool observations remain in `trace.jsonl` only.
- `results.json`: plans, executed nodes, fixed criteria, attempts, judgments,
  unexecuted node IDs and raw provider usage.
- `delivery.md`: accepted terminal results, only when the whole graph completed.
- `workspace/`: local source inputs for the bundled case.
- `workspace.tar`: final sandbox files, including generated artifacts. Extract
  into a new directory to inspect them (`mkdir artifacts && tar -xf workspace.tar -C artifacts`).

Logs show each provider call and completed stage. Usage records each successful
provider invocation, including intermediate executor/judge turns, embeddings
and reranks. Missing provider usage remains null. The manifest status distinguishes
`completed`, `attempt_limit`, `needs_revision`, `blocked` and operational `failed`.
Only `completed` exits zero, apart from `--prepare` and `--help`.

## Deliberate limits

There is one fresh P1 even when the gate keeps no skills, always without P0.
P0 retrieval uses request and goal, with no criteria. Hybrid retrieval uses
Victor's native fusion without the older lab's vector threshold. A single global
gate evaluates the union of candidates. Final node routing independently queries
the full catalog using request, goal and criteria. Neither executor nor judge
receives ancestor history; both can inspect the shared sandbox filesystem.

There are no infrastructure retries, resume, scheduling waves, automated plan
revision, baseline arms or statistical comparisons. The first unfinished node
stops the run; remaining nodes are reported as unexecuted. Agent protocol repairs
use the existing package behavior. Executor history and local observations survive
judge feedback; each judge invocation has a fresh context. Result acceptance is
the judge's assessment, not an independently measured task-success score.

This is a runnable architecture diagnostic, not evidence that separating criteria
or judging outperforms joint generation or self-evaluation. Follow-up comparisons
need fixed tasks, objective artifact checks and explicit baselines.
