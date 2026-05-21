---
name: coding-conventions
description: Shared Nx monorepo coding conventions with repository invariants and language-scoped implementation guidance for Rust, TypeScript/JavaScript, and Python. Use to apply architecture, testing, naming, scaffolding, and idiomatic language rules without treating language-specific polish as universal.
---

# Coding conventions

Use this skill to ensure code and architectural designs adhere to our Nx monorepo standards. Apply repository invariants to every language, then apply only the language-specific guidance that matches the files being designed, reviewed, or edited.

## Success criteria
- **Architecture/Planning**: The proposed design successfully implements SOLID principles, maintains deep modules with simple boundaries, and reuses existing code without speculative shared abstractions.
- **Implementation (All Languages)**: The delivered code keeps source files cohesive, refactors files over 500 lines, uses context-driven naming, keeps control flow flat in a language-native form, applies explicit dependency injection at appropriate boundaries, documents APIs natively where useful, and places tests in the correct folder structure.
- **Language-Specific Polish**: Rust code follows the Sexy Rust reference, including ownership-aware functional style where it improves clarity. TypeScript/JavaScript code follows the TS/JS functional guidance where it improves readability, and Python code follows the Python/Nx tooling conventions. Do not project Rust-only mechanics such as typestate, `?`, or conversion traits onto non-Rust code.
- **Testing**: New behavior is driven by focused unit tests where practical, follows Red-Green-Refactor, and keeps tests fast, isolated, repeatable, self-validating, and behavior-oriented.
- **Scaffolding**: New projects correctly implement the `@nxlv/python` or `@monodon/rust` templates and targets.

## Applicability
Determine the task's language and project type before choosing reference files.

- **Repository invariants apply broadly:** architecture principles, deep modules, context-driven naming, test placement, file-size limits, dependency boundaries, and native API documentation.
- **Rust-only implementation polish:** type-driven design, expression-oriented syntax, ownership-aware functional core, `?`, `Option`/`Result` combinators, pattern matching, iterators, typestate, conversion traits, `rustfmt`, and `clippy`.
- **TypeScript/JavaScript-only implementation polish:** immutability, pure core logic, and declarative array transforms when those patterns make the code clearer.
- **Python-specific guidance:** uv/Nx scaffolding, ruff formatting/linting, pytest targets, docstrings, and idiomatic Python APIs.
- **Mixed-language boundaries:** preserve the invariant intent, but express it in the idioms of the target language instead of translating another language's syntax or design pattern mechanically.

## Naming: Context-Driven Brevity (Repository Invariant)
When generating or refactoring code, prefer the shortest clear identifier that relies on available hierarchical context. This applies broadly to file names, variable names, function/method names, class names, struct names, modules, and namespaces.

- Do not add prefixes or suffixes that duplicate the containing folder, module, namespace, class, struct, or enclosing function.
- Use the parent structure to carry domain context; reserve the identifier for the distinguishing concept.
- Brevity is valid only when the missing words are clearly present in the parent context. If removing context makes the name ambiguous to a local reader, keep the clarifying term.
- During refactors, shorten redundant names when the surrounding path or scope already supplies the repeated meaning.
- Test case names are exempt from this brevity rule when the repository-standard scenario naming format requires explicit method, scenario, and expected behavior terms.

### Examples

#### Files (Rust)
```text
# Redundant
transcript/transcript_user_prompt.rs
transcript/user_prompt.rs

# Preferred
transcript/user.rs
```

#### Functions and methods (TypeScript)
```ts
// Redundant: `AuthTokenService` already provides auth/token context.
class AuthTokenService {
  validateAuthToken(authToken: string) {}
  refreshAuthToken(authToken: string) {}
}

// Preferred
class AuthTokenService {
  validate(value: string) {}
  refresh(value: string) {}
}
```

