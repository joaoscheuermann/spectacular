# Implementation Standards

## 1. Test Location (Repository Invariant)

All tests must be placed in a dedicated `tests/` directory at the root of the relevant package, crate, or project segment.

- **Format:** `tests/<test_file_name>.<ext>`
- **Rule:** Do not create inline test cases in production source files. Do not place test files alongside source files (e.g., `libs/auth/src/login.spec.ts` is forbidden).
- **Rust Exception:** A minimal `#[cfg(test)] mod tests { include!(...) }` harness is permitted in source only to include a file from the `tests/` directory.

## 2. File Size Limits

Source files should target a size between **250 and 500 lines**.

- **< 250 lines:** Acceptable if cohesive. Do not pad artificially.
- **> 500 lines:** Refactor required. Split into smaller modules, extract helpers, or introduce a new sub-package based on distinct behaviors.

## 3. Early Returns and Guard Clauses

Use the guard clause pattern over deep nesting of `if/else`.

- Handle edge cases, errors, and invalid states at the top of a function.
- Return or throw immediately. Do not use an `else` block after a `return`.

## 4. Explicit Dependency Injection

Functions should receive their dependencies as explicit arguments rather than importing, constructing, or hiding them in module state.

- Resolve clients, config, and tokens at the application entry point and pass them down explicitly.
- Avoid module-level shared mutable state or factories that capture dependencies in closures when simple parameters would suffice.

## 5. Functional Programming Standard (TS/JS-Oriented)

For core logic, transforms, and generators in TypeScript/JavaScript:

- **Strict Immutability:** Prefer `const` and structural sharing (`{...obj}`, `[...arr]`).
- **Pure Functions:** Isolate I/O, network, and mutation at the boundaries (e.g., `main.ts` or framework callbacks).
- **Declarative Flow:** Prefer `.map`, `.filter`, `.reduce` over imperative loops when readability wins.

## 6. Method and Function Documentation

Provide concise comments (1-3 sentences) above every method or function to explain its purpose, parameters, return values, and side effects.

- Use native language standards: JSDoc (`/** */`) for TS/JS, docstrings (`""" """`) for Python, and `///` for Rust.
