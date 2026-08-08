# Append-only study artifacts

The runner creates one directory per run under `traces/` and `records/`.
Events are written with exclusive creation, a monotonic sequence, a payload
hash, and a previous-event hash. Derived traces are immutable and named by
their content hash. Every execution attempt receives a separate record; a
resume never overwrites the technical failure that preceded it.

Keep credentials outside this tree. `structure` capture contains only stable
identifiers, statuses, counts, and durations. `io` additionally contains
model-visible content and deterministic tool I/O after recursive secret and
private-reasoning removal.

Freeze manifests, schedules, score rows, review artifacts, model prices,
prompts, seeds, and container digests belong in the packaged study file list.
The `package` command copies only an explicit regular-file allowlist and uses
exclusive creation at the destination.
