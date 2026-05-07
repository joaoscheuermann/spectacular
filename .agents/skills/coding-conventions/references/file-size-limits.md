# File size limits

This convention enforces strict file size boundaries to keep modules readable, modular, and maintainable.

## Rule

Any source file **must** be between **250 and 500 lines**.

| Boundary | Action |
|----------|--------|
| < 250 lines | Acceptable if the module is complete and cohesive; do not pad artificially. |
| 250–500 lines | **Target zone.** |
| > 500 lines | **Refactor required.** Split into smaller modules, extract helpers, or introduce a new sub-package. |

## Guidance

1. **Split by responsibility**: If a single file exceeds 500 lines, look for distinct behaviors (e.g., parsing vs. transformation, business logic vs. I/O) and extract them into separate files.
2. **Prefer composition over large classes/objects**: Large objects often signal multiple responsibilities; break them into collaborating units.
3. **Keep cohesion high**: Shorter files (< 250 lines) are fine if they represent a single, well-defined concern. Do not merge unrelated logic just to hit a minimum.
4. **Tests and generated code**: This rule applies primarily to **hand-written source files**. Auto-generated boilerplate or snapshots may exceed the limit, but should be isolated in their own directories.
