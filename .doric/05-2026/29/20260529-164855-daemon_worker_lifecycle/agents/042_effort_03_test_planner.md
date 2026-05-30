# Agent Receipt: effort 03 test planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7662-03b9-7aa3-81cf-fdae95808d97
- Spawn result: completed
- Close result: completed status returned before shutdown
- Required agents row: `| development | efforts/03_lifecycle_domain_redaction.md | test planner | worker | agents/042_effort_03_test_planner.md | 019e7662-03b9-7aa3-81cf-fdae95808d97 | accepted |`

## Role

Test planner for effort 03 lifecycle domain redaction.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/PRD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/03_lifecycle_domain_redaction.md`
- `.agents/skills/doric/references/05-development.md`
- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/current-architecture/SKILL.md`
- Existing lifecycle package files.

## Read ownership

- `packages/lifecycle/src/lib.rs`
- `packages/lifecycle/proto/doric/lifecycle/v1.proto`
- Existing lifecycle test harness.
- Config masking and LLM provider redaction conventions.

## Write ownership

- None.

## Coding conventions

Used:

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/current-architecture/SKILL.md`

Constraints applied:

- Keep tests under `packages/lifecycle/tests/unit`.
- Test public lifecycle APIs, not private helpers.
- Keep lifecycle independent of `cli`, `daemon`, `worker`, and `tools`.
- Prove redaction and stable domain conversion only; do not claim sandboxing, authorization, path confinement, or environment-secret discovery.

## Prompt summary

Translate effort 03 acceptance criteria into concrete red tests and validation commands for lifecycle domain identity, status/event wrappers, and central redaction helpers.

## Output

Proposed test files:

- `packages/lifecycle/tests/unit/redaction.rs`
- `packages/lifecycle/tests/unit/domain.rs`

Proposed harness:

- Update `packages/lifecycle/src/lib.rs` to expose planned modules: `identity`, `repo`, `status`, `event`, and `redaction`.
- Include the new nested unit test files in the existing `#[cfg(test)]` harness.

Proposed redaction tests:

- `redact_repo_url_https_credentials_removes_userinfo`
- `redact_repo_url_token_like_username_removes_userinfo`
- `redact_repo_url_safe_query_and_fragment_preserves_them`
- `redact_failure_text_api_key_like_token_replaces_secret`
- `repo_identity_from_raw_url_stores_redacted_display`

Proposed domain tests:

- `worker_id_from_str_valid_value_round_trips_display`
- `worker_id_from_str_blank_or_whitespace_rejects_value`
- `request_id_from_str_valid_value_round_trips_display`
- `request_id_from_str_blank_or_whitespace_rejects_value`
- `status_from_proto_known_values_maps_to_domain`
- `status_domain_unavailable_and_untracked_have_stable_display`
- `event_constructors_lifecycle_milestones_preserve_status_and_name`
- `event_history_truncated_constructor_uses_stable_stream_event`

Expected red failure:

- Initial red should fail at compile time with unresolved imports/modules such as `lifecycle::redaction`, `lifecycle::repo::RepoIdentity`, `lifecycle::identity::{WorkerId, RequestId}`, `lifecycle::status`, and `lifecycle::event`.

Focused green commands:

- `cargo test -p lifecycle --no-fail-fast`
- `cargo test -p lifecycle redaction --no-fail-fast`
- `cargo test -p lifecycle domain --no-fail-fast`
- `cargo clippy -p lifecycle --all-targets -- -D warnings`
- `npx nx run lifecycle:test`
- `npx nx run lifecycle:lint`

Broader regression commands:

- `cargo fmt --all -- --check`
- `cargo test --workspace --all-features --no-fail-fast`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build -p cli --bin doric`

## Files changed

- None by sub-agent.

## Blocking questions

- None.

## Coordinator decision

accepted