#### Variables and structs (Rust)
```rust
// Redundant: the `transcript` module already supplies transcript context.
mod transcript {
    struct TranscriptUser {
        transcript_user_id: String,
    }
}

// Preferred
mod transcript {
    struct User {
        id: String,
    }
}
```

#### Classes and variables (Python)
```python
# Redundant: the package/module path is `billing/invoice.py`.
class BillingInvoice:
    def send_billing_invoice(self, billing_invoice_id: str): ...

# Preferred
class Invoice:
    def send(self, id: str): ...
```

## Testing Practices

Use Test-Driven Development for new or changed behavior when practical. Keep the feedback loop short and avoid speculative implementation.

### Red-Green-Refactor
- **Red:** Design APIs from the caller's perspective, write the smallest failing test for one new behavior, and confirm it fails for the expected assertion or contract reason rather than syntax, setup, or environment errors.
- **Green:** Write the minimum implementation needed to pass. Use "Shameless Green" when helpful, including simple hardcoded values, to preserve a fast feedback loop and avoid future-feature work.
- **Refactor:** Clean duplication, magic strings, naming issues, and structural flaws while the tests stay green. Refactor in small increments, run tests after each meaningful change, and do not add features or alter behavior during refactoring.

### Unit Test Creation Rules
- Apply F.I.R.S.T.: tests should be fast, isolated/independent, repeatable without network/databases/system-clock dependence, self-validating with pass/fail outcomes, and timely relative to implementation.
- Structure tests with Arrange-Act-Assert: set up preconditions and dependencies, trigger one behavior, then assert expected outcomes.
- Name tests with `MethodUnderTest_Scenario_ExpectedBehavior`, adapting casing to the language while preserving the three-part meaning. This explicit scenario format overrides context-driven brevity for test case names.
- Verify one logical concept per test so failures identify a single scenario.
- Keep tests in the repository-standard `tests/` directory described in `references/implementation-standards.md`.

### Unit Test Maintenance Rules
- Test behavior through public APIs and contracts, not private methods or internal state details.
- Isolate external dependencies with mocks, stubs, fakes, or injected test doubles so failures point to local unit logic.
- Treat test code as production code: keep formatting, names, helpers, builders, and fixtures clean and maintainable.
- Quarantine flaky tests immediately and resolve or isolate them so the suite remains trusted.
- Integrate unit tests into CI/CD so they run on every commit or pull request where project tooling supports it.

## Retrieval & Stop Rules
- First identify whether the task is architecture, implementation, testing, scaffolding, or language-specific polish.
- Read only the reference files that match that task and language. For example, read Sexy Rust only for Rust code, and read the Python or Rust scaffolding pages only when creating or changing those project targets.
- **Do not read every file.** Use the minimum evidence sufficient to understand the applicable standard, then stop reading.
- Assume standard industry practices for anything not explicitly covered in these docs. Make another retrieval call only if the missing context would materially change the code or create meaningful risk.

## Reference index

### Architecture & Design
[references/architecture-principles.md](references/architecture-principles.md)
Contains: SOLID Principles (SRP, OCP, DIP, ISP), KISS, DRY/Reusability, and Deep vs. Shallow Modules.

### Implementation Standards
[references/implementation-standards.md](references/implementation-standards.md)
Contains: repository-wide test locations, file-size limits, language-native flat control flow, dependency injection, method documentation, and TS/JS-specific functional style.

### Nx Scaffolding
- [Python setup (@nxlv/python)](references/nxlv-python.md) - uv-based projects, `project.json` targets.
- [Rust setup (@monodon/rust)](references/monodon-rust.md) - Crates in Nx, `project.json` targets.

### Language-Specific Rust Readability
[Sexy Rust](references/sexy-rust.md)
Contains: Rust-only type-driven design, expression-oriented syntax, ownership-aware functional style, flat control flow with `?` and combinators, pattern matching and destructuring, zero-cost iterators, typestate, conversion traits, rustfmt, and clippy.
