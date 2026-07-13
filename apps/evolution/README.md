# Prompt evolution CLI

`evolution` evaluates and improves a workspace's Markdown (`.md`) or text
(`.txt`) system prompt independently for each configured target model. It uses
manually authored binary assertions, a single LLM judge, an isolated validation
split, and durable attempt history. The original prompt and evaluation suite
remain unchanged.

## Usage

Build and run through Nx:

```sh
npx nx run evolution:run -- init ./prompts
npx nx run evolution:run -- evolve ./prompts/evolution.config.json
npx nx run evolution:run -- evolve ./prompts/evolution.config.json --dry-run
```

After installing the workspace binary, the equivalent direct commands are:

```sh
evolution init [directory]
evolution evolve <config-path> [--dry-run]
```

- `init` resolves `[directory]` from the current working directory, defaulting
  to `.`, and non-destructively creates only a missing real `scenarios/`
  directory and credential-free `evolution.config.json` scaffold. It does not
  create a prompt or scenarios.
- `<config-path>` is required. Its parent directory is the workspace root, and
  a relative path resolves from the caller's current working directory.
- `--dry-run` performs provider calls and may read matching history, but creates,
  changes, and deletes no file or directory.

Before evolving, create exactly one non-empty source prompt at
`default/SYSTEM_PROMPT.md` or `default/SYSTEM_PROMPT.txt`, configure the models
and assertions, and author at least one training and one validation scenario.
An empty or incomplete suite fails before any provider call.

Progress is rendered by Pino and Pino Pretty on stderr. Prompt, scenario,
assertion, strategy, model-output, judge-reasoning, failure-evidence, and
credential bodies are not logged. The final machine-readable JSON summary is
the only stdout output. Invalid input, configuration, credentials, or provider
responses fail the command with a non-zero exit code.

## Configuration

