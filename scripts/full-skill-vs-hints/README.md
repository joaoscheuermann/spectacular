# Full Skill vs. Hints

This experiment compares two revisions of the same request-derived goals:

- one revision receives the complete skill bodies;
- one revision receives model-extracted skill hints.

The option order is randomized. A human or a separate judge model evaluates the
options without receiving their treatment labels. Runs are checkpointed so paid
evaluations can resume after a failure.

## Completed Run

Run `ca713efe-e7ee-4023-b77c-6301251914c0` completed 18 evaluations with:

- full skill: 12;
- hints: 3;
- both: 3;
- neither: 0.

The local checkpoint remains at
`.llm-lab/runs/ca713efe-e7ee-4023-b77c-6301251914c0.json`.

## Usage

```sh
npm run llm:full-skill-vs-hints -- --judge
```

New checkpoints are written below
`.llm-lab/full-skill-vs-hints/runs`. An explicit `--resume` path can still load
checkpoints from the original `.llm-lab/runs` directory.
