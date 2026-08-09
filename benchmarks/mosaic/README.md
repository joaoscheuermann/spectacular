# MOSAIC empirical benchmark

`mosaic-benchmark` is the private, offline-first evaluation instrument for
`packages/mosaic`. It contains the frozen catalog, deterministic in-memory
world, condition executors, append-only traces, scoring, blind review, and R
analysis protocol. Doric-targeted smokes are opt-in harness contracts and never
enter the confirmatory analysis; they do not execute the dirty `agents/doric`
composition root.

> **Readiness status (2026-08-08): NO-GO for an official paid study.** The
> remaining validity, recovery, metering, case-authoring, and operational work
> is tracked in [READINESS.md](./READINESS.md). Passing the existing test suite
> or `--validate-config` does not by itself mean that the instrument is ready.
> Do not use `--yes-paid-study` until every P0 gate in that document is closed.

## Frozen runtime

- Primary model: `openai/gpt-5.6-luna`, effort `medium`.
- Reranker: `voyageai/rerank-2.5-lite`.
- Embedding: `voyageai/voyage-4-large`, 2,048 dimensions.
- Five paired repetitions.
- Sixty English pilot micro-skills and exactly 24 deterministic tools.

The production runner uses OpenRouter through `OPENROUTER_API_KEY`. The key is
read only from the process environment and is never accepted by a schema,
manifest, trace, or command argument. The model provider is wrapped so every
completion uses `medium` effort even if the public MOSAIC package has a
different local default.
The V1 study contract still owns one model profile per run. The runner maps
that same model and effort to MOSAIC planning, revision, and execution so the
new production API does not change the frozen experimental estimand.

## Build and gates

```sh
npx nx sync
npx nx show projects
npx nx build mosaic-benchmark
npx nx test mosaic-benchmark
npx nx run mosaic-benchmark:schemas
node benchmarks/mosaic/dist/src/cli.js validate
node benchmarks/mosaic/dist/src/cli.js conformance
```

`analysis-test` is intentionally opt-in because it requires the frozen R
environment. It never pulls or builds an image:

```sh
npx nx run mosaic-benchmark:analysis-test
```

## Automated computational study

After the human-authored inputs and immutable analysis image are ready, the
study orchestrator runs the complete computational path: gates, pilot,
calibration, power, freeze, primary/replication/sensitivity runs, scoring, and
the three R analyses. It pins the input bytes below the external study root and
resumes only stages that have a valid immutable receipt.

```sh
node benchmarks/mosaic/scripts/study.mjs --print-config > /tmp/study.json
# Fill every placeholder and point root outside the repository.
node benchmarks/mosaic/scripts/study.mjs --validate-config /tmp/study.json

export OPENROUTER_API_KEY='from-your-secret-manager'
node benchmarks/mosaic/scripts/study.mjs \
  --config /tmp/study.json \
  --yes-paid-study
```

Rerun the same final command after an interruption; the runner uses the stored
attempts and receipts rather than deleting work. The explicit paid flag is
mandatory because the pilot alone contains 1,800 model runs. Blinded human
review, optional failure-only oracles, and the audited publication package
remain separate post-study steps described in [STUDY.md](./STUDY.md).

## CLI

Every command emits exactly one JSON document to stdout. Progress and R output
go to stderr. Output artifacts use exclusive creation and are never replaced.

```text
mosaic-benchmark validate [--cases confirmatory-cases.json --n-final <N>] \
  [--schedule schedule.json] [--prices prices.json] \
  [--pilot-scores pilot-scores.json] [--calibration calibration.json] \
  [--power-config power-config.json --power-result power-result.json]
mosaic-benchmark conformance
mosaic-benchmark pilot --study-id <id> --seed <seed> [--output schedule.json]
mosaic-benchmark calibrate-models --input calibration-input.json \
  [--output calibration.json]
mosaic-benchmark power --config power-config.json \
  --image '<repository>@sha256:<digest>' --work-dir verification \
  --result power.json
mosaic-benchmark freeze --input freeze-input.json --path freeze.json \
  --pilot-scores pilot-scores.json --calibration calibration.json \
  --power-config power-config.json --power-result power-result.json \
  --cases confirmatory-cases.json --schedule confirmatory-schedule.json \
  --prices prices.json --image '<repository>@sha256:<digest>'
mosaic-benchmark run --schedule schedule.json --artifacts artifacts \
  --prices prices.json [--cases cases.json] [--freeze freeze.json] [--resume]
mosaic-benchmark score --schedule schedule.json --artifacts artifacts \
  --family exploratory|primary|replication|sensitivity \
  [--cases cases.json] [--freeze freeze.json] \
  [--output scores.json] [--csv scores.csv]
mosaic-benchmark review prepare --input review-plan.json \
  --assignments assignments.json --key private-key.json
mosaic-benchmark review ingest --input review-ingest.json [--output reviews.json]
mosaic-benchmark review status --input review-status.json
mosaic-benchmark analyze --scores scores.csv --config analysis-config.json \
  --freeze freeze.json --image '<repository>@sha256:<digest>' \
  --work-dir verification --result result.json [--report report.md]
mosaic-benchmark package --input package-plan.json
```

