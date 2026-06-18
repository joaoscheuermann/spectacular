# Workflow Instructions

These instructions apply to workflow packages under `workflows/`.

## Purpose

Keep workflows pure and portable. A workflow should transform caller-provided
artifacts and options into returned artifacts without discovering this
repository's local instructions or documentation at runtime.

## Folder Structure Rules

- Put each workflow in one package directory at `workflows/<workflow-name>`.
  The directory name should be short, lowercase, and describe the workflow's
  responsibility, for example `workflows/prompt`.
- Treat each workflow directory as an Nx library package. Keep its
  `package.json`, `project.json`, `tsconfig.json`, `tsconfig.lib.json`, and
  `tsconfig.spec.json` at the workflow package root when TypeScript source or
  tests are present.
- Name workflow packages as `workflow-<workflow-name>` in package metadata.
  Set `sourceRoot` to `workflows/<workflow-name>/src` and expose the package
  through its public entrypoint, not private implementation files.
- Keep the public TypeScript surface in `src/index.ts`. Export the primary
  workflow function, artifact constructors, and public types from this file.
- Put the workflow coordinator in `src/lib/<workflow-name>.ts`. This file
  should compose the workflow passes and own the top-level artifact in/artifact
  out behavior.
- Put public workflow artifact and option types under `src/lib/types/`. Keep
  type files narrow and named after the workflow or the concept they model.
- Put pass-specific agent definitions and pass runners under
  `src/lib/agents/`. Prefer one file per pass or agent role, named for the role
  it performs.
- Put workflow-scoped tool adapters, tool safety rules, and tool storage
  builders under `src/lib/tools/`. Do not place general-purpose workspace tools
  here; those belong in reusable packages with public entrypoints.
- Put private helper functions under `src/lib/utils/`. Utilities must support
  this workflow's implementation rather than becoming an unowned shared
  toolbox.
- Put tests under `tests/` at the workflow package root. Tests should exercise
  the public workflow entrypoint and the workflow's runtime constraints, not
  private folder structure.
- Keep runtime prompt assets, templates, and pass text inside the same workflow
  package when they are workflow-specific. Codebase-specific facts, policies,
  or goals for those prompts must still arrive through workflow inputs.
- Do not import from another workflow's private `src/lib` files. If two
  workflows need the same behavior, first prove the second consumer and then
  extract the shared contract into an appropriate reusable package.

## Runtime Rules

- A workflow accepts artifacts and options as input and returns artifacts as
  output.
- Workflow runtime must not load `GROUNDING.md`, root or nested `AGENTS.md`
  files, `docs/`, README files, or other markdown guidance from this
  repository.
- Do not use a helper, package, or dependency to indirectly load those files or
  make workflow behavior depend on them.
- If policy, instruction, goal, or documentation content should affect a
  workflow, the caller must pass that content explicitly as an artifact or
  option.
- Workflow packages may import workspace or npm packages through public
  entrypoints. Those imports are allowed as code dependencies that compile or
  bundle into the workflow output.
- Runtime prompt assets may be imported from the same workflow package. Any
  codebase-specific facts or goals for those prompts must come from workflow
  inputs.
