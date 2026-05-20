---
name: iocraft
description: Guidelines for building Rust CLI, TUI, and terminal output with the iocraft crate. Use when designing, implementing, reviewing, or testing declarative iocraft UIs, components, hooks, layouts, forms, render loops, and terminal interactions.
---

# IOCraft

Use this skill when working with the Rust `iocraft` crate for declarative terminal UI, CLI output, fullscreen TUIs, forms, progress displays, tables, and interactive terminal workflows.

## Success criteria
- **Declarative UI:** The solution represents terminal UI with `element!`, built-in components, and small custom `#[component]` functions rather than ad hoc terminal writes.
- **Component-first transcript/TUI rendering:** New or refactored TUI rows are rendered through meaningful IOCraft components; compatibility helpers that flatten domain state into `Vec<RenderLine>`/`Vec<String>` stay legacy-only and are not expanded.
- **Correct rendering mode:** Static output uses `print`, `eprint`, `write`, or `to_string`; dynamic inline UI uses `render_loop`; fullscreen apps use `fullscreen`.
- **Idiomatic components:** Components use `Props`, borrowed props where practical, stable `key` values for dynamic lists, and clear component boundaries.
- **Hook safety:** Hooks are called unconditionally in a stable order, with state, refs, effects, futures, context, and terminal events used for their intended purposes.
- **Terminal UX:** Keyboard input, focus, mouse capture, exit behavior, borders, colors, wrapping, and layout sizing are handled deliberately and tested where interaction matters.

## Retrieval & Stop Rules
Use the minimum evidence sufficient to implement or review the requested IOCraft work, then stop reading.
- Read `references/iocraft-guide.md` when you need IOCraft concepts, coding patterns, or best practices.
- Check upstream docs or examples only when a specific API detail is missing, version-sensitive, or materially affects correctness.
- Do not browse unrelated terminal UI crates unless the user explicitly asks for comparison or migration guidance.

## Decision rules
- Prefer existing IOCraft built-ins (`View`, `Text`, `MixedText`, `TextInput`, `Button`, `ScrollView`, `Fragment`, `ContextProvider`) before writing custom low-level components.
- Prefer simple component decomposition over clever abstractions; extract a component when it creates a meaningful UI boundary or reduces repeated layout code.
- For TUI transcript and row rendering, prefer existing `packages/spectacular-tui/src/components/*` components before adding rendering helpers; add or improve a component when the UI concept is missing.
- Treat `packages/spectacular-tui/src/format.rs::transcript_item_render_lines` as a legacy anti-pattern reference: do not copy its broad match-and-flatten shape, do not route new UI through it, and do not create shallow wrappers that only translate domain items into line vectors.
- Prefer borrowed props for domain data and owned state for interactive UI state.
- Prefer `use_state` only when changes should trigger rerendering; use `use_ref` or local variables for non-render-affecting mutable data.
- Prefer mock terminal tests for non-trivial keyboard, form, focus, or render-loop behavior.

## Reference index

### IOCraft Guide
[references/iocraft-guide.md](references/iocraft-guide.md)
Contains: core concepts, component patterns, layout, hooks, terminal events, render modes, Spectacular transcript rendering rules, context, forms, testing, and best practices.
