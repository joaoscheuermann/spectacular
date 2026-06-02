# Validation: 01_shared_terminal_lines.md

## Red evidence

- Command: `cargo test -p lifecycle format_timestamp_unix_epoch_renders_compact_utc_rfc3339`
- Exit code: `1`
- Failure summary: expected compile failure before implementation; `lifecycle::terminal::format_timestamp` is unresolved because the public terminal module does not exist yet.

- Command: `cargo test -p lifecycle format_line_fixed_timestamp_renders_timestamped_safe_message`
- Exit code: `1`
- Failure summary: expected compile failure before implementation; `lifecycle::terminal::format_line` is unresolved because the public terminal module does not exist yet.

- Command: `cargo test -p lifecycle safe_message`
- Exit code: `1`
- Failure summary: expected compile failure before implementation; `lifecycle::terminal::safe_message` is unresolved because the public terminal module does not exist yet.

The red failures map to the effort acceptance criteria for shared timestamp formatting, timestamped lifecycle lines, and safe one-line message rendering.

## Green evidence

- Command: `cargo test -p lifecycle format_timestamp`
- Exit code: `0`
- Summary: timestamp formatting tests passed.

- Command: `cargo test -p lifecycle format_line`
- Exit code: `0`
- Summary: timestamped lifecycle line tests passed.

- Command: `cargo test -p lifecycle safe_message`
- Exit code: `0`
- Summary: safe-message tests passed, including control normalization, API-key redaction, credential URL redaction, and ordinary text preservation.

- Command: `cargo test -p lifecycle redact_`
- Exit code: `0`
- Summary: existing redaction-focused tests passed.

- Command: `cargo test -p lifecycle`
- Exit code: `0`
- Summary: full lifecycle test suite passed with 25 tests.

- Command: `cargo fmt -p lifecycle -- --check`
- Exit code: `0`
- Summary: lifecycle package formatting passed.

- Command: `cargo clippy -p lifecycle --all-targets -- -D warnings`
- Exit code: `0`
- Summary: clippy passed after replacing one iterator loop in `packages/lifecycle/src/terminal.rs`.

- Command: `cargo test -p lifecycle malformed`
- Exit code: `0`
- Summary: malformed credential-bearing URL regression test passed after the fail-closed redaction fix.

- Command: `cargo test -p lifecycle`
- Exit code: `0`
- Summary: full lifecycle test suite passed after the fail-closed redaction fix with 26 tests.

- Command: `cargo clippy -p lifecycle --all-targets -- -D warnings`
- Exit code: `0`
- Summary: clippy passed after the fail-closed redaction fix.

- Command: `cargo test -p lifecycle format_timestamp`
- Exit code: `0`
- Summary: replacement validation after the malformed URL fix passed.

- Command: `cargo test -p lifecycle format_line`
- Exit code: `0`
- Summary: replacement validation after the malformed URL fix passed.

- Command: `cargo test -p lifecycle safe_message`
- Exit code: `0`
- Summary: replacement validation after the malformed URL fix passed.

- Command: `cargo test -p lifecycle malformed`
- Exit code: `0`
- Summary: replacement validation confirmed malformed credential-bearing URL text no longer leaks userinfo.

- Command: `cargo test -p lifecycle redact_`
- Exit code: `0`
- Summary: replacement validation after the malformed URL fix passed.

- Command: `cargo test -p lifecycle`
- Exit code: `0`
- Summary: replacement validation full lifecycle suite passed with 26 tests.

- Command: `cargo fmt -p lifecycle -- --check`
- Exit code: `0`
- Summary: replacement validation formatting passed.

- Command: `cargo clippy -p lifecycle --all-targets -- -D warnings`
- Exit code: `0`
- Summary: replacement validation clippy passed.

## Focused commands

- `cargo test -p lifecycle format_timestamp`
- `cargo test -p lifecycle format_line`
- `cargo test -p lifecycle safe_message`
- `cargo test -p lifecycle redact_`

## Regression commands

- `cargo test -p lifecycle`
- `cargo fmt -p lifecycle -- --check`
- `cargo clippy -p lifecycle --all-targets -- -D warnings`

## Unavailable tooling

None.

## Refactors applied

- `packages/lifecycle/src/terminal.rs`: replaced a clippy-flagged `while let Some(ch) = chars.next()` iterator loop with `for ch in chars.by_ref()` inside ANSI CSI escape skipping.

## Reviewer decision

Blocked by `01_reviewer_01`.

- Finding severity: high.
- Finding summary: malformed credential-bearing URL text such as `https://user:pass@/path` can be emitted unchanged because `redact_credential_urls` falls back to the original candidate when `redact_repo_url` returns `MissingHost`.
- Required fix: add a package-level public API test proving malformed credential-bearing URL text is redacted before display, then make terminal-safe URL credential redaction fail closed for userinfo-bearing candidates.

Passed by replacement reviewer `01_reviewer_02`.

- Decision: PASS.
- Summary: prior malformed credential-bearing URL leak is fixed; effort 01 is scoped to lifecycle terminal/redaction/lib plus package-level tests; focused tests, malformed regression, full lifecycle suite, fmt, and clippy passed.
- Commit checkpoint: ready.

## Fix red evidence

- Command: `cargo test -p lifecycle malformed`
- Exit code: `1`
- Failure summary: expected assertion failure before the fix; `safe_message("cloning repo: https://user:pass@/path")` still contains `user:pass`.
