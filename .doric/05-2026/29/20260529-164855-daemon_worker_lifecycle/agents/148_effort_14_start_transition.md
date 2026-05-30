# Coordinator Transition: Effort 14 Start

## Transition

- Effort: `efforts/14_lifecycle_integration_smoke.md`
- Previous status: `todo`
- New status: `in-progress`
- Previous cursor: `Current effort: none`, `Next effort index: 13`
- New cursor: `Current effort: efforts/14_lifecycle_integration_smoke.md`, `Next effort index: 13`
- Active lock: effort 14 workers

## Preconditions

- Earlier efforts 0 through 12 are marked `done`.
- Effort 13 is marked `done` and has commit checkpoint `03622570cba2945353164912478bb7c0d05a2840`.
- Effort order index 13 is `efforts/14_lifecycle_integration_smoke.md`.

## Write scope

- `packages/cli/tests/lifecycle_smoke.rs`
- `packages/daemon/tests/integration/lifecycle_service.rs`
- `packages/worker/tests/integration/runtime_session.rs`
- `packages/lifecycle/tests/unit/proto_contract.rs`
- `packages/daemon/src/service.rs`
- `packages/worker/src/runtime.rs`
- `packages/cli/src/main/lifecycle.rs`
- Effort 14 Doric artifacts
- Carried-forward effort 13 hash finalization artifacts

## Coordinator decision

Coordinator decision: effort 14 may begin with test planning.
