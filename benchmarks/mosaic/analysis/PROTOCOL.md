# Confirmatory analysis protocol

The analysis consumes schema-valid `ScoreRowV1` rows. Infrastructure outcomes
remain rows with `success = 0`; the scorer derives `primaryEligible` from the
registered family policy, frozen model, phase, conditions, manifest hash, and
budget. Cost, token, call, and duration fields are descriptive and never gate
success or eligibility.

Each input contains exactly M1 and the frozen baseline, five paired repetitions
per case, unique run identifiers after technical retry resolution, one frozen
model configuration, and one freeze hash. Primary and replication inputs are
uncapped. Sensitivity inputs use one positive frozen model-call cap.
The analysis implementation hash must exactly equal
`freeze.artifactHashes.analysis`; the result also carries the manifest hash.
The dataset contains exactly `freeze.nFinal` cases and `freeze.nFinal * 10`
rows. Every case/repetition pair shares one exclusive paired-block identifier
between M1 and the baseline.

The primary analysis is fitted separately for each frozen model family. The
replication is never pooled with the OpenAI study. The exact model is:

```r
glmer(
  success ~ condition * composition_class + (1 | case_id),
  family = binomial(),
  control = glmerControl(
    optimizer = "bobyqa",
    optCtrl = list(maxfun = 200000)
  )
)
```

The two-sided primary contrast is M1 versus the frozen pilot baseline among
composition classes E/F or cases pre-registered as adaptive. Alpha is 0.05 and
the minimum relevant absolute effect is 0.10. Outputs include the odds ratio,
standardized absolute probability difference, 95% interval, and p-value.
Secondary families use Holm correction.

The budget sensitivity freezes the nearest-rank p95 model-call count across
all frozen-baseline pilot rows, including failures. It runs as the separate
`sensitivity` family and never replaces the uncapped primary result.

Fallback order is frozen: `bobyqa`, `nloptwrap`, a fixed-effects binomial GLM
with case-clustered HC2 covariance, then a 10,000-sample case bootstrap. A
bootstrap with fewer than 95% valid fits is `not-estimable`; no later method or
manual substitution is allowed.

Power uses 10,000 simulations with `L'Ecuyer-CMRG`. `N_power` is the first
multiple of 24 reaching 80%. `N_final` is `max(240, N_power)` rounded up to a
multiple of 120, preserving the 6-by-4 matrix and exact 20% review cells.
The power result records its numeric seed, input-config hash, baseline
probability, random-intercept SD, and adaptive composition classes. The freeze
records the config-byte hash as `powerConfig` and the published result-byte hash
as `powerResult`. Primary and replication analyses accept the result only when
both hashes, every field, and both sample sizes match. Secondary and sensitivity
validate the same frozen result but materialize `power = null` in their output.
All families use exactly 10,000 bootstrap samples and the frozen numeric power
seed. Only registered secondary families may supply custom contrasts.

Run the primary and replication inputs in distinct invocations. Runtime
containers must use an immutable image digest, `--network none`, a read-only
root filesystem, no capabilities, and the frozen `renv.lock`.
