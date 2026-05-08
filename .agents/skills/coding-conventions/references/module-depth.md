# Deep and shallow modules

Open this file when defining module, package, class, service, or public API boundaries; when splitting a file; when adding a wrapper or adapter; or when a proposed layer mostly forwards calls.

## Meaning

- Treat every module as an interface plus an implementation.
- A deep module exposes a small, stable interface while hiding substantial behavior, knowledge, or implementation complexity.
- A shallow module exposes nearly as much interface complexity as the implementation complexity it hides.
- Module depth is a cost-benefit check: the benefit is useful behavior hidden behind the boundary; the cost is the interface every caller must learn.
- Prefer information-hiding boundaries over arbitrary "small class" or "small file" decomposition.

## Apply in a typical Nx monorepo

- Package boundaries should hide volatile decisions such as storage layout, provider APIs, schemas, generated artifacts, runtime details, and retry or error policy.
- App and CLI modules should expose product-level actions and results instead of leaking parser, transport, persistence, or UI plumbing.
- Adapters should translate and normalize dependency behavior; if an adapter only forwards one-for-one calls, it is probably shallow.
- Shared packages should own a stable abstraction, not just collect chronological steps from a workflow.
- Generated bindings, FFI surfaces, and serialized contracts should stay narrow because every exported type becomes interface cost.

## Decision Rules

- Before adding a public function, type, trait, package, or layer, ask: "What knowledge or design decision does this hide from its callers?"
- Pull complexity downward into the module when it simplifies common caller behavior.
- Keep the common path simple even if the implementation must handle more cases internally.
- Split a module when the split hides independent volatile decisions or serves distinct client roles.
- Merge or remove a layer when it only renames calls, mirrors execution order, or forces callers to understand both sides of the boundary.
- Prefer a small number of deep operations over many shallow one-off methods, unless separate methods are needed for distinct caller roles or safety.
- Do not reduce implementation complexity by pushing sequencing rules, data-shape knowledge, or error recovery onto callers.

## Avoid

- "Manager", "coordinator", or "service" layers that mostly pass through to another module.
- Public helpers whose documentation would be longer than their implementation.
- Splitting code only to satisfy line-count instincts when the split creates new public concepts without hiding knowledge.
- Interfaces that mirror a database schema, provider SDK, or lower-level transport when callers need domain behavior.
- Temporal decomposition where modules correspond only to execution steps rather than hidden design decisions.

## Examples

- Prefer `SessionStore::resume(session_id)` over callers manually reading index files, resolving paths, decoding records, and handling missing sessions.
- Prefer a provider client that owns authentication, retries, rate-limit interpretation, and response normalization over a wrapper exposing raw HTTP details everywhere.
- Prefer direct calls over a `UserService` that only forwards every method to `UserRepository`.
- Prefer one command execution boundary returning a domain result over multiple public methods that require callers to coordinate parse, validate, execute, and render steps.

## Sources

- [John Ousterhout, "A Philosophy of Software Design"](https://www.web.stanford.edu/~ouster/cgi-bin/aposd.php)
- [David Parnas, "On the Criteria to Be Used in Decomposing Systems into Modules"](https://cacm.acm.org/research/on-the-criteria-to-be-used-in-decomposing-systems-into-modules/)
