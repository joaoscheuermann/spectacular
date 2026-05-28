# Handover

## Completed efforts

- `01_workspace_identity`: Renamed workspace package directories to `cli`, `agent`, `commands`, `config`, `llms`, `tools`, and `tui`; updated Cargo members, package names, Nx project names, source roots, and target dirs; configured `packages/cli` to build the `doric` binary.
- `02_rust_identity_refs`: Replaced active Rust crate imports, type names, command names, config directory names, debug log names, temp names, environment variables, and vendored IOCraft constants with Doric naming. Short crate names are absolute in CLI code where local modules share names with external crates.
- `03_docs_and_guidance`: Updated README and active repo guidance, and created `docs/architecture-and-packages.md` for the renamed package map.
- `04_validation_and_handover`: Regenerated `Cargo.lock`, ran Rust validation, scanned for stale old-name references, and documented validation gaps.

## Tests added or changed

- Updated existing tests that asserted product/package identity, including CLI parsing/config output, debug log startup, config type names, TUI banner/footer/path expectations, and hard-coded temp/workspace names.
- Updated TUI active-render banner parity assertions so Doric title text is padded to the fixed banner width.

## Test results

- PASS: `cargo metadata --format-version 1 --no-deps`
- PASS: `cargo build -p cli --bin doric`
- PASS: `cargo check --workspace --all-targets`
- PASS: `cargo fmt --all -- --check`
- PASS: `cargo clippy --workspace --all-targets -- -D warnings`
- PASS: `cargo test -p tui --test active_render_visual_parity -- --nocapture`
- PASS: `cargo test --workspace --all-features --no-fail-fast`
- PASS: `rg -n "spectacular|Spectacular|SPECTACULAR|spcetacular" --glob "!target/**" --glob "!dist/**" --glob "!node_modules/**" --glob "!.doric/**"`
- PASS: filename scan for `spectacular|Spectacular|SPECTACULAR|spcetacular`, excluding `.git`, `.doric`, `dist`, `target`, and `node_modules`
- BLOCKED: `npx nx show projects` because local Nx modules are not installed in the checkout.
- BLOCKED: `cargo nextest run --workspace --all-features` because `cargo-nextest` is not installed.

## Commit checkpoints

No commits were created; the user did not request commits. Suggested effort checkpoint subjects if committing later:

- `refactor: rename workspace packages for doric`
- `refactor: rename rust identity to doric`
- `docs: document doric architecture`
- `test: validate doric hard rename`

## Modified files

- Workspace metadata: `Cargo.toml`, `Cargo.lock`, package `Cargo.toml` files, and package `project.json` files.
- Package directories: `packages/spectacular` to `packages/cli`, `packages/spectacular-agent` to `packages/agent`, `packages/spectacular-commands` to `packages/commands`, `packages/spectacular-config` to `packages/config`, `packages/spectacular-llms` to `packages/llms`, `packages/spectacular-tools` to `packages/tools`, and `packages/spectacular-tui` to `packages/tui`.
- Product/runtime identity: Rust source and tests under `packages/cli`, `packages/agent`, `packages/commands`, `packages/config`, `packages/llms`, `packages/tools`, `packages/tui`, and `packages/vendor/iocraft`.
- Docs/guidance: `README.md`, `docs/architecture-and-packages.md`, `.agents/skills/current-architecture/SKILL.md`, `.agents/skills/iocraft/SKILL.md`, `.agents/skills/iocraft/references/iocraft-guide.md`.
- Doric run artifacts: prompt, PRD, TDD, effort files, and this handover under `.doric/05-2026/28/20260528-124001_rename_spectacular_to_doric/`.

## User decisions

- Hard rename only.
- No compatibility aliases, fallback config paths, old binary names, or old package prefixes.
- CLI entry package is `cli`; user-facing binary command is `doric`.

## Agent receipts

- Requirements, PRD, TDD, decomposition, and test-planning sub-agents were used during the run.
- No per-agent receipt files were present in the run directory; receipt details remain in the conversation transcript.

## Residual risks

- Nx validation remains unverified until JS dependencies are installed locally.
- `cargo nextest` remains unverified until `cargo-nextest` is installed.
- Git currently reports old package paths as deletions and new package paths as untracked because files were moved in the worktree but not staged.

## Follow-up work

- Install JS dependencies and run `npx nx show projects` plus the relevant Nx `lint`, `build`, and `typecheck` targets.
- Install `cargo-nextest` if CI requires it locally, then run `cargo nextest run --workspace --all-features`.
- Stage the directory moves before reviewing the final diff so Git can detect renames more clearly.
