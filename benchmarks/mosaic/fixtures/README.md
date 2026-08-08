# Deterministic fixtures

`world-v1` is defined in `src/runtime/world.ts`. Each run receives a detached
snapshot. The 24 benchmark tools can read or replace values only inside that
snapshot; they contain no network, shell, process, or external-filesystem
operation.