`validate` returns the canonical hashes for the local protocol, schemas,
catalog, tools, pilot cases, conditions, prompts, R implementation, and
`renv.lock`. With study artifacts supplied, it also validates and returns the
confirmatory-case, paired-seed, price, pilot-score, calibration, power-config,
and power-result hashes required by the freeze. `--power-config` and
`--power-result` are an inseparable pair: the first binds the simulation input
bytes and the second binds the published result bytes.

`calibrate-models` accepts one independent 60-case calibration corpus plus the
complete 180-row M1 score sets for Luna and the candidate. It verifies the
three paired repetitions, models, case metadata, canonical score provenance
fields, and separation from pilot families before deriving the bootstrap
observations; free-form success observations are not accepted.

The frozen price input has this shape. The numeric values below are
illustrative only: replace every value with the exact price captured from the
named source at freeze time. Zero or guessed prices are not acceptable study
data.

```json
{
  "schemaVersion": 1,
  "currency": "USD",
  "capturedAt": "2026-08-08T00:00:00.000Z",
  "source": "https://provider.example/pricing",
  "models": {
    "openai/gpt-5.6-luna": {
      "inputPerMillion": 1.25,
      "outputPerMillion": 10,
      "cachedInputPerMillion": 0.125,
      "perRequest": 0
    },
    "voyageai/voyage-4-large": {
      "perRequest": 0.001
    },
    "voyageai/rerank-2.5-lite": {
      "perRequest": 0.001
    }
  }
}
```

`run --resume` retries only a technical failure before the first model call.
Once any model call begins, an interruption is terminal and remains in the
dataset. Confirmatory and replication schedules additionally require a clean
worktree and a matching immutable freeze manifest. Use `--doric-smoke` only
for a schedule containing exclusively the six smoke cases.

`score` never accepts caller-authored assertions. It resolves the terminal
attempt, verifies the per-attempt hash chain and derived trace, evaluates the
frozen state/tool/delivery oracle, and then writes `ScoreRowV1`. Primary,
replication, and sensitivity scoring require the matching freeze.

`freeze` recomputes the complete 1,800-row pilot grid, the success/cost/ID
baseline tie-break, the p95 budget, calibration bootstrap, power provenance,
independent confirmatory corpus, complete paired schedule, prices, local
instrument hashes, OCI digest, and current Git commit. Its input must omit
`manifestHash`; the command computes it and creates the manifest exactly once.
The schedule supplied to `freeze` may still carry `freezeHash: null`; because
the seed ledger intentionally excludes that field, stamp the returned
`manifestHash` into the otherwise unchanged run schedule before `run`.

`power` and `analyze` invoke only the local OCI image named by an immutable
digest. Each command runs twice with no network and publishes a result only
after byte-identical outputs; the verification directory and checksum hash
remain in the command result.

## Study sequence

1. Run `validate` and `conformance`.
2. Generate and execute the 1,800-run pilot schedule (`60 × 6 × 5`).
3. Score all terminal records and freeze the best B0–B3 baseline by success,
   mean cost including failures, then lexical condition ID.
4. Run the 60-case, three-repetition M1 calibration for Luna and the candidate
   family. Freeze is blocked unless the paired bootstrap interval remains in
   `[-0.05, 0.05]`.
5. Run the 10,000-simulation power analysis, author at least 240 independent
   confirmatory families at the calculated 120-case boundary, validate the
   balanced 6-by-4 corpus and complete paired schedule, then write the freeze
   once from a clean recorded commit.
6. Execute primary and replication schedules separately. Run failure-only
   oracles only for failed parents.
7. Analyze each model family separately in the pinned container and verify two
   byte-identical results.

Costs are descriptive and never gate end-to-end success. No paid run is
started by build, test, validation, schema generation, or analysis targets.
