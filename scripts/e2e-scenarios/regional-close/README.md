# Regional close: Direct versus MOSAIC

This synthetic task isolates data selection, causal dependencies and final
consistency using the runners' existing container and tools. It is inspired by
the demographic integration problem in SkillsBench's `sales-pivot-analysis`,
but uses original generated data and CSV/JSON outputs. It is not that benchmark,
and no historical model failure rate applies to this case.

The agent must reconcile 240 active regions across eight fictional states,
resolve aliases and revisions, reconstruct population cells, calculate national
income quartiles, and deliver mutually consistent aggregates and a complete
record audit. Repeated district names, leading-zero codes, provisional and late
records, inactive regions, tied incomes and zero earners make shortcuts visible.

Both agents receive exactly `request.md` and `inputs/`. The default common skill
catalog remains unchanged. There are no new tools, case-specific skills, Python
dependencies, Docker images or changes to either orchestration pipeline.

## Layout and provenance

- `request.md`: the complete public task contract and output schemas.
- `inputs/`: four generated CSV files copied into each isolated workspace.
- `generate.py`: deterministic standard-library-only fixture generator.
- `fixtures/expected.json`: private expected output derived from complete
  canonical facts, before corrupting and shuffling their source presentation.
- `fixtures/manifest.json`: hashes of inputs, request, generator and expected output.
- `score.py`: external artifact scorer; reads an exported tar in memory without
  extracting files or executing agent code. Requires only host Python 3.

Never pass this whole directory as the task workspace: the generator and private
references would reveal the answer. Pass only `inputs/`. The provided inputs and
references are frozen together; to intentionally regenerate them after a recipe
change, run `python3 scripts/e2e-scenarios/regional-close/generate.py` and review
the resulting changes. Do not regenerate between arms of a comparison.

## Prepare without provider calls

Run from the repository root, with the usual dependencies and compiled core
bundle available:

```sh
npm run llm:direct-skills-e2e -- --prepare \
  --request scripts/e2e-scenarios/regional-close/request.md \
  --workspace scripts/e2e-scenarios/regional-close/inputs

npm run llm:mosaic-e2e -- --prepare \
  --request scripts/e2e-scenarios/regional-close/request.md \
  --workspace scripts/e2e-scenarios/regional-close/inputs
```

This uses the existing `node:22-bookworm` sandbox configuration. Node's standard
library or the image's Python standard library suffices to solve the task. No
spreadsheet package or installation during execution is needed.

## Run the paired diagnostic

The following commands make paid provider calls. Each starts a fresh run and
prints its output directory. Models are explicit so default changes do not
silently alter the comparison.

```sh
npm run llm:direct-skills-e2e -- \
  --request scripts/e2e-scenarios/regional-close/request.md \
  --workspace scripts/e2e-scenarios/regional-close/inputs \
  --model deepseek/deepseek-v4.1-flash \
  --execution-model deepseek/deepseek-v4.1-flash

npm run llm:mosaic-e2e -- \
  --request scripts/e2e-scenarios/regional-close/request.md \
  --workspace scripts/e2e-scenarios/regional-close/inputs \
  --model deepseek/deepseek-v4.1-flash \
  --execution-model deepseek/deepseek-v4.1-flash \
  --criteria-model deepseek/deepseek-v4.1-flash \
  --judge-model deepseek/deepseek-v4.1-flash
```

## Score exported artifacts

Replace each placeholder below with the UUID printed by its runner:

```sh
python3 scripts/e2e-scenarios/regional-close/score.py \
  scripts/direct-skills-e2e/output/DIRECT_UUID

python3 scripts/e2e-scenarios/regional-close/score.py \
  scripts/mosaic-e2e/output/MOSAIC_UUID
```

The scorer accepts a standalone `workspace.tar` as well; then run metadata and
usage are unavailable. It prints JSON and exits **0** only when every required
output matches the frozen reference and all four input files are unchanged;
**1** denotes an incorrect or missing artifact, and **2** a scoring/input error.
Passing a run directory also requires its request to match the frozen request.

Checks cover every row and cell, exact selected source IDs, all exclusion
reasons, national quartile assignment, empty state/quartile groups, weighted
rounding, report totals, unique row keys and required source ordering. Formatting
of equivalent numeric values is not scored; JSON numeric values must be numbers.
Errors are bounded to five examples per file, with the complete error count.

## Reading the comparison

The primary endpoint is external `artifact_pass`, independent of either
agent's own `completed` status. Inspect individual failed files to locate
selection, integration, grouping or delivery errors. The scorer also reports
recorded provider calls, cost by provider unit, missing cost coverage, timestamps
and runtime configuration. Usage accounts for recorded successful calls, not
unreported billing or failed provider attempts.

Keep request, input hashes, catalog snapshots, tool definitions, source revision,
execution model/effort and sandbox resources identical across the pair. Save the
scorer outputs with the run evidence. The task is one fixed diagnostic, not a
statistical benchmark; repeat paired runs before drawing conclusions about
reliability, and report disagreements and ties as well as wins.

The Direct arm already retrieves skills initially and on demand. MOSAIC adds
planning, node routing, criteria and judging. Its per-node turn limits give it a
different total budget from Direct's 20 executor turns. This comparison measures
the complete configured systems, including their extra computation; it does not
isolate orchestration at equal tokens, calls, time or cost. A win must be read
alongside the extra latency and recorded cost. No outcome is assumed in advance.
