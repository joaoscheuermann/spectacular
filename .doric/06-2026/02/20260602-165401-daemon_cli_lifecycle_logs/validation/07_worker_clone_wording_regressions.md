# Validation: 07_worker_clone_wording_regressions

## Red evidence

- `cargo test -p worker run_worker_session_start_job_emits_redacted_clone_event_with_raw_repo_request`
- Exit code: 1
- Failure summary: `runtime::run_worker_session_start_job_emits_redacted_clone_event_with_raw_repo_request` expected worker `repo_preparation` message `cloning repo: https://github.com/org/repo.git`, but current implementation emitted `Preparing repository for prompt requirements`.
- Acceptance mapping: proves the worker runtime still emits the old clone-start wording before implementation changes. The new test also guards that credential-bearing raw repo URLs stay available to the repo preparer while display text is redacted.

Secondary red evidence from the test-writer worker:

- `cargo test -p worker run_worker_session_blank_token_preserves_outbound_status_and_event_text`
- Exit code: 1
- Failure summary: existing blank-token lifecycle text assertion is red on the same old repo-preparation wording.
- `cargo test -p worker run_worker_session_repo_preparation_failure_sends_failed_status_without_prompt_runner`
- Exit code: 1
- Failure summary: repo-preparation failure path is red on the same old repo-preparation wording.
- `cargo test -p daemon lifecycle_service_worker_frames_replay_prompt_milestones_without_collapsing_names`
- Exit code: 0
- Baseline note: daemon replay already preserves incoming worker event names and messages when supplied by worker frames.

## Green evidence

- `cargo test -p worker runtime`
- Exit code: 0
- Summary: 21 worker runtime tests passed, including redacted clone-start event text and raw repo request preservation.
- `cargo test -p worker repo`
- Exit code: 0
- Summary: 23 worker repo tests passed, including raw `git clone -- <repo> <repo-dir>` command construction and clone failure redaction.
- `cargo test -p daemon --test lifecycle_service`
- Exit code: 0
- Summary: daemon lifecycle service replay test passed and preserved worker event name/message.
- `cargo test -p cli handle_lifecycle_worker_stream_clone_activity_displays_incoming_clone_message`
- Exit code: 0
- Summary: CLI clone stream display regression passed.
- `cargo test -p cli handle_lifecycle_worker_stream_control_characters_and_secrets_render_one_safe_line`
- Exit code: 0
- Summary: CLI stream safety regression passed.
- `cargo test -p worker event`
- Exit code: 0
- Summary: 8 worker event tests passed.
- `cargo test -p daemon worker_session`
- Exit code: 0
- Summary: 9 daemon worker-session tests passed.
- `cargo test -p cli lifecycle_output`
- Exit code: 0
- Summary: 2 CLI lifecycle output tests passed.
- `cargo test -p lifecycle`
- Exit code: 0
- Summary: 33 lifecycle tests passed.
- `cargo fmt --check`
- Exit code: 0
- Summary: no formatting diff.
- `cargo clippy -p worker --all-targets -- -D warnings`
- Exit code: 0
- Summary: worker Clippy gate passed.

## Focused commands

- `cargo test -p worker run_worker_session_start_job_emits_redacted_clone_event_with_raw_repo_request`

## Regression commands

- `cargo test -p worker runtime`
- `cargo test -p worker repo`
- `cargo test -p worker event`
- `cargo test -p daemon --test lifecycle_service`
- `cargo test -p daemon worker_session`
- `cargo test -p cli lifecycle_output`
- `cargo test -p lifecycle`
- `cargo fmt --check`
- `cargo clippy -p worker --all-targets -- -D warnings`

## Unavailable tooling

None so far.

## Refactors applied

None. Validator/refactor found no blocker and did not edit files.

## Reviewer decision

Approved by `07_reviewer_01` (`Hubble`). No blocking findings; reviewed red/green evidence, raw clone preservation, safe clone event display, daemon replay message coverage, CLI stream regression coverage, file-size gates, and path-limited commit scope.

Commit checkpoint: `8367d4d` (`fix(worker): render redacted clone lifecycle wording`).
