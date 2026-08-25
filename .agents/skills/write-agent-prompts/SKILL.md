---
name: write-agent-prompts
description: Write or revise concise, self-contained system prompts for agents and LLMs. Use when drafting agent instructions, system messages, prompt templates, or prompt specifications, or when simplifying an existing prompt for clarity and scope.
---

# Write Agent Prompts

Produce the smallest system prompt that fully defines the target agent's required behavior.

## Success criteria

- State the desired outcome and necessary constraints in simple, direct, unambiguous language.
- Keep only instructions and context the target agent needs to perform its task.
- Make the prompt self-contained for the environment in which it will run.
- Avoid personas, fictional identities, character traits, and roleplaying.
- Include context the target agent cannot gather with its available tools; omit context it can reliably retrieve itself.
- Include a glossary only when non-trivial, overloaded, domain-specific, or project-specific wording requires definition.
- Remove repetition, motivational language, process narration, and decorative formatting.

## Decision rules

Define what the agent must accomplish before prescribing how to accomplish it. Add procedural steps only when order is required for correctness, safety, or interoperability.

Treat tools and supplied context as complementary:

- If the agent has a tool that can reliably retrieve current context, instruct it what evidence it needs without embedding that context.
- If the agent lacks such a tool, supply the minimum context necessary to act correctly.
- If tool availability is unknown, state the required context or retrieval capability explicitly instead of assuming it exists.

Limit the prompt to the target agent's responsibility. Exclude unrelated architecture, organizational background, downstream work, and explanations intended only for the prompt author.

Use a glossary only when a reasonable target agent could misinterpret a required term. Define each included term once, briefly, and in the meaning used by the prompt.

## Retrieval and stop rules

Inspect only the source material needed to identify the target outcome, constraints, tool access, missing context, and ambiguous terminology. Make another retrieval call only when one of those required facts is missing or conflicting.

Stop gathering context once the prompt can satisfy every success criterion. Draft the prompt, remove anything that does not change the target agent's behavior, and return the final prompt without commentary unless the user requests rationale or alternatives.
