# Effort: Daemon dispatch validation and UUIDv6 IDs

Status: todo

## Requirement links

- FEATURES.md: F2, F3, F5, F8.
- PRD.md: FR14, FR15, FR16, FR17, FR18, FR19, FR20, FR21, FR22; User Value Hops 2, 3, 5, and 8.
- TDD.md: Proposed design components 2 and 3; Dependency Hops 4, 5, and 6; daemon service contracts; rollout step 1/2.

## Goal

Wire the shared repo validation into daemon dispatch before side effects, and replace production timestamp-label worker IDs with UUIDv6 generation behind the existing daemon `IdGenerator` seam.

## Sequence

- Position: 03 of 07.
- Previous effort: 02_repo_url_validation.md.
- Next effort: 04_daemon_timestamps_logger.md.
- Dependency reason: daemon dispatch must enforce accepted repo URLs and UUIDv6 IDs before timestamped summaries, logs, CLI lifecycle output, and worker stream output can rely on correct worker creation data.

## Target files

- `packages/daemon/src/service.rs`: validate repo through the shared lifecycle URL type before worker ID allocation, layout creation, registry insertion, accepted-event append, or launch; use redacted identity in response/registry and raw URL in launch request.
- `packages/daemon/src/server.rs`: replace production `TimestampIdGenerator` wiring with the UUIDv6 generator while keeping injected `IdGenerator` test seams.
- `packages/daemon/Cargo.toml`: add `uuid` dependency and required features if UUID generation lives in `daemon`.
- `Cargo.lock`: update if the UUID dependency is added.
- `packages/daemon/tests/unit/service.rs`: add side-effect-free invalid repo cases and UUIDv6 dispatch assertions.
- `packages/daemon/tests/unit/server.rs`: update production service wiring tests if they assert generator type/wiring.
- `packages/daemon/tests/integration/lifecycle_service.rs`: update worker ID fixtures only where newly-created-worker behavior is under test.

Graph impact: daemon dispatch is the creation boundary. It depends on `lifecycle::repo` for validation, `lifecycle::identity::WorkerId` for typed IDs, `daemon::root` for layout safety, `daemon::registry` for state, and `daemon::process`/session launcher for raw clone execution.

## Coupled files

- `packages/lifecycle/src/repo.rs`: validation source of truth from the previous effort.
- `packages/daemon/src/root.rs`: worker layout remains coupled because generated IDs become path components.
- `packages/worker/src/repo.rs`: raw clone URL remains the worker Git clone argument later.
- `packages/cli/src/main/lifecycle_output.rs`: later displays UUIDv6 worker IDs returned by daemon.

Graph impact: invalid repo input must terminate at the daemon service node with no edges to ID generation, root layout creation, registry mutation, worker launch, accepted event append, or terminal logger.

## Ownership

- Intended worker write scope: daemon service/server code, daemon manifest/lockfile as needed, daemon service/server/integration tests.
- Read-only context: lifecycle repo validation tests, worker repo clone tests, CLI lifecycle output tests.
- Known conflict risks: existing test fixtures may use descriptive IDs. Update only tests that exercise newly-created production worker IDs; descriptive IDs may remain in non-creation tests.

## Tests to add or update

- Daemon dispatch rejects relative paths, absolute POSIX paths, Windows drive paths, UNC paths, path-like values, and `file://` inputs before registry writes, ID generation, layout creation, accepted-event append, or launch.
- Daemon dispatch accepts representative remote URLs and SCP-like remotes according to lifecycle validation tests.
- Dispatch response for newly created production workers exposes a UUIDv6 worker ID.
- Production UUIDv6 node-id/clock strategy is encapsulated behind `IdGenerator`; tests retain deterministic injected IDs.
- Launch request receives the raw accepted repo URL, while response/registry identity uses the redacted display value.

## Regression suites

- `cargo test -p daemon service`
- `cargo test -p daemon server`
- `cargo test -p daemon --test lifecycle_service`
- `cargo test -p lifecycle`

Graph impact: lifecycle validates the input policy, daemon service validates side-effect ordering and display/raw data flow, daemon server validates production wiring, and integration validates worker-session replay compatibility after creation changes.

## Acceptance criteria

- Daemon rejects invalid repo inputs before worker ID allocation, layout creation, registry insertion, accepted-event append, or worker launch.
- Newly created production workers use UUIDv6 IDs.
- UUIDv6 generation remains behind `IdGenerator`; UUID node-id/clock strategy does not leak into callers.
- `WorkerId::from_str` remains nonblank-compatible unless implementation evidence proves broad tightening is necessary and that decision is separately reviewed.
- Raw repo URL is used only for clone execution via launch request; redacted repo identity is used for dispatch response and registry summaries.
- Existing lifecycle semantics remain unchanged except for required validation and ID generation changes.

## Notes

- If `uuid` crate feature resolution fails, stop and report the dependency blocker rather than hand-rolling UUIDv6 in this effort without review.
