# Agent Instructions

Applies to `agents/*`. Follow the root `GROUNDING.md` and `AGENTS.md` first;
this file only adds local conventions for agent packages.

## Keep It Readable

- Prevent deep nesting. Add folders only when they make the code easier to
  scan.
- Keep most executor behavior in `executor.ts`.
- Write minimal, streamlined code.
- Prefer direct imperative control flow when it is more legible than a
  functional pipeline.
- Do not over-decouple. If logic is used once and reads clearly inline, leave
  it inline.
- Extract a helper only when it removes real duplication or names a meaningful
  concern.

## File Shape

- `index.ts` starts the server.
- The server file should stay minimal: wire config, routes, executor, and
  startup only.
- `executor.ts` contains only the main executor logic.
- A2A Agent Card definitions live in `card.ts`.
- Message object factories live under `lib/messages/`.
- Constant variables live under `lib/constants/`.
- Supporting logic lives in `lib/utils/<utility>.ts`.
