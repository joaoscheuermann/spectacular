# Technical Design Document

## Summary

Hard-rename the repository identity from Spectacular to Doric across Rust workspace metadata, Nx project metadata, source imports, public CLI naming, runtime storage paths, logs, tests, and documentation.

The rename is intentionally breaking. No `spectacular` package names, dependency aliases, binary aliases, config path fallbacks, environment variable aliases, or command compatibility shims should remain. Behavior stays unchanged except for visible names and storage path names.

Target package mapping:

| Current directory               | New directory       | Cargo package/crate | Nx project | Role                                    |
| ------------------------------- | ------------------- | ------------------- | ---------- | --------------------------------------- |
| `packages/spectacular`          | `packages/cli`      | `cli`               | `cli`      | CLI composition root and binary package |
| `packages/spectacular-agent`    | `packages/agent`    | `agent`             | `agent`    | Agent runtime                           |
| `packages/spectacular-commands` | `packages/commands` | `commands`          | `commands` | Slash-command parsing/metadata          |
| `packages/spectacular-config`   | `packages/config`   | `config`            | `config`   | Persisted configuration                 |
| `packages/spectacular-llms`     | `packages/llms`     | `llms`              | `llms`     | Provider traits/implementations         |
| `packages/spectacular-tools`    | `packages/tools`    | `tools`             | `tools`    | Built-in tools                          |
| `packages/spectacular-tui`      | `packages/tui`      | `tui`               | `tui`      | TUI state, reducer, rendering, runtime  |

The CLI package is named `cli`, but its installed binary must be `doric`.

## Current architecture context

The architecture document `docs/architecture-and-packages.md` is absent in this checkout, so the authoritative sources are the workspace and package manifests, README, CI workflow, and source imports.

Current architecture from manifests:

- Root `Cargo.toml` has workspace members under `packages/spectacular*`.
- Root `package.json` uses npm workspaces `packages/*`.
- `nx.json` enables `@monodon/rust`; each Rust package has a `project.json`.
- Each `packages/*/project.json` currently uses old Nx names, old `sourceRoot`, and `dist/target/<old-project-name>`.
- `.github/workflows/ci.yml` runs `cargo nextest run --workspace --all-features` and `npx nx run-many -t lint build typecheck`.
- `Cargo.lock` is checked in and currently contains package entries for `spectacular` and `spectacular-*`.

Current package boundaries:

- `spectacular` is the application composition root.
- `spectacular-agent` owns agent runtime, event flow, tool loop, compaction, queueing, retries, and cancellation.
- `spectacular-llms` owns provider traits, provider request/stream types, provider registry, debug logging, and OpenAI/OpenRouter support.
- `spectacular-tools` owns built-in tools and depends on the agent trait surface.
- `spectacular-tui` owns TUI state, actions, reducers, rendering, prompt composer, and IOCraft runtime glue.
- `spectacular-commands` owns slash-command parsing and command metadata.
- `spectacular-config` owns persisted config schema and config IO.
- `packages/vendor/iocraft` is excluded from the workspace but patched through Cargo and currently contains Spectacular-specific environment variables and temp log naming, so it is an active rename surface.

## Proposed design

Implement the rename as a coordinated metadata-first refactor.

1. Preflight collision check:
   - Verify `packages/cli`, `packages/agent`, `packages/commands`, `packages/config`, `packages/llms`, `packages/tools`, and `packages/tui` do not already exist.
   - Verify no existing Cargo package or Nx project already uses the target names.
   - Stop before edits if a collision exists.

2. Rename package directories:
   - Move old package directories to the target role-based names.
   - Keep `packages/vendor/iocraft` unchanged as a directory, but rename active Spectacular-specific constants inside it.

