# Agent Receipt: effort 04 test planner

## Spawn proof

- Tool: multi_agent_v1.spawn_agent
- Agent type: worker
- Agent id: 019e7670-eee1-7761-b223-672f94a575ab
- Spawn result: completed
- Close result: completed status returned before shutdown
- Required agents row: `| development | efforts/04_cli_lifecycle_parse_routing.md | test planner | worker | agents/050_effort_04_test_planner.md | 019e7670-eee1-7761-b223-672f94a575ab | accepted |`

## Role

Test planner for effort 04 CLI lifecycle parse/routing.

## Input artifacts

- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/TDD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/PRD.md`
- `.doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/04_cli_lifecycle_parse_routing.md`
- `.agents/skills/doric/references/05-development.md`
- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/current-architecture/SKILL.md`
- Existing CLI main and chat startup files.

## Read ownership

- `packages/cli/src/main/cli_types.rs`
- `packages/cli/src/main/entry.rs`
- `packages/cli/src/main/output.rs`
- `packages/cli/src/main.rs`
- `packages/cli/tests/unit/main_cli.rs`
- `packages/cli/tests/debug_log_startup_test.rs`
- Read-only chat/config routing context.

## Write ownership

- None.

## Coding conventions

Used:

- `.agents/skills/coding-conventions/SKILL.md`
- `.agents/skills/current-architecture/SKILL.md`

Constraints applied:

- Keep effort 04 inside `cli`; later efforts own real daemon client/output wiring.
- Avoid adding daemon, worker, or lifecycle runtime dependencies unless already required.
- Use dependency injection for routing-order tests so fake debug logger creation, chat, config, and lifecycle handlers can prove order without filesystem/provider side effects.

## Prompt summary

Translate effort 04 acceptance criteria into concrete red tests and validation commands for lifecycle CLI grammar and debug-log startup routing.

## Output

Proposed test files:

- Update `packages/cli/tests/unit/main_cli.rs`.
- Add `packages/cli/tests/unit/entry.rs`.
- Include `packages/cli/tests/unit/entry.rs` from `packages/cli/src/main/entry.rs` with a minimal test harness.
- Update `packages/cli/tests/debug_log_startup_test.rs`.

Proposed red tests:

- `top_level_help_lists_lifecycle_commands_without_chat`
- `try_parse_from_feature_with_prompt_and_repo_preserves_mode`
- `try_parse_from_debug_with_prompt_and_repo_preserves_mode`
- `try_parse_from_answer_with_ids_and_text_preserves_routing_fields`
- `try_parse_from_feature_without_prompt_returns_missing_required_argument`
- `try_parse_from_debug_without_repo_returns_missing_required_argument`
- `try_parse_from_answer_without_text_returns_missing_required_argument`
- `dispatch_config_command_does_not_create_debug_logger`
- `dispatch_lifecycle_command_does_not_create_debug_logger`
- `dispatch_bare_invocation_creates_debug_logger_before_chat_run`
- `doric_process_config_command_does_not_replace_debug_log_on_start`

Expected red failures:

- Lifecycle parse/help tests fail with unknown subcommands or missing help text because only `config` exists.
- Required-argument tests fail as invalid subcommands instead of required-field errors.
- Entry routing tests fail to compile until a testable dispatch seam exists.
- Debug-log startup process test fails because current `entry::run` creates/truncates the LLM debug log before config dispatch.

Focused commands:

- `cargo test -p cli top_level_help_lists_lifecycle_commands_without_chat`
- `cargo test -p cli try_parse_from_feature_with_prompt_and_repo_preserves_mode`
- `cargo test -p cli try_parse_from_debug_with_prompt_and_repo_preserves_mode`
- `cargo test -p cli try_parse_from_answer_with_ids_and_text_preserves_routing_fields`
- `cargo test -p cli dispatch_lifecycle_command_does_not_create_debug_logger`
- `cargo test -p cli dispatch_bare_invocation_creates_debug_logger_before_chat_run`
- `cargo test -p cli doric_process_config_command_does_not_replace_debug_log_on_start`

Regression commands:

- `cargo test -p cli --no-fail-fast`
- `cargo build -p cli --bin doric`
- `cargo clippy -p cli --all-targets -- -D warnings`
- `npx nx run cli:test`

## Files changed

- None by sub-agent.

## Blocking questions

- None.

## Coordinator decision

accepted
