---
name: coding-conventions
description: Shared design principles, Nx Python/Rust scaffolding references, naming conventions, and coding rules for Nx monorepos. Defines standards for architecture, SOLID principles, DRY, context-driven identifier brevity, module depth, early returns, TS/JS functional style, and testing. Use to ensure codebase consistency.
---

# Coding conventions

Use this skill to ensure code and architectural designs adhere to our Nx monorepo standards. 

## Success criteria
- **Architecture/Planning**: The proposed design successfully implements SOLID principles, maintains deep modules with simple boundaries, and reuses existing code.
- **Implementation**: The delivered code respects file size limits (250-500 lines), uses context-driven naming, uses early returns, applies explicit dependency injection, documents methods natively, and places tests in the correct folder structure.
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
