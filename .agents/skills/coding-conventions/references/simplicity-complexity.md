# Simplicity & Complexity

Use this reference when designing, reviewing, or refactoring code where cognitive load, module boundaries, control-flow complexity, side effects, or decomposition decisions could materially affect maintainability.

## Goal

Complexity is the compounding tax paid on every future change. Simplicity is the deliberate practice of minimizing cognitive load so code can be understood, tested, and changed safely.

## Core Concepts

### Simplicity vs. Easiness

Do not confuse simple with easy:

- **Simple:** unentangled, focused on one concept, and free of systemic side effects. Simplicity is an architectural property.
- **Easy:** familiar, nearby, or quick to implement right now.

Choosing easy over simple often introduces hidden coupling, global mutable state, or shortcut abstractions that make future changes harder.

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

- **The "And" Test:** Can the purpose of this file, class, or function be fully described in one sentence without using the word "and"? Use this as a responsibility-boundary signal; it does not require splitting cohesive lifecycle operations that belong behind one stable module interface.
- **The Blast Radius Test:** If this module's internals change, will callers outside the module remain unaffected?
- **The State Mutation Test:** Can this logic execute without changing global data, mutating parameters, or mutating local values across multiple scopes? Local mutation is acceptable when it is scoped, language-idiomatic, and improves clarity or performance without creating hidden side effects.
- **The Predictability Test:** Does the function do exactly what its name says and nothing surprising?
- **The Pure Logic Test:** Is pure data transformation separated from side-effect-heavy operations such as database transactions, network requests, or filesystem access?

## Refactoring Triggers

Treat these thresholds as repository invariants unless a task explicitly requires an exception and the tradeoff is documented.

| Metric | Threshold | Refactoring Action |
| --- | --- | --- |
| **Function Length** | More than 25 lines of executable code | Extract focused helpers or smaller operations. |
| **Nesting Depth** | More than 3 layers deep | Flatten control flow with guard clauses, early returns, or language-native equivalents. |
| **Function Arguments** | More than 3 parameters | Group related values into a domain object, struct, DTO, dependency object, or configuration object while keeping dependencies explicit. |
| **Cyclomatic Complexity** | Score greater than 8 | Split branches into smaller operations, strategies, or polymorphic behavior. |
| **File Size** | More than 500 lines | Split along clear domain boundaries. |

## Structural Triggers

Refactor when any of these patterns appear:

- **Leaky Abstraction:** A caller must import a third-party, database-specific, or infrastructure-specific type only to pass values into another module. Accept primitives or clean domain abstractions at the boundary instead.
- **God Object:** One class or module handles multiple lifecycles, such as parsing configuration, processing business logic, and producing output. Split it into single-responsibility units.
- **Implicit Side Effects:** A function modifies state outside its own scope or mutates its inputs unexpectedly. Prefer a pure function that accepts data and returns a new result.
