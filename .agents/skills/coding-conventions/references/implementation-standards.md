# Implementation Standards

## 1. Test Location (Repository Invariant)

All tests must be placed in a dedicated `tests/` directory at the root of the relevant package, crate, or project segment.

- **Format:** `tests/<test_file_name>.<ext>`
- **Rule:** Do not create inline test cases in production source files. Do not place test files alongside source files (e.g., `libs/auth/src/login.spec.ts` is forbidden).
- **Rust Exception:** A minimal `#[cfg(test)] mod tests { include!(...) }` harness is permitted in source only to include a file from the `tests/` directory.

## 2. Test-Driven Development

Use Red-Green-Refactor for new or changed behavior when the project has an executable unit test path.

### Red: write a failing test

- Design from the outside in: shape APIs and interfaces from the caller's perspective to keep boundaries loosely coupled.
- Write only enough test code to fail for one new behavior or contract.
- Confirm the failure is expected and meaningful, such as an assertion failure or missing behavior, not syntax, dependency installation, fixture setup, or environment configuration.

### Green: make the test pass

- Write the minimum implementation needed for the current test.
- Avoid speculative development and unrequested future features.
- Apply "Shameless Green" when useful: hardcode or simplify temporarily to keep the feedback loop short, then improve during refactoring.

### Refactor: clean up safely

- Remove duplication, magic strings, unclear names, and structural flaws in production or test code while preserving behavior.
- Refactor incrementally and rerun the relevant tests after each meaningful change.
- Keep refactoring separate from feature work; do not add behavior during the refactor phase.

## 3. Unit Test Creation Rules

Use these rules for unit tests unless a language-specific framework imposes a stronger convention.

- **F.I.R.S.T.:** Tests should be fast, isolated/independent, repeatable without environmental state, and self-validating with a clear pass/fail result. Write them close to or before implementation.
- **Arrange-Act-Assert:** Arrange preconditions, mock data, and dependencies; act by triggering one method or behavior; assert actual outcomes against expected outcomes.
- **Naming:** Use `MethodUnderTest_Scenario_ExpectedBehavior`, adapting capitalization to the language or test framework. Example: `withdrawFunds_whenBalanceIsSufficient_shouldDeductAmount`.
- **One logical concept:** Each test case should validate one scenario or concept so failures are easy to diagnose.

## 4. Unit Test Maintenance Rules

- Test behavior and public contracts instead of private methods, internal state variables, or implementation details.
- Manage external dependencies with mocks, stubs, fakes, or injected test doubles so unit failures identify local logic problems.
- Treat test code as production code: keep it formatted, readable, deduplicated, and supported by builders or helpers where setup is repeated.
- Quarantine flaky tests immediately, then fix or isolate them so the suite remains trustworthy.
- Run unit tests in CI/CD on every commit or pull request when the repository tooling supports it.

## 5. File Size Limits

Source files should target a size between **250 and 500 lines**.

- **< 250 lines:** Acceptable if cohesive. Do not pad artificially.
- **> 500 lines:** Refactor required. Split into smaller modules, extract helpers, or introduce a new sub-package based on distinct behaviors.

## 6. Early Returns and Guard Clauses

Use the guard clause pattern over deep nesting of `if/else`.

- Handle edge cases, errors, and invalid states at the top of a function.
- Return or throw immediately. Do not use an `else` block after a `return`.

## 7. Explicit Dependency Injection

Functions should receive their dependencies as explicit arguments rather than importing, constructing, or hiding them in module state.

- Resolve clients, config, and tokens at the application entry point and pass them down explicitly.
- Avoid module-level shared mutable state or factories that capture dependencies in closures when simple parameters would suffice.

## 8. Functional Programming Standard (TS/JS-Oriented)

For core logic, transforms, and generators in TypeScript/JavaScript:

- **Strict Immutability:** Prefer `const` and structural sharing (`{...obj}`, `[...arr]`).
- **Pure Functions:** Isolate I/O, network, and mutation at the boundaries (e.g., `main.ts` or framework callbacks).
- **Declarative Flow:** Prefer `.map`, `.filter`, `.reduce` over imperative loops when readability wins.

## 9. Method and Function Documentation

Provide concise comments (1-3 sentences) above every method or function to explain its purpose, parameters, return values, and side effects.

- Use native language standards: JSDoc (`/** */`) for TS/JS, docstrings (`""" """`) for Python, and `///` for Rust.
