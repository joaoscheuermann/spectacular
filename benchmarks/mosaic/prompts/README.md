# Frozen benchmark prompts

These English Markdown prompts are model-facing protocol artifacts. Baseline
conditions load them through `src/conditions/prompts.ts`; they are hashed as a
single prompt set for freeze provenance. Structured outputs are enforced by the
runtime boundary, not by embedding JSON in the outer user message.
