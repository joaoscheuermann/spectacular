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
- **Testing:** Static output and interactive flows are developed with focused behavior tests where practical, using Red-Green-Refactor and deterministic IOCraft test utilities.

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

## Testing Practices

Use Test-Driven Development for IOCraft behavior when practical, especially for formatting, static rendering, input handling, focus movement, form validation, and render-loop exit behavior.

### Red-Green-Refactor
- **Red:** Design the component API from the caller's perspective, write the smallest failing test for one visible behavior, and confirm the failure is caused by the expected output or interaction mismatch rather than terminal setup or async test wiring.
- **Green:** Add the minimum component, prop, hook, or event-handling logic needed to pass. Prefer a quick deterministic pass over speculative abstractions.
- **Refactor:** Clean duplicated layout fragments, magic strings, unclear component names, and awkward props while keeping behavior unchanged. Run the relevant static render or mock-terminal tests after each meaningful change.

### Unit Test Creation Rules
- Apply F.I.R.S.T.: tests should be fast, isolated, repeatable, self-validating, and written close to or before the implementation.
- Use Arrange-Act-Assert: arrange props, mock terminal events, and dependencies; act with `to_string`, `write`, `render_loop`, or mock-terminal execution; assert stable user-visible output or state transitions.
- Name tests with `MethodUnderTest_Scenario_ExpectedBehavior`, adapted to Rust snake_case when needed while preserving the three-part meaning.
- Verify one logical UI behavior per test. For example, split static rendering, focus traversal, submission, and exit handling into separate scenarios.

### Unit Test Maintenance Rules
- Test public component behavior and terminal output rather than private hook state or implementation details.
- Use IOCraft static rendering tests for deterministic output and `mock_terminal_render_loop` with synthetic events for interactive behavior.
- Isolate external dependencies with mocks, stubs, fakes, or injected test doubles; do not require network, databases, wall-clock timing, or a real terminal.
- Treat test code as production code with clear fixtures, helpers, and builders for repeated component setup.
- Quarantine flaky terminal or async tests immediately, then fix timing/event assumptions before re-enabling them in the trusted suite.
- Keep IOCraft tests in the repository-standard `tests/` directory and include them in CI/CD where project tooling supports it.

## Reference index

### IOCraft Guide
[references/iocraft-guide.md](references/iocraft-guide.md)
Contains: core concepts, component patterns, layout, hooks, terminal events, render modes, Spectacular transcript rendering rules, context, forms, testing, and best practices.
