# Paired local agent scenarios

Run the same frozen request and inputs with both agents, then score exported
files independently of runtime status.

| Scenario               | Challenge                                                   | Size                                          |
| ---------------------- | ----------------------------------------------------------- | --------------------------------------------- |
| `regional-close`       | Revisions, recovery and aggregate reconciliation            | 240 regions, 1,208 source records             |
| `inventory-replay`     | Bitemporal selection, FIFO, returns and atomic transfers    | 24 SKUs, 3 warehouses, 768 event rows         |
| `warehouse-allocation` | Nested kits, shared resources and exact global optimization | 12 days, 120 indivisible orders, 3 warehouses |

These are authored diagnostics, not established benchmarks. Their actual
difficulty and comparative success rates must be measured with repeated runs.

## Execute a pair

Build the core bundle, have Docker running and configure `OPENROUTER_API_KEY`
in `.env`. These commands make paid calls:

```sh
SCENARIO=inventory-replay
npm run llm:mosaic-e2e -- \
  --request "scripts/e2e-scenarios/$SCENARIO/request.md" \
  --workspace "scripts/e2e-scenarios/$SCENARIO/inputs" \
  --model deepseek/deepseek-v4.1-flash \
  --execution-model deepseek/deepseek-v4.1-flash \
  --criteria-model deepseek/deepseek-v4.1-flash \
  --judge-model deepseek/deepseek-v4.1-flash

npm run llm:direct-skills-e2e -- \
  --request "scripts/e2e-scenarios/$SCENARIO/request.md" \
  --workspace "scripts/e2e-scenarios/$SCENARIO/inputs" \
  --model deepseek/deepseek-v4.1-flash \
  --execution-model deepseek/deepseek-v4.1-flash
```

Use `SCENARIO=warehouse-allocation` for optimization. Add `--prepare` for free
preparation without Docker or provider calls. **Pass only `inputs/` as workspace**;
the scenario root contains private answers and generators.

## Score

```sh
python3 "scripts/e2e-scenarios/$SCENARIO/score.py" scripts/mosaic-e2e/output/MOSAIC_UUID
python3 "scripts/e2e-scenarios/$SCENARIO/score.py" scripts/direct-skills-e2e/output/DIRECT_UUID
```

Scorers read `workspace.tar` in memory without extraction or executing agent code.
Exit 0 means correct required artifacts and unchanged inputs; 1 means artifact
failure; 2 means invalid scoring inputs or fixture drift. Standalone tar paths
are accepted without runtime metadata. Scoring checks exact CSV headers, row
coverage, identifiers, numeric values and JSON fields/types, including duplicate
JSON keys. Auxiliary files outside required outputs are not scored.

Report `artifact_pass`, runtime status, latency, calls and provider-reported cost
separately. Correct artifacts with turn exhaustion differ from incorrect output.
MOSAIC has per-node and judge budgets, so this is not equal-compute testing.
Freeze request/input hashes, catalog, tool snapshots, models and source versions
across each pair. No runner defaults, tools or skills are changed by these cases.

## Provenance

Each scenario owns its request, generator, scorer, public inputs and private
fixtures. The two new scenarios share `artifacts.py` for persistence and scoring;
the regional scorer is unchanged. SHA-256 manifests bind the request, generator,
scorer, shared scorer, inputs and expected outputs. Only after an intentional
source change, regenerate and review the fixtures:

```sh
python3 scripts/e2e-scenarios/inventory-replay/generate.py
python3 scripts/e2e-scenarios/warehouse-allocation/generate.py
```

Never regenerate between comparison arms. Generation and scoring use only Python's
standard library and make no provider calls. Inventory references are authored
from closed-form allocation facts before adding revision noise and shuffling.
Allocation references enumerate every feasible assignment using independently
specified flattened kit requirements. No expected output comes from an agent.
