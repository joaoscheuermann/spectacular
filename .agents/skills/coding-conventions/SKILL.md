---
name: coding-conventions
description: Shared design principles, Nx Python/Rust scaffolding references, naming conventions, and coding rules for Nx monorepos. Defines standards for architecture, SOLID principles, DRY, context-driven identifier brevity, module depth, early returns, TS/JS functional style, and testing. Use to ensure codebase consistency.
---

# Coding conventions

Use this skill to ensure code and architectural designs adhere to our Nx monorepo standards. 

## Success criteria
- **Architecture/Planning**: The proposed design successfully implements SOLID principles, maintains deep modules with simple boundaries, and reuses existing code.
- **Implementation**: The delivered code respects file size limits (250-500 lines), uses context-driven naming, uses early returns, applies explicit dependency injection, documents methods natively, and places tests in the correct folder structure.
- **Testing**: New behavior is driven by focused unit tests where practical, follows Red-Green-Refactor, and keeps tests fast, isolated, repeatable, self-validating, and behavior-oriented.
- **Scaffolding**: New projects correctly implement the `@nxlv/python` or `@monodon/rust` templates and targets.

## Naming: Context-Driven Brevity (Repository Invariant)
When generating or refactoring code, prefer the shortest clear identifier that relies on available hierarchical context. This applies broadly to file names, variable names, function/method names, class names, struct names, modules, and namespaces.

- Do not add prefixes or suffixes that duplicate the containing folder, module, namespace, class, struct, or enclosing function.
- Use the parent structure to carry domain context; reserve the identifier for the distinguishing concept.
- Brevity is valid only when the missing words are clearly present in the parent context. If removing context makes the name ambiguous to a local reader, keep the clarifying term.
- During refactors, shorten redundant names when the surrounding path or scope already supplies the repeated meaning.

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
- Name tests with `MethodUnderTest_Scenario_ExpectedBehavior`, adapting casing to the language while preserving the three-part meaning.
- Verify one logical concept per test so failures identify a single scenario.
- Keep tests in the repository-standard `tests/` directory described in `references/implementation-standards.md`.

### Unit Test Maintenance Rules
- Test behavior through public APIs and contracts, not private methods or internal state details.
- Isolate external dependencies with mocks, stubs, fakes, or injected test doubles so failures point to local unit logic.
- Treat test code as production code: keep formatting, names, helpers, builders, and fixtures clean and maintainable.
- Quarantine flaky tests immediately and resolve or isolate them so the suite remains trusted.
- Integrate unit tests into CI/CD so they run on every commit or pull request where project tooling supports it.

## Retrieval & Stop Rules
- Read the specific reference files below to gather context before implementing code or proposing architecture.
- **Do not read every file.** Use the minimum evidence sufficient to understand the standard, then stop reading.
- Assume standard industry practices for anything not explicitly covered in these docs. Make another retrieval call only if the missing context would materially change the code or create meaningful risk.

## Reference index

### Architecture & Design
[references/architecture-principles.md](references/architecture-principles.md)
Contains: SOLID Principles (SRP, OCP, DIP, ISP), KISS, DRY/Reusability, and Deep vs. Shallow Modules.

### Implementation Standards
[references/implementation-standards.md](references/implementation-standards.md)
Contains: Invariant Test Locations, File Size Limits, Early Returns, Functional Programming (TS/JS), Dependency Injection, and Method Documentation.

### Nx Scaffolding
- [Python setup (@nxlv/python)](references/nxlv-python.md) — uv-based projects, `project.json` targets.
- [Rust setup (@monodon/rust)](references/monodon-rust.md) — Crates in Nx, `project.json` targets.