3. Update Cargo workspace and package metadata:
   - Root `Cargo.toml` workspace members become:
     - `packages/cli`
     - `packages/agent`
     - `packages/commands`
     - `packages/config`
     - `packages/llms`
     - `packages/tools`
     - `packages/tui`
   - Package `name` values become `cli`, `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.
   - Internal dependency keys and paths use target names directly. Do not preserve old dependency keys with `package = "..."`
     aliases.
   - `packages/cli/Cargo.toml` adds an explicit binary target:
     ```toml
     [[bin]]
     name = "doric"
     path = "src/main.rs"
     ```
   - Regenerate checked-in `Cargo.lock` after manifest changes.

4. Update Rust imports and crate references:
   - Replace old crate identifiers:
     - `spectacular_agent` -> `agent`
     - `spectacular_commands` -> `commands`
     - `spectacular_config` -> `config`
     - `spectacular_llms` -> `llms`
     - `spectacular_tools` -> `tools`
     - `spectacular_tui` -> `tui`
   - Update tests, examples, doc comments, prompt templates, and any string literals that represent product or package identity.
   - Rename public/internal old-brand types where the old product name is embedded, especially `SpectacularConfig` and `SpectacularConfigWire`, to `DoricConfig` and `DoricConfigWire`.
   - Avoid broad API redesign; this is a mechanical identity rename.

5. Update Nx metadata:
   - Each `project.json` name, `sourceRoot`, and target dir must use the new project name:
     - `packages/cli/project.json`: `name: "cli"`, `sourceRoot: "packages/cli/src"`, `dist/target/cli`
     - `packages/agent/project.json`: `dist/target/agent`
     - `packages/commands/project.json`: `dist/target/commands`
     - `packages/config/project.json`: `dist/target/config`
     - `packages/llms/project.json`: `dist/target/llms`
     - `packages/tools/project.json`: `dist/target/tools`
     - `packages/tui/project.json`: `dist/target/tui`
   - `cli` remains an application project and keeps its `run` target.
   - Library packages remain library projects.

6. Update user-facing naming:
   - CLI command name becomes `doric`.
   - Clap command metadata changes from `#[command(name = "spectacular")]` to `#[command(name = "doric")]`.
   - Help/about/config text uses `Doric`/`doric`.
   - Command examples use `doric` and `npx nx run cli:run`.
   - TUI banners, notices, title strings, prompts, and snapshots use Doric.

7. Update storage, debug, log, and temp names:
   - Config directory constant changes from `spectacular` to `doric`.
   - Platform paths become:
     - Windows: `%APPDATA%\doric\config.json`, `%APPDATA%\doric\sessions\*.jsonl`
     - macOS: `~/Library/Application Support/doric/...`
     - Linux: `$XDG_CONFIG_HOME/doric/...` or `~/.config/doric/...`
   - Session subdirectory remains `sessions`.
   - Model cache file remains `model-cache.json`.
   - Config file remains `config.json`.
   - Tool output trace subdirectory remains `tool-output`, now under the Doric config dir.
   - Debug log file changes from `spectacular-debug.log` to `doric-debug.log`.
   - Test temp dirs and temp file prefixes using `spectacular-*` change to `doric-*`.
   - TUI env vars hard-rename:
     - `SPECTACULAR_TUI_SELECTION_TEXT_COLOR` -> `DORIC_TUI_SELECTION_TEXT_COLOR`
     - `SPECTACULAR_TUI_SELECTION_BACKGROUND_COLOR` -> `DORIC_TUI_SELECTION_BACKGROUND_COLOR`
   - Vendored IOCraft Spectacular-specific env/log names hard-rename:
     - `SPECTACULAR_TUI_KEY_DEBUG` -> `DORIC_TUI_KEY_DEBUG`
     - `SPECTACULAR_TUI_FORCE_KEYBOARD_ENHANCEMENT` -> `DORIC_TUI_FORCE_KEYBOARD_ENHANCEMENT`
     - `spectacular-tui-key-debug.log` -> `doric-tui-key-debug.log`

8. Update docs:
   - Rewrite README product identity, command examples, local data paths, package table, build/test examples, and CI notes.
   - Create `docs/architecture-and-packages.md` because the expected architecture map is absent and this change alters workspace members, package names, package responsibilities, and config locations.
   - Update active repository skill docs that reference package paths or product naming, including `.agents/skills/current-architecture/**` and `.agents/skills/iocraft/**`.
   - The new architecture doc should describe observed current architecture after rename, not future intent.

## Data model

No persisted JSON schema change is required for config, model cache, or session records unless a field directly contains the old product name.

Storage path change:

