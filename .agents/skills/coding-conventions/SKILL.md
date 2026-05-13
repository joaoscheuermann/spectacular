---
name: coding-conventions
description: Shared design principles, Nx Python/Rust scaffolding references, and coding rules for Nx monorepos. Defines standards for architecture, SOLID principles, DRY, module depth, early returns, TS/JS functional style, and testing. Use to ensure codebase consistency.
---

# Coding conventions

Use this skill to ensure code and architectural designs adhere to our Nx monorepo standards. 

## Success criteria
- **Architecture/Planning**: The proposed design successfully implements SOLID principles, maintains deep modules with simple boundaries, and reuses existing code.
- **Implementation**: The delivered code respects file size limits (250-500 lines), uses early returns, applies explicit dependency injection, documents methods natively, and places tests in the correct folder structure.
- **Scaffolding**: New projects correctly implement the `@nxlv/python` or `@monodon/rust` templates and targets.

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
