# Direct Skill Planning Gated

This diagnostic runs every case in parallel through one fixed pipeline:

1. generate P0;
2. query and rerank skills independently for every P0 goal using
   `Objective + Goal`;
3. classify every retrieved `Goal + Skill` pair as `keep` or `drop` with a
   reason;
4. generate P1 from the kept skill bodies; and
5. report how many expected skills reached the final bundle.

Cases are JSON files in `cases/`. They contain only `name`, `objective`, and
`skills.expected`. Skill bodies are preloaded Markdown files in `cases/skills/`;
the runtime never downloads skills.

A skill reaches the final bundle when at least one goal keeps it. Skill names
are deduplicated before P1. The fixed per-goal retrieval limits are
`retrievalK = 20` and `topK = 10`. Set `OPENROUTER_API_KEY` and run:

```sh
npm run llm:direct-skill-planning-gated
```

Runtime logic lives in `index.mjs`, prompt text in `prompt.mjs`, message
construction in `utils.mjs`, and Zod contracts in `schemas.mjs`. All progress,
intermediate results, final bundles, plans, and metrics are appended as JSON
Lines to `output.log` in this directory.
