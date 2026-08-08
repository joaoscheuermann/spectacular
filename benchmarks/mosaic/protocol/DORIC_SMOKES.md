# Doric-targeted smoke contracts

The harness defines exactly six opt-in smokes, one for each composition class
A–F. Their cases use `phase: smoke`; their conditions use `kind: smoke` and
`eligibility: opt-in`.

They are integration checks only. Primary, confirmatory, replication, power,
baseline-selection, and semantic-review routines must not include them.

The current opt-in runner exercises these contracts through the isolated
benchmark composition. It does not claim to execute `agents/doric` itself:
that composition root has an unrelated in-progress model change and must
stabilize before a real host smoke can be added without overwriting that work.
