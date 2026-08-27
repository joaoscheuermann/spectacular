# Direct Skill Planning

This minimal lab isolates the planning hypothesis behind a simpler Mosaic
pipeline: can one model call decompose the original request correctly when it
receives the full bodies of skills retrieved directly from that request?

The scaffold currently compares:

- a request-only plan;
- a direct plan using the same request plus a fixed set of retrieved skills.

It intentionally has no retrieval implementation, hints, P1 revision, judge,
rounds, or checkpointing. The fixed skills in `case.mjs` stand in for request-only
top-K retrieval so planning can be evaluated before retrieval becomes another
experimental variable.

## Usage

```sh
npm run llm:direct-skill-planning
```

The command prints both structured plans as JSON. Edit `case.mjs` to change the
request or the simulated retrieval result. It calls the configured paid model
and requires `OPENROUTER_API_KEY`.
