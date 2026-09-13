# Direct execution with initial and on-demand skills

This independent diagnostic uses P0 only to improve initial skill retrieval.
One direct agent completes the entire request in one conversation. It receives
the original request and environment in the user message, and selected complete
skill bodies plus always-available core instructions in the system prompt.
It has all seven core tools and `search_skills`. It never receives P0.

```text
request → P0 → per-goal hybrid retrieval, reranking and keep/drop selection
        → union of approved skills, deduplicated by name
        → direct agent with initial skills
            ↔ core tools
            ↔ search_skills → hybrid retrieval → reranking → focused keep/drop
        → final Markdown response and workspace artifacts
```

There is no P1, execution graph, generated acceptance criteria or completion judge.
The direct agent reports `completed` or `blocked`; this is self-assessment, not
an independently verified success score.

## Run

From the repository root, with Docker running and `OPENROUTER_API_KEY` in `.env`:

```sh
npx nx run bundle-core:build
npm run llm:direct-skills-e2e -- --prepare
npm run llm:direct-skills-e2e
npm run llm:direct-skills-e2e -- --request /absolute/request.md --workspace /absolute/inputs
```

`--prepare` loads the compiled core bundle and catalog, copies the bundled input
and writes a manifest without Docker or provider calls. Normal execution makes
paid provider calls. `--model` configures P0 and selection; `--execution-model`
configures the direct agent. Both default to `deepseek/deepseek-v4.1-flash`, with
high effort for planning/selection and low effort for execution. `--skills DIR`
replaces the flat local Markdown catalog. Core optional skills are added to it;
always-available skills bypass retrieval and are injected into the direct system
prompt. The defaults use hybrid top 20, reranked top 10 and 20 executor turns.

The index is built once per run and reused by initial and on-demand retrieval.
Each P0 goal has an independent search and gate, recorded as `retrieval.p0.<n>`
and `gate.p0.<n>`. The initial bundle includes a skill if any goal approves it,
once per name in first-accepted order. A rejection for another goal does not
remove it. Empty candidate sets skip the gate model call. There is no global gate.
`search_skills` takes `query` and `context`, searches using the original request
and the current need, and selects candidates specifically for that need. It
returns names and complete bodies, including an empty list when appropriate.
Selection reasons remain in the trace. It neither executes skill instructions
nor expands the tool menu. Repeated queries may return previously supplied skills.
Each search may call embedding, reranking and a fresh selection agent; those
provider calls do not consume the direct agent's own turn budget.

## Outputs

For a harder shared task using the same container and tools, see
[Regional close](../e2e-scenarios/regional-close/README.md). It supplies identical
inputs for this runner and `mosaic-e2e`, plus an external artifact scorer.

Each invocation creates `output/<uuid>/`:

- `manifest.json`: configuration, request, complete catalog and core snapshot,
  experiment source hashes, environment and lifecycle status.
- `trace.jsonl`: incremental stage inputs/results, tool observations and provider usage.
- `stages/`: sequential individual JSON results, including repeated stages, with
  the same `at`, `stage`, `data` envelope. Inputs, usage and observations remain
  in JSONL only.
- `results.json`: P0, initial skill names, direct result, on-demand search summary,
  observations, metrics and provider usage when the flow returns.
- `delivery.md`: final Markdown only for a self-reported completed result.
- `workspace/`: bundled source input.
- `workspace.tar`: exported sandbox workspace before cleanup, including failures.

P0, initial retrieval, gate, execution and each on-demand search have separate
trace stages. Operational failures mark the manifest failed and preserve prior
trace entries; there are no infrastructure retries or resume. Forced process
termination can prevent export and lifecycle finalization.

One Docker sandbox owns `/workspace`, with inputs copied through the sandbox API,
no host bind mounts and container networking disabled. Core `web` uses the host
network; the agent must respect the request's restrictions on external data.
Credentials stay in the host. The sandbox image is `node:22-bookworm` with the
same resource configuration as mosaic-e2e. Custom input directories are not modified.

The bundled CSV case is copied from mosaic-e2e for comparison. Expected recoveries
are Q1 south=180 and Q2 north=190; sums are north=510, south=640 and total=1150.
`input.csv` must remain unchanged. Check actual exported artifacts; a completed
agent response alone does not establish correctness. This diagnostic establishes
neither a comparative benefit nor statistical superiority.
