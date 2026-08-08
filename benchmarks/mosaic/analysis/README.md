# Offline analysis

For development only, generate the deterministic synthetic input and run the
focused R goldens in an already-pinned local R environment:

```sh
Rscript analysis/generate-synthetic.R /tmp/scores.csv
Rscript analysis/test-analysis.R
Rscript analysis/power.R analysis/fixtures/power-config.json /tmp/power.json
Rscript analysis/analyze.R /tmp/scores.csv analysis/fixtures/analysis-config.json /tmp/result.json
Rscript analysis/report.R /tmp/result.json /tmp/scores.csv /tmp/report.md
```

The 10,000-simulation power and bootstrap paths are intentionally expensive.
Do not lower their repetition count for a frozen result. Frozen outputs must
use `container/verify-power.sh` and `container/verify-determinism.sh`, which run
power and analysis twice from the same local OCI digest with no network. The
analysis verifier also renders the report twice. Both scripts compare bytes
and record SHA-256 hashes.
