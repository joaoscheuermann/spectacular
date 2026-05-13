# @nxlv/python

`@nxlv/python` is an Nx plugin that manages Python projects using the **uv** package manager. It provides generators to scaffold new projects and executors for dependency management, building, linting, formatting, testing, and running commands inside an activated virtual environment.

## Generator

Scaffold a new Python project with uv:
`npx nx generate @nxlv/python:uv-project <name> [options]`

## Standard Executors

Use the canonical executor names; do not invent parallel conventions.

| Target    | Executor                    | Purpose                                         |
| --------- | --------------------------- | ----------------------------------------------- |
| `lock`    | `@nxlv/python:lock`         | Regenerate `uv.lock` without upgrading          |
| `sync`    | `@nxlv/python:sync`         | Sync venv with locked dependencies              |
| `add`     | `@nxlv/python:add`          | Add a dependency (`--name`, optional `--local`) |
| `update`  | `@nxlv/python:update`       | Upgrade a dependency                            |
| `remove`  | `@nxlv/python:remove`       | Remove a dependency                             |
| `install` | `@nxlv/python:install`      | Install project into venv                       |
| `build`   | `@nxlv/python:build`        | Build sdist/wheel                               |
| `lint`    | `@nxlv/python:ruff-check`   | Lint with ruff                                  |
| `format`  | `@nxlv/python:ruff-format`  | Format with ruff                                |
| `test`    | `@nxlv/python:run-commands` | Often `uv run pytest tests/`                    |

## Workspace Conventions

- **ruff** is the typical linter/formatter pair.
- Prefer Nx targets for dependency changes (`nx run <project>:add --name <dep>`) instead of ad-hoc `uv add` in a terminal to keep the graph and lockfile consistent.

## Canonical project.json Template

```json
{
  "name": "<project-name>",
  "projectType": "application",
  "sourceRoot": "<project-root>/<module>",
  "targets": {
    "lock": { "executor": "@nxlv/python:lock", "options": { "update": false } },
    "sync": { "executor": "@nxlv/python:sync" },
    "add": { "executor": "@nxlv/python:add" },
    "build": {
      "executor": "@nxlv/python:build",
      "outputs": ["{projectRoot}/dist"],
      "options": {
        "outputPath": "{projectRoot}/dist",
        "publish": false,
        "lockedVersions": true,
        "bundleLocalDependencies": true
      }
    },
    "lint": {
      "executor": "@nxlv/python:ruff-check",
      "options": { "lintFilePatterns": ["<module>", "tests"] }
    },
    "format": {
      "executor": "@nxlv/python:ruff-format",
      "options": { "filePatterns": ["<module>", "tests"] }
    },
    "test": {
      "executor": "@nxlv/python:run-commands",
      "options": { "command": "uv run pytest tests/", "cwd": "{projectRoot}" }
    }
  }
}
```
