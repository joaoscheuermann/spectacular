# Architecture & Design Principles

## 1. SOLID Principles

### Single Responsibility (SRP)

- Group code that changes together for the same reason. Separate code that changes for different reasons.
- **Nx Monorepo Application:**
  - `apps/*`: UX, navigation, lifecycle, platform integration.
  - `packages/*` (bindings): Language boundary, generated bindings, marshaling.
  - `packages/*` (native): Latency-sensitive paths, inference, transforms.
  - `packages/*` (tooling): Artifact generation, ETL, validation.
  - `models/*`: Architecture, train loops, export.

### Open/Closed (OCP)

- Keep a design open for extension and closed for modification.
- Extend existing behavior through new Nx targets, additive scripts, new artifacts, or configuration inputs instead of editing many switch statements across stacks.

### Dependency Inversion (DIP)

- High-level policy must not depend directly on low-level details. Point source dependencies toward the policies that should survive library or vendor swaps.
- Flow goes: **apps → bindings/surface → runtime/tooling APIs**, not the reverse.
- Pass paths, credentials, and limits at initialization or call boundaries rather than using hidden globals.

### Interface Segregation (ISP)

- Keep clients from depending on methods or fields they do not use.
- Split unrelated concerns (e.g., export-time vs runtime-init) even when they mention the same product feature. Keep boundary operations (init, query, load, teardown) as separate entry points.

## 2. Simplicity & Reusability

### Keep It Simple, Stupid (KISS)

- Prefer the simplest design that works. Start from direct flows before building generic frameworks.
- Extend the existing workspace layout before adding a cross-cutting "platform" package.
- Defer generalization until a second concrete consumer validates the shape.

### Don't Repeat Yourself (DRY) & Reusability-First

- Keep schemas, IDLs, or proto definitions as the source of truth for generated code in multiple languages.
- **Workflow before writing new code:**
  1. Audit the workspace for similar logic.
  2. Extract logic that could serve a second caller.
  3. Move cross-app shared code into `packages/*` using workspace generators.

## 3. Deep and Shallow Modules

- **Deep module:** Exposes a small, stable interface while hiding substantial behavior, knowledge, or implementation complexity. (e.g., A provider client that owns auth, retries, and rate-limits).
- **Shallow module:** Exposes nearly as much interface complexity as the implementation complexity it hides. (e.g., A service layer that merely forwards calls 1-to-1).
- **Rule:** Pull complexity downward into the module to simplify common caller behavior. Avoid public helpers whose documentation is longer than their implementation.