- The application stops reading and writing the old Spectacular config directory.
- New reads and writes use the Doric config directory.
- No automatic copy, migration, fallback lookup, or warning path is added because the PRD requires a hard rename with no compatibility aliases.

Persisted file names retained:

- `config.json`
- `model-cache.json`
- `sessions/*.jsonl`

Persisted schema handling retained:

- Existing config schema validation and existing session event schemas remain behaviorally unchanged.
- Existing schema-version handling that is not product-name compatibility can remain.
- If a test fixture or serialized snapshot embeds old product text, update the fixture only where the old text is a visible identity string.

Source data model rename:

- `SpectacularConfig` becomes `DoricConfig`.
- `SpectacularConfigWire` becomes `DoricConfigWire`.
- Related comments and tests should refer to Doric configuration.
- Serialized JSON field names remain unchanged unless they explicitly include `spectacular`.

## API or interface contracts

CLI contract:

- Binary: `doric`
- Cargo package: `cli`
- Cargo command examples:
  - `cargo build -p cli --bin doric`
  - `cargo run -p cli --bin doric -- --help`
- Nx command examples:
  - `npx nx run cli:run`
  - `npx nx test cli`
- No `spectacular` binary or alias remains.

Rust workspace contract:

- Internal dependency keys use target crate names directly.
- Source imports use target crate identifiers directly.
- No compatibility dependency aliases like `spectacular-config = { package = "config", ... }`.
- Integration tests that use Cargo binary env vars must switch from `CARGO_BIN_EXE_spectacular` to `CARGO_BIN_EXE_doric`.

Environment variable contract:

- Only `DORIC_*` environment variables are documented and read.
- Old `SPECTACULAR_*` variables are not read as fallbacks.

Nx contract:

- Project names are `cli`, `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.
- Target output dirs are `dist/target/<new-project-name>`.
- CI and docs use new project names.

Architecture contract:

- `cli` remains the composition root and depends on `agent`, `commands`, `config`, `llms`, `tools`, and `tui`.
- `agent` may depend on `llms`.
- `tools` may depend on `agent`.
- `tui` may depend on `commands`.
- `commands`, `config`, and `llms` remain foundational libraries and should not depend on `cli`.

## Security and privacy

The rename does not introduce new secrets, authorization paths, network calls, or data exposure.

Security-relevant behavior:

- API keys remain stored as plain text in `config.json`; the path changes from the old app directory to the new Doric app directory.
- The hard rename intentionally avoids copying old config files, which also avoids silently moving secrets between app identities.
- Debug logs remain local files, renamed to Doric names.
- Old environment variables are not honored, so users must opt into any debug or selection-color behavior through new `DORIC_*` variables.

Risk controls:

- Do not add migration code that reads old Spectacular config paths.
- Do not add binary aliases that could hide which app identity is writing local data.
- Update docs to make the new storage paths explicit.

## Performance and operations

Runtime performance should be unchanged. The rename affects build metadata, path names, and string constants.

Operational impacts:

- Existing local `dist/target/spectacular*` build artifacts may remain on disk but are no longer target outputs.
- Existing installed `spectacular` binaries will not be updated in place by this repo rename.
- Existing user config under old Spectacular paths will not be read.
- Cargo and Nx caches may invalidate because package names, paths, and target dirs change.
- Checked-in `Cargo.lock` must be regenerated to avoid stale package entries.

No runtime stale-name scan should be added to the product. Stale-reference detection belongs to implementation validation.

## Testing strategy

Preflight checks:

```powershell
Test-Path packages\cli
Test-Path packages\agent
Test-Path packages\commands
Test-Path packages\config
Test-Path packages\llms
Test-Path packages\tools
Test-Path packages\tui
```

Each preflight path check must return false before package directory moves begin.

Metadata validation:

```powershell
cargo metadata --format-version 1 --no-deps
npx nx show projects
```

Focused package validation:

```powershell
cargo build -p cli --bin doric
cargo run -p cli --bin doric -- --help
cargo test -p cli
cargo test -p config
cargo test -p llms
cargo test -p agent
cargo test -p tools
cargo test -p tui
cargo test -p commands
```

Workspace validation:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
cargo nextest run --workspace --all-features
npx nx run-many -t lint build test
npx nx run-many -t lint build typecheck
```

