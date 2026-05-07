# Test file location

This convention standardizes where all tests live in the repository, making discovery, execution, and tooling configuration predictable.

## Rule

**Any test** written for a module/package/crate must be placed under a dedicated `tests/` directory, using the naming convention:

```
tests/<test_file_name>.<ext>
```

- `<test_file_name>`: A descriptive name matching the functionality under test (e.g., `user-service.spec.ts`, `parser_integration_test.rs`).
- `<ext>`: The standard extension for the language and test harness in use (e.g., `.spec.ts`, `.test.js`, `_test.py`, `_test.rs`, `.spec.lua`).

## Guidance

1. **Dedicated `tests/` folder**: Place the folder at the root of the relevant package, crate, or project segment—not scattered alongside source files.
   - Good: `libs/auth/tests/login.spec.ts`
   - Bad: `libs/auth/src/login.spec.ts`
2. **Mirror structure when helpful**: Inside `tests/`, you may mirror the source tree if the package is large, but keep everything under `tests/`.
3. **No inline test logic in source files**: Do not embed test cases within production source files. Even language-native inline test blocks (e.g., `#[cfg(test)]` in Rust) should prefer external files unless they are trivial unit checks tightly coupled to private internals.
4. **One concern per file**: A single test file should target one module or behavior. If a test file itself grows past the file-size limit (see [file-size-limits.md](file-size-limits.md)), split it by sub-feature.
