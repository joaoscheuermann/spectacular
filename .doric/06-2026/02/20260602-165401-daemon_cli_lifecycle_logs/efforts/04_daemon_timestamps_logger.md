# Effort: Daemon timestamps and lifecycle logger

Status: todo

## Requirement links

- FEATURES.md: F4, F7, F8.
- PRD.md: FR3, FR8, FR11, FR12, FR13, FR15, FR20, FR21; User Value Hops 4, 7, and 9.
- TDD.md: Proposed design components 4 and 5; Dependency Hops 7, 8, and 12; daemon service contracts; testing strategy daemon logger and timestamp cases.

## Goal

Populate daemon lifecycle timestamps on worker summaries/events and add daemon terminal lifecycle logging through an injected logger boundary in `LifecycleService` and `SessionManager`.

## Sequence

- Position: 04 of 07.
- Previous effort: 03_daemon_dispatch_uuid_validation.md.
- Next effort: 05_cli_feature_list_output.md.
- Dependency reason: CLI table and stream rendering need daemon timestamps, and daemon lifecycle logs need validated repo identities and UUIDv6 worker IDs from earlier daemon creation work.

## Target files

- `packages/daemon/src/event.rs`: carry occurred-at metadata on registry worker events, or expose it through an equivalent timestamped event model.
- `packages/daemon/src/registry.rs`: record `updated_at` on worker records/summaries and `occurred_at` on appended events at successful mutation time.
- `packages/daemon/src/service.rs`: populate `WorkerSummary.updated_at` and `WorkerEvent.occurred_at`; inject and call daemon lifecycle logger after successful state changes and stream start.
- `packages/daemon/src/worker_session.rs`: inject logger through `WorkerSessionConfig`/`SessionManager` and log attach, status updates, worker events, input requests, answer forwarding, failures, success, stop, deadline failures, and child exits.
- `packages/daemon/src/server.rs`: wire production terminal logger and test/noop logger through service/session construction.
- `packages/daemon/src/main.rs`: no change expected unless production stderr wiring requires it.
- `packages/daemon/tests/unit/registry.rs`, `packages/daemon/tests/unit/service.rs`, `packages/daemon/tests/unit/worker_session.rs`, and `packages/daemon/tests/integration/lifecycle_service.rs`: add timestamp and logger coverage.

Graph impact: registry mutation is the timestamp source; service converts timestamps to proto; session manager sees worker lifecycle categories; logger uses shared lifecycle terminal formatting from effort 01 and must not feed back into registry state.

## Coupled files

- `packages/lifecycle/src/terminal.rs`: daemon logger should render through shared safe timestamped lines.
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`: read-only; existing optional timestamp fields are used without protobuf changes.
- `packages/cli/src/main/lifecycle_client.rs`: later consumes `occurred_at` from proto stream/list responses.
- `packages/worker/src/runtime.rs`: worker-emitted event names and messages drive daemon session logging.

Graph impact: logger injection edges are `server -> LifecycleService` and `server -> SessionManager`; service/session call the logger after successful registry or command state changes. Logger writes to terminal only and does not create durable audit storage.

## Ownership

- Intended worker write scope: daemon event/registry/service/worker_session/server code and daemon unit/integration tests.
- Read-only context: lifecycle terminal formatter, generated proto types, worker event names, CLI client conversion.
- Known conflict risks: avoid editing protobuf unless implementation proves current fields are absent. Current TDD says they already exist.

## Tests to add or update

- Registry tests prove inserted/updated summaries carry update timestamps and appended events carry occurred-at timestamps.
- Service tests prove list and stream proto conversions populate timestamp fields where available and tolerate compatibility cases where timestamps are absent.
- Service/session logger tests use recording/noop logger and assert required lifecycle categories without writing to real terminal output.
- Logger tests verify messages are concise, safe, and redacted.
- Stream-start logging is covered without changing gRPC method names or stream behavior.

## Regression suites

- `cargo test -p daemon registry`
- `cargo test -p daemon service`
- `cargo test -p daemon worker_session`
- `cargo test -p daemon --test lifecycle_service`
- `cargo test -p lifecycle`

Graph impact: daemon registry tests validate timestamp source nodes, service/session tests validate logger injection and lifecycle categories, integration validates replay/tail event semantics remain intact.

## Acceptance criteria

- `WorkerSummary.updated_at` and `WorkerEvent.occurred_at` are populated from daemon mutation time without protobuf changes.
- Timestamp capture happens after successful state mutation and does not claim failed mutations occurred.
- `LifecycleLogger` or equivalent daemon-local boundary is injected through both `LifecycleService` and `SessionManager`.
- Production daemon lifecycle logs write terminal lines through the shared lifecycle formatter, preferably stderr unless review decides otherwise.
- Recording/noop loggers make logger behavior testable without real terminal assertions.
- Logs cover worker creation, stream start, attach, status update, repo clone start, input requested, answer provided without answer text, failure, success, stop, attach deadline failure, spawn failure, and child exit where those events occur.
- Logger output is concise, one-line, redacted, and does not add durable audit storage or structured log formats.

## Notes

- If required lifecycle categories cannot be observed from service/session boundaries, stop and report the missing category before moving logging into registry observers.
