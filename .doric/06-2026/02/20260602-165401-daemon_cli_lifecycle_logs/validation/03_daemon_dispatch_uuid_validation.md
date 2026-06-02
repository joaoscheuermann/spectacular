# Validation: 03_daemon_dispatch_uuid_validation

## Red evidence

- Reviewer-triggered regression command: `cargo test -p daemon dispatch_whitespace_padded_remote_repo_rejects_without_id_record_layout_event_or_launch`
  - Exit code: 1
  - Expected failure: daemon dispatch trims repo input before shared `RepoUrl` validation, allowing whitespace-padded remote URLs to bypass lifecycle validation and losing the exact raw clone input.
  - Evidence: test panicked at `packages/daemon/tests/unit/service.rs:123` because `dispatch(" https://example.com/org/repo.git ")` returned `Ok(DispatchResponse { worker_id: "worker-1", mode: Feature, repo_identity: "https://example.com/org/repo.git", status: Accepted })` instead of rejecting before ID generation, layout creation, registry insertion, accepted-event append, or launch.
  - Assertion scope: the new test asserts the eventual error must not echo the whitespace-padded input, `fixture.remaining_ids()` remains `["worker-1"]`, and `fixture.worker_layout_exists("worker-1")` is false.

- Command: `cargo test -p daemon dispatch_invalid_repo_urls_reject_without_id_record_layout_event_or_launch`
  - Exit code: 1
  - Expected failure: daemon dispatch accepts path-like repo input before shared `RepoUrl` validation.
  - Evidence: test panicked because `dispatch("org/repo")` returned `Ok(DispatchResponse { worker_id: "worker-1", mode: Feature, repo_identity: "org/repo", status: Accepted })` instead of rejecting before ID generation, layout creation, registry insertion, accepted-event append, or launch.
  - Scope correction: invalid repo table now also includes bare `repo` and POSIX-style UNC `//server/share/repo`.

- Command: `cargo test -p daemon dispatch_remote_repo_url_uses_raw_clone_input_and_redacted_identity`
  - Exit code: 0
  - Evidence: required raw clone input and redacted/display identity coverage already passes for credential-bearing HTTPS, `ssh://`, `git://`, and SCP-like `git@host:path` remotes. This behavior is already present before the implementation pass.

- Command: `cargo test -p daemon build_process_service_dispatch_generates_uuid_v6_worker_id`
  - Exit code: 1
  - Expected failure: production `build_process_service` still wires timestamp-based worker IDs.
  - Evidence: test panicked with `worker id should be UUIDv6-shaped, got feature-1780440208727042200`.

## Notes

- Added tests only in daemon service/server unit tests.
- Green implementation changes are limited to daemon service/server production code, daemon Cargo metadata, `Cargo.lock`, and a daemon service unit assertion update needed to keep the bare `repo` rejection wording non-echoing.
- Validator/refactor pass kept edits within the assigned daemon manifest and this validation record. Reduced daemon `uuid` features from explicit `v4`, `v6`, `std`, `rng` to `v4`, `v6`; `std` remains the crate default and `rng` is implied by `v4`, while `v6` supplies `Uuid::now_v6` plus the atomic clock sequence.

## Validator/refactor evidence

- Replacement validator pass, 2026-06-02:
  - Minimal refactor: strengthened `dispatch_whitespace_padded_remote_repo_rejects_without_id_record_layout_event_or_launch` with a positive assertion that the sanitized error remains in the remote-URL validation class.
  - Code inspection reconfirmed repo requiredness preserves original nonblank repo strings before `RepoUrl::try_from`.
  - Code inspection reconfirmed whitespace-padded remotes reject before ID generation, layout creation, registry insertion, accepted-event append, or launch.
  - Code inspection reconfirmed invalid local/path-like repos reject with safe wording and no path echo.
  - Code inspection reconfirmed accepted remotes and SCP-like remotes use exact raw clone input for launch and redacted identity for registry, response, and launch metadata.
  - Code inspection reconfirmed production process service generates UUIDv6 behind the existing `IdGenerator` seam.
  - Code inspection reconfirmed `WorkerId::from_str` remains nonblank-compatible in `packages/lifecycle/src/identity.rs`.

