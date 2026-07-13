# OKF package

`okf` generates an Open Knowledge Format representation of a repository. It
discovers the complete input snapshot before processing, respects scoped
`.gitignore` files, skips binary content, and processes text files in bounded
concurrent batches.

## API

```ts
import { defaultOutput, generate } from 'okf';

const result = await generate(
  {
    provider,
    model: 'model-id',
    effort: 'low',
    batchSize: 5,
  },
  repositoryRoot,
);

console.log(result.output === defaultOutput(repositoryRoot));
```

`generate(config, root, output?)` requires an injected `LlmProvider` and model.
The optional configuration fields are `effort`, `promptTarget`, `batchSize`,
`ignore`, and `signal`. The default output is
`<root>/.agents/bundles/project`. An explicit absolute or root-relative output
must be a child of `<root>/.agents/bundles`; the bundle root itself and paths
outside it are rejected.

The result reports the canonical root, output and index paths, represented
source paths, and generated versus cache-hit counts. Each concept mirrors its
source path, records a SHA-256 content hash, and is reused when that hash is
already present. The root `index.md` is regenerated deterministically.

## Prompt evolution

Classification and frontmatter are stage-level evolution workspaces. Analysis
has one complete system prompt per supported classification kind:

```text
prompts/
  classify/default/SYSTEM_PROMPT.md
  frontmatter/default/SYSTEM_PROMPT.md
  analyze/
    source_code/default/SYSTEM_PROMPT.md
    test/default/SYSTEM_PROMPT.md
    # ...one directory for every classification kind
```

For an evolution run, place an `evolution.config.json` and `scenarios/` beside
the relevant stage's `default/` directory as described by
`apps/evolution/README.md`. Provider and judge choices are intentionally not
preconfigured by this package. The evolution CLI writes a model prompt to
`<stage>/<model-id>/SYSTEM_PROMPT.md`, or
`analyze/<kind>/<model-id>/SYSTEM_PROMPT.md` for analysis. Select the common
model target at runtime with `promptTarget: '<model-id>'`. Omitting
`promptTarget` uses `default`. Classification and frontmatter targets are
loaded before file processing; the matching analysis target is loaded after
classification and never falls back to another kind or model.

Per-file evidence is sent as a JSON user message. System instructions remain
in the prompt files so evolution evaluates the same prompt used in production.
