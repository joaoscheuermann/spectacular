# Implementation Standards

Use these standards for all implementation work unless a language-specific reference narrows the rule for the files being changed. Apply the invariant intent everywhere, but use the idioms and tooling of the target language.

## 1. Test Location (Repository Invariant)

All tests must be placed in a dedicated `tests/` directory at the root of the relevant package, crate, or project segment.

- **Format:** `tests/<test_file_name>.<ext>`
- **TypeScript/JavaScript:** Prefer `tests/<unit-or-feature>.test.ts` or
  `.test.js` unless project tooling requires another suffix.
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
- Apply "Shameless Green" only as an intermediate step: hardcode or simplify temporarily to keep the feedback loop short, then remove temporary hardcoding during refactoring unless the hardcoded value is the intended behavior.

### Refactor: clean up safely

- Remove duplication, magic strings, unclear names, and structural flaws in production or test code while preserving behavior.
- Refactor incrementally and rerun the relevant tests after each meaningful change.
- Keep refactoring separate from feature work; do not add behavior during the refactor phase.

## 3. Unit Test Creation Rules

Use these rules for unit tests unless a language-specific framework imposes a stronger convention.

- **F.I.R.S.T.:** Tests should be fast, isolated/independent, repeatable without environmental state, and self-validating with a clear pass/fail result. Write them close to or before implementation.
- **Arrange-Act-Assert:** Arrange preconditions, mock data, and dependencies; act by triggering one method or behavior; assert actual outcomes against expected outcomes.
- **Naming:** Use framework-native names that make the behavior, scenario, and
  expected outcome clear. In TypeScript/JavaScript, prefer readable string test
  names in `test(...)` or `it(...)`, such as
  `writes one JSON line when append receives a record`; keep the unit under
  test in the file name or a `describe(...)` block. Avoid compressed
  underscore/camelCase names like
  `append_whenCalledWithRecord_shouldWriteOneJsonLine` in TS/JS tests. In
  identifier-based frameworks, preserve the same meaning in the language's
  idiomatic casing.
- **One logical concept:** Each test case should validate one scenario or concept so failures are easy to diagnose.

## 4. Unit Test Maintenance Rules

- Test behavior and public contracts instead of private methods, internal state variables, or implementation details.
- Manage external dependencies with mocks, stubs, fakes, or injected test doubles so unit failures identify local logic problems.
- Treat test code as production code: keep it formatted, readable, deduplicated, and supported by builders or helpers where setup is repeated.
- Quarantine flaky tests immediately, then fix or isolate them so the suite remains trustworthy.
- Run unit tests in CI/CD on every commit or pull request when the repository tooling supports it.

## 5. File Size Limits

Source files should target a size between **250 and 500 lines**, but 500 lines is the hard threshold.

- **< 250 lines:** Acceptable if cohesive. Do not pad artificially; 250 lines is not a minimum.
- **> 500 lines:** Refactor required. Split into smaller modules, extract helpers, or introduce a new sub-package based on distinct behaviors.

## 6. Early Returns and Guard Clauses

Keep control flow flat in the idiom of the language being edited. Guard clauses are the default expression of this rule in languages where early `return`, `throw`, or `raise` improves readability.

- Handle edge cases, errors, and invalid states at the top of a function.
- Return, throw, raise, or propagate errors immediately when that clarifies the path.
- Do not use an `else` block after a terminal branch unless the language construct makes the alternative clearer.
- For Rust-specific flat control-flow idioms, use `references/sexy-rust.md` instead of translating generic guard-clause advice mechanically.

## 7. Explicit Dependency Injection

Public APIs, constructors, entry points, and module seams should receive their dependencies explicitly rather than importing, constructing, or hiding them in global module state.

- Resolve clients, config, and tokens at the application entry point and pass them down explicitly.
- Prefer injecting dependencies once at constructors, factories, or composition roots. Keep routine public methods focused on domain inputs so deep modules remain small and semantic.
- When explicit dependencies would push a boundary past the repository argument threshold, group cohesive dependencies into a named dependency or configuration object. Do not hide dependencies in service locators, globals, or untyped bags.
- Let internal functions use dependencies owned by their module when that keeps the public interface small and stable.
- Avoid module-level shared mutable state or factories that capture dependencies in closures when a boundary parameter would be clearer.

## 8. Functional Programming Standard (TS/JS-Oriented)

Use this section only for TypeScript/JavaScript. For other languages, preserve the intent of small, predictable core logic, but do not force TS/JS collection or immutability idioms onto the language.

- **Strict Immutability:** Prefer `const` and structural sharing (`{...obj}`, `[...arr]`).
- **Pure Functions:** Isolate I/O, network, and mutation at the boundaries (e.g., `main.ts` or framework callbacks).
- **Scoped Mutation:** Local mutation is acceptable when it is contained, language-idiomatic, and improves clarity or performance without creating hidden side effects.
- **Declarative Flow:** Prefer `.map`, `.filter`, `.reduce` over imperative loops when readability wins.

## 9. Method and Function Documentation

Provide concise comments (1-3 sentences) above public APIs, core boundaries, and non-obvious methods or functions to explain purpose, parameters, return values, and side effects.

- Use native language standards: JSDoc (`/** */`) for TS/JS, docstrings (`""" """`) for Python, and `///` for Rust.
- Do not add boilerplate comments to obvious private helpers when the signature and name already explain the behavior.