CI parity:

- Keep `.github/workflows/ci.yml` behavior equivalent.
- If CI names explicit projects later, update them to new project names.
- Validate the current CI gates locally where tooling is available:
  - `cargo nextest run --workspace --all-features`
  - `npx nx run-many -t lint build typecheck`

Rename-focused tests:

- Update CLI parsing tests to invoke `doric`.
- Update help/config output assertions to expect Doric.
- Update config-dir tests to expect `doric`.
- Update debug-log startup tests to expect `CARGO_BIN_EXE_doric` and `doric-debug.log`.
- Update TUI snapshot/parity tests to expect Doric banners and new env var names.
- Update tests and examples for new crate imports.
- Add or keep coverage proving old env vars are not accepted if current tests already cover env-var parsing.

Stale-reference checks:

```powershell
rg -n "Spectacular|spectacular|SPECTACULAR|spcetacular" --glob "!target/**" --glob "!dist/**" --glob "!node_modules/**" --glob "!.doric/**"
rg -n -i "spectacular|spcetacular" --glob "!target/**" --glob "!dist/**" --glob "!node_modules/**" --glob "!.doric/**"
Get-ChildItem -Recurse -Force |
  Where-Object {
    $_.FullName -notmatch "\\.git\\|\\.doric\\|\\dist\\|\\target\\|\\node_modules\\" -and
    $_.FullName -match "spectacular|spcetacular"
  }
```

Any remaining matches must be documented with path, line, and reason. Expected allowed matches should be limited to historical Doric run artifacts or immutable third-party metadata. Active repo-controlled code, manifests, docs, lockfiles, tests, and vendored active patches should not retain old names.

## Rollout and migration

Recommended implementation order:

1. Confirm target package directory/project-name collisions do not exist.
2. Rename directories.
3. Update root Cargo workspace members.
4. Update package `Cargo.toml` names, dependency keys, paths, and `[[bin]]`.
5. Update `project.json` names, source roots, and target dirs.
6. Update Rust imports and old-brand type names.
7. Update runtime strings, config dir, env vars, debug logs, session/temp/log names.
8. Update tests, examples, prompt templates, and snapshots.
9. Update README and create `docs/architecture-and-packages.md`.
10. Regenerate checked-in `Cargo.lock`.
11. Run metadata, focused package, workspace, CI-parity, and stale-reference validation.

Migration policy:

- This is a hard rename.
- No old binary alias.
- No old config path fallback.
- No old env var fallback.
- No old Cargo dependency aliases.
- No old Nx project aliases.
- No command aliases.

User-facing migration consequence:

- Users must invoke `doric`.
- Users who need old settings must manually move or recreate config under the Doric config directory.
- Existing old sessions are not discovered unless manually moved into the Doric sessions directory.

## Risks

- Generic crate names such as `config`, `commands`, and `tools` can be confused with local modules or common external crate names. Mitigate by compiling after each package/import stage and using explicit imports where needed.
- Renaming every package at once can produce many compiler errors. Mitigate by changing manifests and imports mechanically, then validating package by package.
- `Cargo.lock` can be forgotten because package renames are metadata-only but lockfile entries are checked in. Regenerate and include it.
- Integration tests using `CARGO_BIN_EXE_spectacular` will fail until the explicit `doric` binary name and env var references are updated together.
- Vendored IOCraft contains active Spectacular env/log names outside normal package directories. Include it in old-name scans.
- Active `.agents` skills contain package paths and product references. Update current-architecture and IOCraft skill docs so future repo guidance does not point at old paths.
- README and test snapshots contain many product-name assertions. Update only identity expectations, not unrelated behavior.
- Absence of `docs/architecture-and-packages.md` means architecture documentation must be created rather than edited.
- Hard rename breaks existing users and automation. This is intentional per PRD, but should be called out in handover.
- External registry, credential, or release ownership constraints can delay publication under renamed package or binary names, but they should be documented as deferred release risks and must not stop repository-controlled metadata, docs, commands, and local rename work.

## Open questions

None blocking technical design.

Implementation should stop only if preflight detects a local package, project, or path collision. External publication ownership constraints should be documented as release risks while completing local repository-controlled rename work.
