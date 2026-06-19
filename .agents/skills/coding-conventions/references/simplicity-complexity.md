# Simplicity & Complexity

Use this reference when designing, reviewing, or refactoring code where cognitive load, module boundaries, control-flow complexity, side effects, or decomposition decisions could materially affect maintainability.

## Goal

Complexity is the compounding tax paid on every future change. Simplicity is the deliberate practice of minimizing cognitive load so code can be understood, tested, and changed safely.

Aggressive simplicity means unjustified structure does not get the benefit of the doubt. If a proposed code path, abstraction, dependency, package, configuration option, or layer does not prove that it solves a present need better than the simpler alternatives, do not add it.

## Core Concepts

### Simplicity vs. Easiness

Do not confuse simple with easy:

- **Simple:** unentangled, focused on one concept, and free of systemic side effects. Simplicity is an architectural property.
- **Easy:** familiar, nearby, or quick to implement right now.

Choosing easy over simple often introduces hidden coupling, global mutable state, or shortcut abstractions that make future changes harder.

### Minimality Enforcement Ladder

Apply this ladder before adding code, keeping dead code, introducing an abstraction, adding a dependency, splitting a package, creating a config surface, or inserting another layer of indirection. Stop at the first rung that satisfies the current requirement:

1. **Does this need to exist now?** If the need is speculative, skip it.
2. **Does current repository code already do this?** Reuse or extend the existing boundary before creating a sibling.
3. **Does the standard library or native platform do this correctly?** Prefer language, runtime, browser, database, operating-system, and Nx features over custom code.
4. **Does an already-installed dependency solve it?** Use existing dependencies before adding another one, but only when the dependency API stays readable at the call site.
5. **Is direct code clearer than an abstraction?** A small focused function beats an interface, factory, registry, or framework layer when there is only one real behavior.
6. **Only then add the minimum new structure.** The new code must have a current caller, a current test or contract, and a named reason that cannot be satisfied by a higher rung.

This ladder is an enforcement gate, not a research project. If two rungs both work, choose the higher rung. If the higher rung is weaker on correctness, safety, or edge cases, choose the smallest lower rung that is correct and document the reason when it matters.

### Aggressive Anti-Patterns

Treat these patterns as findings during review unless the code shows a current, concrete reason:

- An interface, abstract class, strategy map, or registry with one implementation and no proven second implementation.
- A factory with one product or a builder that only mirrors a constructor or object literal.
- A configuration option for a value that never changes in current requirements.
- A pass-through module, service, adapter, or helper that hides less complexity than its public interface adds.
- A new package split whose only justification is folder organization, future reuse, or an old out-of-scope architecture name.
- A new dependency for behavior that the standard library, platform, current repository code, or a few clear lines already cover.
- Boilerplate, scaffolding, extension points, lifecycle hooks, or generated-looking structure added for "later."

Prefer deletion or direct code over preserving a weak abstraction. When deliberately keeping a simple implementation with a known ceiling, name the ceiling and the upgrade trigger in a short comment or nearby documentation so future readers know the limit is intentional.

### Correctness Floor

Minimality cannot remove the checks that make code safe and trustworthy. Do not simplify away input validation at trust boundaries, error handling that prevents data loss, authorization or secret-handling safeguards, accessibility basics, or tests for non-trivial logic. Small code still needs the narrowest reliable proof that it works.

### Deep vs. Shallow Modules

Complexity is driven by dependencies and obscurity. Prefer deep modules that provide substantial value behind small, stable, semantic interfaces.

- **Deep module:** hides implementation complexity behind a minimal interface. Example: a provider client that owns auth, retries, and rate limits.
- **Shallow module:** exposes interface complexity similar to, or greater than, the implementation value it provides. Example: a service layer that only forwards calls one-to-one.

Pull complexity downward into modules to simplify common caller behavior. Avoid public helpers whose documentation is longer than their implementation.

