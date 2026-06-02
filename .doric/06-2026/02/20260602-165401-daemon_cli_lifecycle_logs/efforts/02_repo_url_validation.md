# Effort: URL-only repo validation

Status: done

## Requirement links

- FEATURES.md: F2, F8.
- PRD.md: FR16, FR17, FR18, FR19, FR21, FR22, FR24; User Value Hops 3 and 8.
- TDD.md: Proposed design component 2; Dependency Hops 3, 4, and 10; security and privacy; testing strategy lifecycle repo validation.

## Goal

Add a shared lifecycle repository URL contract that accepts cloneable remote URLs only, rejects local/path-like inputs, and preserves separate raw clone and redacted display views for downstream daemon, CLI, and worker use.

## Sequence

- Position: 02 of 07.
- Previous effort: 01_shared_terminal_lines.md.
- Next effort: 03_daemon_dispatch_uuid_validation.md.
- Dependency reason: daemon and CLI validation must consume one shared URL contract before side-effect-free worker creation can be enforced.

## Target files

- `packages/lifecycle/src/repo.rs`: introduce `RepoUrl` or equivalent stricter constructor while preserving `RepoIdentity` display/redaction behavior.
- `packages/lifecycle/src/redaction.rs`: support credential redaction for accepted URL forms and safe error wording for rejected local inputs.
- `packages/lifecycle/src/lib.rs`: export any new repo validation type.
- `packages/lifecycle/tests/unit/redaction.rs` and/or `packages/lifecycle/tests/unit/domain.rs`: add accepted/rejected URL validation and display-view coverage.
- `packages/lifecycle/Cargo.toml`: add a parsing dependency such as `url` only if the implementation needs it.
- `Cargo.lock`: update only if a dependency is added.

Graph impact: `lifecycle::repo` is the policy node for repository inputs. Later daemon dispatch and CLI command code should call this node instead of reimplementing scheme, SCP-like, or path rejection logic.

## Coupled files

- `packages/daemon/src/service.rs`: later validates dispatch input before worker ID allocation and registry writes.
- `packages/cli/src/main/lifecycle.rs`: later validates `feature --repo` before connecting to the daemon.
- `packages/worker/src/runtime.rs` and `packages/worker/src/repo.rs`: later receive raw clone input only after daemon validation.
- `packages/worker/tests/unit/repo.rs`: later verifies raw repo URL remains the exact clone argument.

Graph impact: raw clone URL flows only from accepted `RepoUrl` through daemon `LaunchRequest` to worker Git clone execution. Redacted identity/display flows to registry summaries, daemon logs, CLI tables, and stream lines.

## Ownership

- Intended worker write scope: lifecycle repo/redaction modules, lifecycle exports, lifecycle unit tests, and lifecycle manifest/lockfile if a parser dependency is required.
- Read-only context: daemon dispatch, CLI lifecycle command handling, worker repo clone command tests.
- Known conflict risks: do not tighten `WorkerId::from_str`; this effort is about repository input validation only.

## Tests to add or update

- Accept explicit scheme remote URLs with non-empty hosts, including representative `https`, `http`, `ssh`, and `git` forms.
- Accept or reject SCP-like remotes explicitly, with tests for the final decision. The accepted TDD default is to accept `git@host:path` when host and path are present.
- Reject blank strings, relative paths, absolute POSIX paths, Windows drive paths, UNC paths, path-like values, and local `file://` inputs.
- Prove rejected local/path-like errors are understandable without unnecessarily echoing local filesystem details.
- Prove credential-bearing accepted URLs expose redacted display identity while retaining raw clone input for clone execution.

## Regression suites

- `cargo test -p lifecycle`

Graph impact: this suite protects the shared validation policy before daemon, CLI, and worker efforts consume it. Daemon/CLI/worker suites become required in later efforts where those packages wire the policy in.

## Acceptance criteria

- Shared repo validation accepts cloneable remote URL syntax only.
- SCP-like remote handling is explicit in tests and error cases; no ambiguous parser behavior is left undocumented.
- Local relative paths, absolute paths, Windows/UNC paths, path-like inputs, blank inputs, and `file://` inputs are rejected.
- Raw repo URL is available only as the accepted clone input view; redacted identity/display is available for all terminal, registry, list, and stream surfaces.
- Credential-bearing URLs are redacted in display identity.
- Rejected local/path-like errors do not leak unnecessary local filesystem details.
- No global `WorkerId::from_str` tightening is introduced.

## Notes

- Keep accepted syntax bounded. If implementation discovers broader Git remote syntax ambiguity, keep the initial accepted set to explicit scheme URLs plus the tested SCP-like form and make errors clear.
