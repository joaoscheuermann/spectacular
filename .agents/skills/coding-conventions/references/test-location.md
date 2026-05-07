# Test file location

This convention standardizes where all tests live in the repository, making discovery, execution, and tooling configuration predictable.

## Rule

**ALL tests MUST be placed in a dedicated `tests/` directory** - this is non-negotiable and applies to every language, framework, and testing scenario without exception.

The required structure is:

```
tests/<test_file_name>.<ext>
```

- `<test_file_name>`: A descriptive name matching the functionality under test (e.g., `user-service.spec.ts`, `parser_integration_test.rs`).
- `<ext>`: The standard extension for the language and test harness in use (e.g., `.spec.ts`, `.test.js`, `_test.py`, `_test.rs`, `.spec.lua`).

**THIS IS MANDATORY**: No test files may exist outside a `tests/` directory under any circumstances. Agents must refuse to create, modify, or accept test files that violate this rule.

## Guidance

1. **Dedicated `tests/` folder is REQUIRED**: The folder MUST be at the root of the relevant package, crate, or project segment. Test files scattered alongside source files WILL NOT be permitted.
   - CORRECT: `libs/auth/tests/login.spec.ts`
   - FORBIDDEN: `libs/auth/src/login.spec.ts`
   - FORBIDDEN: `libs/auth/login.test.js`

2. **Mirror structure when helpful**: Inside `tests/`, you may mirror the source tree if the package is large, but ALL test files MUST remain under `tests/`.

3. **NO inline test cases in source files**: Under NO circumstances may test cases be embedded within production source files. For Rust, a minimal `#[cfg(test)] mod tests { include!(...) }` harness is permitted only when it includes a file from the package's `tests/` directory and contains no `#[test]`, `#[tokio::test]`, or assertions inline. Small `#[cfg(test)]` accessors or seams are allowed only when needed to exercise private behavior from files under `tests/`.

4. **One concern per file**: A single test file should target one module or behavior. If a test file itself grows past the file-size limit (see [file-size-limits.md](file-size-limits.md)), split it by sub-feature.

5. **Enforcement**: Agents MUST refuse any request to create test files outside of a `tests/` directory. If a user requests inline tests or tests alongside source files, the agent MUST decline and explain that all tests MUST be placed in a dedicated `tests/` directory.

## Compliance Check

Before creating or modifying any test file, verify:
- [ ] The file is located within a `tests/` directory
- [ ] The `tests/` directory is at the appropriate package/crate/project root level
- [ ] The file follows the naming convention: `tests/<descriptive_name>.<ext>`
- [ ] Rust source files contain no inline test cases; any source-side test harness only includes files from `tests/`
- [ ] No test files exist outside of `tests/` directories in the project

**Any test file found outside a `tests/` directory is a violation that MUST be corrected immediately.**