### Cost of Cleverness

Prefer predictable, skimmable code over clever code. Dense one-liners, language tricks, implicit behavior, or overly generic abstractions can optimize writing at the expense of reading and debugging.

Code is read far more often than it is written. Optimize for the next reader.

## Complexity Metrics

Use metrics as refactoring signals, not as substitutes for judgment.

### Cyclomatic Complexity

Cyclomatic complexity measures the number of linearly independent execution paths through code. Each conditional or branching construct such as `if`, `while`, `for`, `case`, and `catch` generally adds one path.

High cyclomatic complexity means more paths to test and more ways behavior can interact unexpectedly.

### Cognitive Complexity

Cognitive complexity measures how difficult code is for a human to understand.

It is especially affected by:

- nested structural breaks, such as loops inside loops inside conditionals;
- control flow that interrupts top-to-bottom reading, such as jumps, breaks, or recursion;
- implicit behavior that requires outside knowledge to predict.

Standard language idioms are acceptable when they improve clarity for that language.

## Human Evaluation Checklist

Use this checklist as diagnostic guidance when evaluating whether code preserves systemic simplicity. Checklist failures are signals to inspect the design, not automatic proof that code must be split.

- **The Existence Test:** Is there a current caller, current contract, current behavior, current risk, or current test that requires this code to exist now?
- **The Standard Library Test:** Would existing repository code, the language standard library, the platform, or an already-installed dependency solve this with less ownership burden?
- **The Abstraction Value Test:** Does this abstraction hide real complexity or reduce caller burden, rather than merely renaming direct code?
- **The "And" Test:** Can the purpose of this file, class, or function be fully described in one sentence without using the word "and"? Use this as a responsibility-boundary signal; it does not require splitting cohesive lifecycle operations that belong behind one stable module interface.
- **The Blast Radius Test:** If this module's internals change, will callers outside the module remain unaffected?
- **The State Mutation Test:** Can this logic execute without changing global data, mutating parameters, or mutating local values across multiple scopes? Local mutation is acceptable when it is scoped, language-idiomatic, and improves clarity or performance without creating hidden side effects.
- **The Predictability Test:** Does the function do exactly what its name says and nothing surprising?
- **The Pure Logic Test:** Is pure data transformation separated from side-effect-heavy operations such as database transactions, network requests, or filesystem access?

## Refactoring Triggers

Treat these thresholds as repository invariants unless a task explicitly requires an exception and the tradeoff is documented.

| Metric                    | Threshold                             | Refactoring Action                                                                                                                      |
| ------------------------- | ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| **Function Length**       | More than 25 lines of executable code | Extract focused helpers or smaller operations.                                                                                          |
| **Nesting Depth**         | More than 3 layers deep               | Flatten control flow with guard clauses, early returns, or language-native equivalents.                                                 |
| **Function Arguments**    | More than 3 parameters                | Group related values into a domain object, struct, DTO, dependency object, or configuration object while keeping dependencies explicit. |
| **Cyclomatic Complexity** | Score greater than 8                  | Split branches into smaller operations, strategies, or polymorphic behavior.                                                            |
| **File Size**             | More than 500 lines                   | Split along clear domain boundaries.                                                                                                    |

## Structural Triggers

Refactor when any of these patterns appear:

- **Leaky Abstraction:** A caller must import a third-party, database-specific, or infrastructure-specific type only to pass values into another module. Accept primitives or clean domain abstractions at the boundary instead.
- **Speculative Abstraction:** A new interface, factory, config object, registry, package, or extension point exists for future variation that has no current caller or proven second implementation.
- **God Object:** One class or module handles multiple lifecycles, such as parsing configuration, processing business logic, and producing output. Split it into single-responsibility units.
- **Implicit Side Effects:** A function modifies state outside its own scope or mutates its inputs unexpectedly. Prefer a pure function that accepts data and returns a new result.