- Code inspection: `LifecycleService::dispatch` checks repo requiredness without normalizing the value, passes the original repo string to `RepoUrl::try_from`, and does so before worker-root validation, ID generation, layout creation, registry insertion, accepted-event append, or launch.
- Code inspection: local/path-like repo errors are mapped to `source URL must be a remote URL, not a local path`, avoiding echo of unsafe path input.
- Code inspection: accepted remotes use `RepoUrl::as_clone_input()` for `LaunchRequest.repo` and `RepoUrl::identity()` for registry records, dispatch response, and launch metadata.
- Code inspection: `build_process_service` wires `UuidV6IdGenerator::default()` only at the production process-service composition root; unit-test service construction still injects deterministic `IdGenerator` implementations.
- Code inspection: `WorkerId::from_str` still delegates to nonblank-only parsing in `packages/lifecycle/src/identity.rs`; no tightening was introduced in this effort.
- Dependency inspection: `cargo tree -p daemon -e features | Select-String -Pattern 'uuid feature','getrandom v0.4.2'` shows daemon now requests `uuid` default/std, `v4` with implied `rng`/`getrandom`, and `v6` with implied `atomic`.

## Green evidence

### Replacement command results, 2026-06-02

- `cargo test -p daemon dispatch_whitespace_padded_remote_repo_rejects_without_id_record_layout_event_or_launch`: exit code 0; 1 selected test passed.
- `cargo test -p daemon dispatch_invalid_repo_urls_reject_without_id_record_layout_event_or_launch`: exit code 0; 1 selected test passed.
- `cargo test -p daemon dispatch_remote_repo_url_uses_raw_clone_input_and_redacted_identity`: exit code 0; 1 selected test passed.
- `cargo test -p daemon build_process_service_dispatch_generates_uuid_v6_worker_id`: exit code 0; 1 selected test passed.
- `cargo test -p daemon service`: exit code 0; 20 selected daemon unit tests passed plus the selected lifecycle service integration replay test.
- `cargo test -p daemon server`: exit code 0; 8 selected daemon server unit tests passed.
- `cargo test -p daemon --test lifecycle_service`: exit code 0; 1 integration test passed.
- `cargo test -p lifecycle repo_url`: exit code 0; 10 selected lifecycle repo URL/redaction tests passed.
- `cargo test -p worker repo`: exit code 0; 22 selected worker repo/runtime/tooling tests passed.
- `cargo fmt -p daemon -- --check`: exit code 0.
- `cargo clippy -p daemon --all-targets -- -D warnings`: exit code 0.

- Command: `cargo test -p daemon dispatch_whitespace_padded_remote_repo_rejects_without_id_record_layout_event_or_launch`
  - Exit code: 0
  - Evidence: whitespace-padded remote repo input now rejects before ID generation, registry writes, layout creation, accepted-event append, or launch.

- Command: `cargo test -p daemon dispatch_invalid_repo_urls_reject_without_id_record_layout_event_or_launch`
  - Exit code: 0
  - Evidence: invalid local/path-like repo inputs now reject before ID generation, registry writes, layout creation, accepted-event append, or launch.

- Command: `cargo test -p daemon dispatch_remote_repo_url_uses_raw_clone_input_and_redacted_identity`
  - Exit code: 0
  - Evidence: accepted remote inputs keep the exact raw clone URL in `LaunchRequest.repo` and use redacted identity for launch metadata, registry summaries, and dispatch response.

- Command: `cargo test -p daemon build_process_service_dispatch_generates_uuid_v6_worker_id`
  - Exit code: 0
  - Evidence: production `build_process_service` dispatch returns a UUIDv6-shaped worker ID.

- Command: `cargo test -p daemon service`
  - Exit code: 0
  - Evidence: 20 selected daemon service/server unit tests passed, plus the lifecycle service integration replay test.

- Command: `cargo test -p daemon server`
  - Exit code: 0
  - Evidence: 8 selected daemon server unit tests passed.

- Command: `cargo test -p daemon --test lifecycle_service`
  - Exit code: 0
  - Evidence: lifecycle service integration replay test passed.

- Command: `cargo test -p lifecycle repo_url`
  - Exit code: 0
  - Evidence: 10 lifecycle repo URL/redaction tests passed.

- Command: `cargo test -p worker repo`
  - Exit code: 0
  - Evidence: 22 selected worker repo/runtime/tooling tests passed.

- Command: `cargo fmt -p daemon`
  - Exit code: 0
  - Evidence: daemon package formatting applied before the required format check.

- Command: `cargo fmt -p daemon -- --check`
  - Exit code: 0
  - Evidence: daemon package format check passed.

- Command: `cargo clippy -p daemon --all-targets -- -D warnings`
  - Exit code: 0
  - Evidence: daemon package and all targets passed clippy with warnings denied.
