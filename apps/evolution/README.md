# Prompt evolution CLI

`evolution` initializes prompt workspaces and evolves the non-empty Markdown
(`.md`) or text (`.txt`) prompt in a configured workspace into an independent
prompt for every target model. The original prompt remains unchanged under
`default`.

## Usage

Build and run through Nx:

```sh
npx nx run evolution:run -- init ./prompts
npx nx run evolution:run -- evolve ./prompts/evolution.config.json
npx nx run evolution:run -- evolve ./prompts/evolution.config.json --dry-run
```

After installing the workspace binary, the equivalent direct command is:

```sh
evolution init [directory]
evolution evolve <config-path> [--dry-run]
```

- `init` resolves `[directory]` from the current working directory, defaulting
  to `.`, and creates only a missing `scenarios/` directory and
  `evolution.config.json`. Existing regular config files and real scenario
  directories are preserved. The JSON summary reports the resolved root and
  which entries were created.
- `<config-path>` is required and identifies the workspace through the
  configuration file's parent directory. Relative paths resolve from the
  current working directory, so the command can run from anywhere.
- `--dry-run` performs the same provider calls and in-memory evolution but
  creates, changes, and deletes no files or directories.

All evolve prompt, scenario, and model-output paths are resolved from the
config directory. The evolve command does not read or write relative to its
installation directory or the caller's current working directory.

After `init`, edit the `target-model`, `optimizer-model`, `judge-one`, and
`judge-two` placeholders for the models you want to use. You must also create
exactly one non-empty source prompt at `default/SYSTEM_PROMPT.md` or
`default/SYSTEM_PROMPT.txt`; `init` does not create the `default/` directory or
a prompt.

Progress is rendered by Pino and Pino Pretty on stderr. Prompt bodies and
credential values are not logged. The final machine-readable JSON summary is
the only output written to stdout. Invalid input, configuration, credentials,
or provider responses fail the command with a non-zero exit code.

## Configuration

All non-secret runtime configuration lives in the JSON file passed as
`<config-path>`, conventionally `evolution.config.json`. `evolution init`
creates a minimal OpenAI scaffold that uses `OPENAI_API_KEY` by name and stores
no credential value. A more detailed example follows:

```json
{
  "providers": [
    {
      "id": "local",
      "type": "lmstudio-openai",
      "baseUrl": "http://127.0.0.1:1234/v1"
    },
    {
      "id": "openrouter",
      "type": "openrouter",
      "tokenEnv": "OPENROUTER_API_KEY"
    }
  ],
  "models": [
    {
      "id": "lfm2.5-8b-a1b",
      "provider": "local",
      "model": "lfm2.5-8b-a1b",
      "effort": "none",
      "temperature": 0
    },
    {
      "id": "qwen3.5-4b",
      "provider": "local",
      "model": "qwen3.5-4b",
      "effort": "none",
      "temperature": 0
    }
  ],
  "optimizer": {
    "provider": "openrouter",
    "model": "openai/gpt-5.6-sol",
    "effort": "high",
    "temperature": 0
  },
  "judges": [
    {
      "provider": "local",
      "model": "lfm2.5-8b-a1b",
      "temperature": 0
    },
    {
      "provider": "local",
      "model": "qwen3.5-4b",
      "temperature": 0
    }
  ],
  "evolution": {
    "targetAccuracy": 0.9,
    "plateauPatience": 3,
    "maxEpochs": 20
  }
}
```

Provider `id` values are referenced by target models, the optimizer, and
judges. Target model `id` values are safe directory names and must be unique.
The supported provider types are:

| Type              | `llms` integration              | Credential                              |
| ----------------- | ------------------------------- | --------------------------------------- |
| `openai`          | OpenAI Responses                | Optional for compatible local endpoints |
| `openrouter`      | OpenRouter chat completions     | Required                                |
| `lmstudio`        | LM Studio native API            | Optional                                |
| `lmstudio-openai` | LM Studio OpenAI-compatible API | Optional                                |
| `codex`           | ChatGPT Codex Responses         | Required                                |

When a provider needs a credential, `tokenEnv` contains only the environment
variable name. The credential itself is resolved at runtime and must never be
stored in the JSON file. For example:

```sh
set OPENROUTER_API_KEY=...
```

`models` contains the prompts being optimized. Each model starts from the
original default prompt and evolves independently using only that model's
results; a weak model cannot select or reject another model's prompt. The
optimizer proposes prompt and optional scenario candidates during ordinary
target evolution. `judges` must contain at least two distinct `provider:model`
pairs and unanimously accept a generated scenario as correct and unambiguous
before it can be promoted.

`effort`, `temperature`, and `maxOutputTokens` are optional model request
controls. Omit unsupported controls for a provider; in particular, Codex does
not accept `temperature`.

## Scenarios

Scenarios are JSON files in `<config-directory>/scenarios`. They use a
prompt-agnostic contract:

```json
{
  "id": "scenario-01",
  "input": "Input sent to the prompt under test",
  "expected": "Expected output or behavior",
  "rationale": "Optional explanation for judges",
  "tags": ["optional", "metadata"]
}
```

The target model sees `input`, not `expected`, rationale, tags, or other test
metadata. Judges compare the resulting output with `expected`. Proposed
scenarios are rejected for duplicate IDs or normalized inputs and are promoted
only after unanimous, unambiguous judgment by all configured judges.

When the on-disk suite is empty, a shared scenario-initialization phase runs
before any target model is evaluated. On each attempt, the configured optimizer
generates a non-empty candidate batch from the immutable original prompt,
without target-specific context or a replacement prompt. Every candidate is
screened through the same duplicate checks and unanimous, unambiguous judge
validation used during evolution. If an entire batch is rejected, the next
attempt receives the attempted candidates, rejection reasons, and each judge's
decision and rationale. Initialization makes at most `plateauPatience`
attempts. The first batch with one or more accepted candidates supplies all of
its accepted scenarios as the shared baseline for every target. These attempts
do not consume target epochs or plateau state. A non-empty on-disk suite skips
this phase, and scenario additions remain optional during target evolution.

## Workspace layout

The directory containing `<config-path>` is the workspace root. An applied run
uses this layout:

```text
<config-directory>/
  evolution.config.json
  scenarios/
    scenario-01.json
    ...
  default/
    scenarios/
      scenario-01.json
      ...
    SYSTEM_PROMPT.md
  lfm2.5-8b-a1b/
    SYSTEM_PROMPT.md
  qwen3.5-4b/
    SYSTEM_PROMPT.md
```

Exactly one source prompt must exist: `default/SYSTEM_PROMPT.md` or
`default/SYSTEM_PROMPT.txt`; that prompt is immutable. On an applied run, the
CLI appends missing baseline scenario definition files—whether loaded or
initialized—to `default/scenarios` to establish an immutable baseline snapshot
and never overwrites files already there. The config file is also read-only.
The root `scenarios` directory contains that shared baseline plus unanimously
accepted target-evolution additions. Each `<model-id>/SYSTEM_PROMPT.md` is used
only by that configured target model. Writes occur only after all target
evolution completes successfully; `--dry-run` performs initialization and
evolution calls but does not create or change any part of the workspace.

Validate the app with:

```sh
npx nx show projects
npx nx run evolution:typecheck
npx nx run evolution:test
npx nx run evolution:build
```
