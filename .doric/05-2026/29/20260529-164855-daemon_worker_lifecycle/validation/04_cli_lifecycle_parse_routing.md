# Validation: CLI lifecycle parse routing

## Red evidence

Captured before implementation by `agents/051_effort_04_test_writer.md`.

| Command | Exit code | Failure summary | Acceptance criteria mapping |
| --- | --- | --- | --- |
| `cargo test -p cli --no-fail-fast` | 1 | Compile-red due to missing intended production API: `LifecycleDispatchArgs`, `LifecycleAnswerArgs`, `Command::Feature`, `Command::Debug`, `Command::Answer`, `DispatchDependencies`, and `dispatch_with_dependencies`. | Proves lifecycle command grammar and routing-order seams are missing before effort 04 implementation. |

The updated process startup test expects `doric config` to preserve stale debug log content instead of truncating/replacing it, but did not run because compilation stopped first.

## Green evidence

Captured by `agents/053_effort_04_validator_refactor.md`.

| Command | Exit code | Summary |
| --- | --- | --- |
| `cargo test -p cli top_level_help_lists_lifecycle_commands_without_chat` | 0 | Top-level help includes lifecycle commands and excludes `chat`. |
| `cargo test -p cli try_parse_from_feature_with_prompt_and_repo_preserves_mode` | 0 | Feature dispatch command parses prompt, repo, and mode-preserving variant. |
| `cargo test -p cli try_parse_from_debug_with_prompt_and_repo_preserves_mode` | 0 | Debug dispatch command parses prompt, repo, and mode-preserving variant. |
| `cargo test -p cli try_parse_from_answer_with_ids_and_text_preserves_routing_fields` | 0 | Answer command parses worker id, request id, text, and route fields. |
| `cargo test -p cli dispatch_lifecycle_command_does_not_create_debug_logger` | 0 | Lifecycle dispatch bypasses chat/provider debug-log startup. |
| `cargo test -p cli dispatch_bare_invocation_creates_debug_logger_before_chat_run` | 0 | Bare invocation creates debug logger before chat. |
| `cargo test -p cli doric_process_config_command_preserves_stale_debug_log_content` | 0 | Config command no longer truncates/replaces the provider debug log. |
| Added daemon/list/worker parse and output checks | 0 | Daemon preserves `--worker-root`; worker preserves id; list preserves address. |
| `cargo test -p cli --no-fail-fast` | 0 | CLI package tests pass. |
| `cargo build -p cli --bin doric` | 0 | CLI binary builds. |
| `cargo clippy -p cli --all-targets -- -D warnings` | 0 | CLI clippy gate passes with warnings denied. |
| `npx nx run cli:test` | 0 | Nx CLI test target passes. |
| `cargo fmt --all -- --check` | 0 | Workspace formatting check passes. |

## Focused commands

- `cargo test -p cli top_level_help_lists_lifecycle_commands_without_chat`
- `cargo test -p cli try_parse_from_feature_with_prompt_and_repo_preserves_mode`
- `cargo test -p cli try_parse_from_debug_with_prompt_and_repo_preserves_mode`
- `cargo test -p cli try_parse_from_answer_with_ids_and_text_preserves_routing_fields`
- `cargo test -p cli dispatch_lifecycle_command_does_not_create_debug_logger`
- `cargo test -p cli dispatch_bare_invocation_creates_debug_logger_before_chat_run`
- `cargo test -p cli doric_process_config_command_preserves_stale_debug_log_content`

## Regression commands

- `cargo test -p cli --no-fail-fast`
- `cargo build -p cli --bin doric`
- `cargo clippy -p cli --all-targets -- -D warnings`
- `npx nx run cli:test`

## Unavailable tooling

- No tooling was unavailable during red evidence capture.
- No tooling was unavailable during green validation. One attempted Cargo command used two test filters and exited 1; the filters were rerun individually and passed.

## Refactors applied

- Added `LifecycleDaemonArgs` with optional `--worker-root`.
- Added `LifecycleWorkerArgs` with required worker id.
- Routed daemon worker root and worker id through the lifecycle handler/output seam.
- Added parse/output coverage for `daemon`, `list`, and `worker`.
- Split lifecycle args so `daemon`, `list`, and `worker` do not share an underspecified address-only shape.

## Reviewer decision

Reviewer `agents/054_effort_04_reviewer.md` approved effort 04. No blocking findings were reported. The reviewer found acceptance criteria satisfied, scope correct for temporary CLI-local lifecycle routing, validation evidence credible, and the effort ready for a scoped checkpoint after coordinator records the done transition and checkpoint scope reconciliation.