All non-secret runtime configuration lives in the JSON file passed as
`<config-path>`, conventionally `evolution.config.json`. A representative
configuration is:

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
    }
  ],
  "optimizer": {
    "provider": "openrouter",
    "model": "openai/gpt-5.6-sol",
    "effort": "high"
  },
  "judge": {
    "provider": "local",
    "model": "lfm2.5-8b-a1b",
    "temperature": 0
  },
  "evals": [
    {
      "id": "output-contract",
      "assertion": "The output follows the required response schema and formatting contract."
    },
    {
      "id": "evidence-only",
      "assertion": "Every factual claim is supported by the supplied input."
    }
  ],
  "evolution": {
    "accuracy": 0.9,
    "patience": {
      "epochs": 3
    },
    "epochs": 20,
    "history": {
      "limit": 30
    }
  }
}
```

Provider `id` values are referenced by target models, the optimizer, and the
judge. Target model `id` values are safe, unique directory names. Supported
providers are:

| Type              | `llms` integration              | Credential                              |
| ----------------- | ------------------------------- | --------------------------------------- |
| `openai`          | OpenAI Responses                | Optional for compatible local endpoints |
| `openrouter`      | OpenRouter chat completions     | Required                                |
| `lmstudio`        | LM Studio native API            | Optional                                |
| `lmstudio-openai` | LM Studio OpenAI-compatible API | Optional                                |
| `codex`           | ChatGPT Codex Responses         | Required                                |

When a provider needs a credential, `tokenEnv` stores only the environment
variable name. The credential value is resolved at runtime and must not be
stored in the config.

`models` lists the prompts being optimized. Every target starts from the
immutable default prompt and evolves independently. `judge` is one model
reference. `evals` is the ordered set of global binary assertions applied to
every scenario; scenario-local assertions follow them.

`effort`, `temperature`, and `maxOutputTokens` are optional controls for target
and judge requests where their provider supports them. Optimizer temperature is
not configurable: normal and compression requests use `0.2`, and a plateau
escape uses `0.8`. An `optimizer.temperature` field is rejected. Codex optimizer
requests omit temperature and emit one warning on stderr.

`evolution.accuracy` is applied uniformly to training, compression
verification, and validation. `evolution.patience.epochs` controls how many
unsuccessful normal epochs precede a single plateau-escape attempt, while
`evolution.epochs` is a hard cap across attempts. `evolution.history.limit`
limits matching prior attempts supplied to the optimizer.

The config parser is strict. Legacy `judges` and `targetAccuracy` fields are
rejected rather than ignored.

## Scenarios and assertions

Each JSON file directly under `<config-directory>/scenarios` has this strict
shape:

```json
{
  "id": "scenario-01",
  "split": "train",
  "input": "Input sent to the prompt under test",
  "evals": [
    {
      "id": "local-rule",
      "assertion": "A binary requirement for this scenario"
    }
  ],
  "rationale": "Optional author metadata"
}
```

`split` must be `train` or `validation`, and the suite must contain at least one
scenario in each split. Global assertions are merged before local assertions.
Every effective scenario must have at least one assertion, and assertion IDs
must be unique within that effective scenario. `evals` may be empty only when
global assertions make the effective set non-empty. The strict loader rejects
legacy `expected` and `tags` fields.

The CLI does not initialize, propose, judge, accept, merge, snapshot, or write
scenarios. Authors own the suite. This prevents optimization from changing its
own evaluation contract.

## Evaluation and evolution

For every evaluated scenario, the target is called exactly three times. One
structured judge call receives the three outputs and all effective assertions
and must return exactly one result for every `(evalId, sampleIndex)` pair. Each
result contains `evalId`, `sampleIndex`, non-empty `reasoning`, and binary
`passed`. Missing, duplicate, unknown, or malformed results abort evaluation.

A scenario's accuracy is its passed verdicts divided by `3 × evals`. Overall
accuracy is the arithmetic mean of scenario accuracies, so scenarios remain
equally weighted even when they have different numbers of assertions.

Only training scenarios, current training failures, and the newest matching
history attempts are sent to the optimizer. A proposal contains only `prompt`
and `strategy`; a prompt equivalent to the incumbent and a previously failed
strategy are explicitly prohibited. A candidate is accepted only when it
strictly improves training accuracy. After `evolution.patience.epochs` misses at
`0.2`, one `0.8` epoch attempts to escape the plateau. Another miss stops
evolution; an improvement resets the optimizer to `0.2`. `evolution.epochs`
remains a hard cap.

When training first reaches `evolution.accuracy`, one compression proposal runs
at `0.2`. It is accepted only if its trimmed character count is 20–30 percent
shorter than the pre-compression prompt and it still reaches the training
threshold. Otherwise, the pre-compression prompt is retained.

The selected prompt is evaluated against validation exactly once. Validation
scenarios, outputs, reasoning, and failure details are never exposed to
optimization or compression. A target is approved only when validation reaches
the same accuracy threshold. Only approved prompt files are written. A failed
target preserves any existing prompt, and independent targets continue to be
processed.

The per-target summary reports `trainingAccuracy`, optional
`validationAccuracy`, `approved`, `refactored`, `epochsRun`, and a `stopReason`
of `approved`, `validation-failed`, `plateau`, or `max-epochs`.

## History and workspace layout

Applied runs append attempt and terminal records to
`<target-id>/evolution.history.jsonl`. Attempt records include the attempted
prompt, strategy, optimizer mode, training accuracy, failed evaluation
evidence, and disposition; terminal records capture the run's final status.
History is not rewritten.

History reuse is scoped by a SHA-256 fingerprint of the original prompt,
training contract, global assertions, accuracy threshold, target, and judge.
Only the newest `evolution.history.limit` matching attempts inform the
optimizer. A dry run may read this history but never appends it.

```text
<config-directory>/
  evolution.config.json
  scenarios/
    scenario-01.json
    scenario-02.json
  default/
    SYSTEM_PROMPT.md
  lfm2.5-8b-a1b/
    evolution.history.jsonl
    SYSTEM_PROMPT.md       # present only after approval
```

Exactly one default prompt must exist. The config, default prompt, and scenario
files are immutable during evolution. An applied unsuccessful run may append
history, but it does not replace or delete an existing target prompt.

Validate the app with:

```sh
npx nx show projects
npx nx run evolution:typecheck
npx nx run evolution:test
npx nx run evolution:build
```
