# Case families

`src/study/cases.ts` materializes the frozen pilot instrument: 60 independent
families, 10 for each composition class A–F and 15 for each domain. Every case
is parsed by `CaseV1` and includes a canonical content hash.

Confirmatory and calibration families must use new family identifiers and
independently authored content. `validateFamilyIsolation` rejects reused
families and exact non-identity content clones even when IDs or tags change;
the required human pre-outcome audit remains responsible for semantic
near-duplicates and neutrality.
