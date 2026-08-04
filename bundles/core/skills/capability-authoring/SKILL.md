---
name: mosaic-capability-authoring
description: Creates or revises MOSAIC bundle manifests, tool descriptors, and micro-skills while preserving the separation between reusable behavioral instructions and executable operations. Use when adding capabilities under skills/, tools/, or manifest.json.
allowed-tools:
  - tree
  - find
  - grep
  - edit
  - write
  - terminal
---

# MOSAIC Capability Authoring

## Purpose

Create or revise MOSAIC skills, tool descriptors, and bundle manifests so that agents gain reusable behavior without turning reasoning procedures into artificial APIs.

A correct MOSAIC capability keeps four roles separate:

- An **objective** states the result that must become true.
- A **skill** teaches a reusable procedure, criterion, or way of working.
- A **tool** exposes an executable operation with explicit input and observable output.
- The **model** interprets information, chooses actions, transforms content, and decides when the objective is complete.

Do not assume a one-to-one relationship between objectives, skills, and tools. One objective may use zero, one, or several skills and may call zero, one, or several tools.

## Use this skill when

- Creating a new MOSAIC bundle.
- Adding or revising a `SKILL.md` file.
- Adding or revising a tool descriptor under `tools/`.
- Registering tools or skills in `manifest.json`.
- Reviewing a bundle for duplicate, overly broad, empty, conflicting, or invalid capabilities.
- Converting a task-specific agent instruction into composable micro-skills over general-purpose tools.

## Do not use this skill when

- The request only requires using existing skills and tools.
- The requested behavior is already covered by an existing skill.
- A base tool and the model's ordinary reasoning are sufficient and no reusable procedure is being added.
- The user asked for runtime implementation logic rather than descriptors and instructional files.

## Expected bundle layout

Use this structure unless the repository defines a stricter convention:

```text
<bundle>/
├── manifest.json
├── skills/
│   └── <skill-name>/
│       └── SKILL.md
└── tools/
    └── <tool-name>.json
```

All paths recorded in `manifest.json` are relative to the bundle root.

## Authoring workflow

### 1. Inspect before changing anything

Use the narrowest inspection tools that can establish the current state:

1. Use `tree` to understand the bundle structure.
2. Use `find` to locate manifests, existing `SKILL.md` files, and tool descriptors.
3. Use `grep` to search for existing names, similar behaviors, tool references, and manifest entries.
4. Read enough surrounding content to preserve the repository's naming and schema conventions.

Do not create a new capability before checking whether an equivalent or overlapping capability already exists.

### 2. Classify the requested capability

Before writing files, decide whether the request requires a skill, a tool, both, or neither.

Create a **skill** when the missing capability is a reusable behavior, such as:

- selecting evidence before making claims;
- tracing dependencies through source files;
- applying exact-edit discipline;
- validating a change against explicit completion criteria;
- comparing alternatives under stated criteria.

Create a **tool** when the missing capability is an executable operation, such as:

- reading or writing a file;
- searching file contents;
- executing a shell command;
- calling an external service;
- returning structured data from an environment.

Create **both** only when a reusable procedure requires an operation that does not already exist.

Create **neither** when an existing base tool and normal model reasoning are sufficient. A skill is not required merely to permit a tool call, and a tool should not be created merely to package reasoning that the model can perform.

### 3. Define the behavioral and operational boundaries

For each proposed capability, write down:

- the result it helps an objective reach;
- the reusable behavior, if any;
- the concrete external operation, if any;
- the inputs and observable outputs;
- applicable and non-applicable situations;
- how completion can be recognized.

Reject designs that blur these boundaries.

Examples:

| Requirement | Correct representation | Incorrect representation |
|---|---|---|
| Replace exact text in a file | `edit` tool | A tool that decides how to refactor an application |
| Teach safe exact replacement and verification | `exact-text-editing` skill using `edit` | A skill named only `use-edit` |
| Summarize a financial report | A financial-analysis or evidence-grounding skill | `summarize_financial_report` tool containing model reasoning |
| Persist generated content | `write` tool, optionally guided by a file-writing skill | A workflow-specific tool that writes one particular document type |

### 4. Author the tool descriptor when an operation is missing

A MOSAIC tool descriptor must expose a stable operation. For descriptor-only bundles, no handler or implementation is required.

Use this shape:

```json
{
  "name": "tool-name",
  "description": "A precise description of the executable operation.",
  "inputSchema": {},
  "outputSchema": {}
}
```

Follow these rules:

1. Use a unique, canonical name.
2. Describe what the runtime executes, not the full reasoning procedure around it.
3. Express inputs and outputs as valid JSON Schema objects.
4. Make required fields explicit.
5. Make success, failure, truncation, partial results, or error information observable when relevant.
6. Do not rely on hidden arguments or undocumented ambient state.
7. Keep the operation general when the underlying capability is general.
8. Add aliases only when compatibility requires them; otherwise prefer one canonical field name.
9. Do not place planning, interpretation, comparison, summarization, or decision policy inside the descriptor.
10. Do not implement runtime logic when the request asks only for a schema descriptor.

A tool's semantics must remain stable when different skills use it. The skill changes the procedure; the tool continues to perform the same operation.

### 5. Author the micro-skill

A micro-skill is small in responsibility, not necessarily in number of steps. It must satisfy all four properties below:

1. **Coherent:** it teaches one identifiable behavioral concern.
2. **Reusable:** it applies beyond one prompt, fixture, repository, or workflow.
3. **Composable:** it can operate beside other skills without claiming exclusive control of the objective.
4. **Loadable in full:** its body is concise enough to be injected completely when selected.

Avoid both extremes:

- **Too broad:** a skill that discovers, reads, analyzes, decides, writes, persists, and reports an entire workflow.
- **Too empty:** a skill that only repeats a tool name or says to use a tool when needed.

#### Required frontmatter

Use the minimal MOSAIC profile:

```yaml
---
name: canonical-skill-name
description: Explains the behavior taught and the situations in which it is useful.
allowed-tools:
  - exact-tool-name
---
```

Rules:

- `name` is required and must be unique in the catalog.
- Prefer lowercase kebab-case unless the repository establishes another canonical format.
- `description` is required and is an initial retrieval signal. State both the behavior and its applicability.
- `allowed-tools` is optional. Omit it for a purely cognitive skill.
- Every item in `allowed-tools` must exactly resolve to a registered tool name.
- Remove duplicate tool names while preserving their first occurrence.
- Do not use `allowed-tools` as the skill's purpose, as a substitute for its body, or as the sole reason to select it.

#### Recommended body

The body may use any readable Markdown structure, but it must answer five questions:

1. What behavior does the skill teach?
2. When should it be used?
3. When should it not be used?
4. What procedure or criteria must be followed?
5. How is completion recognized?

Recommended template:

```markdown
# Human-Readable Skill Name

## Purpose

State the single behavioral concern taught by the skill.

## Use this skill when

- Describe applicable situations in behavioral terms.

## Do not use this skill when

- Describe exclusions and nearby cases handled elsewhere.

## Procedure

1. Give an operational procedure or explicit decision criteria.
2. Explain how tools support the procedure without replacing reasoning.
3. Define how observations should change the next action.

## Completion

State the observable or semantic result that indicates the procedure is complete.
```

#### Body-writing rules

- Write instructions for executing an objective, not for reproducing one benchmark prompt.
- Use domain terminology that helps body-aware retrieval distinguish the skill from nearby alternatives.
- Include important exclusions so the selector can reject the skill when it is merely related by vocabulary.
- State evidence, ordering, comparison, or validation criteria explicitly when they matter.
- Refer to tools by their canonical names only when the procedure actually depends on them.
- Allow multiple calls to the same tool when the objective requires an iterative trajectory.
- Do not turn each tool call into a separate objective.
- Do not assume this skill is the only member of the bundle.
- Do not contradict likely companion skills. When two procedures genuinely conflict, separate their applicable conditions clearly.
- Do not embed secrets, environment-specific credentials, test answers, literal benchmark requests, or unstable repository-specific identifiers.
- Do not duplicate generic runtime instructions that apply to every skill unless this is intentionally an always-available core skill.

### 6. Register the files in `manifest.json`

Use this manifest shape:

```json
{
  "name": "bundle-name",
  "description": "Bundle description.",
  "tools": [
    {
      "path": "tools/tool-name.json",
      "alwaysAvailable": false
    }
  ],
  "skills": [
    {
      "path": "skills/skill-name/SKILL.md",
      "alwaysAvailable": false
    }
  ]
}
```

Manifest rules:

1. Keep `name`, `description`, `tools`, and `skills` present.
2. Use paths relative to the bundle root.
3. Register each shipped tool and skill exactly once.
4. Do not register missing files.
5. Preserve a deterministic order, preferably canonical-name order, unless the repository defines another convention.
6. Set a tool's `alwaysAvailable` to `true` only when it belongs to the bundle's base tool set and should remain visible even with an empty skill bundle.
7. Set a skill's `alwaysAvailable` to `true` only for a small, universal behavior that should guide every objective. Most domain and procedure skills must be `false`.
8. A non-base tool intended for a procedure must be named in the corresponding skill's `allowed-tools`.
9. Do not make every skill or every specialized tool always available merely to avoid routing decisions.

### 7. Write with the appropriate workspace operation

- Use `write` to create a new file or intentionally replace an entire file.
- Use `edit` for focused changes to an existing file. Each `oldText` must identify one unique, non-overlapping region.
- Use `grep` after editing to confirm that names and references changed everywhere required and nowhere unrelated.
- Use `terminal` for validation commands, not as an undocumented substitute for `write` or `edit`.
- Preserve unrelated content and formatting.

### 8. Validate the bundle

Run repository-provided validators when available. Otherwise perform equivalent checks with the available runtime.

The bundle is invalid if any of these checks fail:

- every JSON file parses;
- every `SKILL.md` frontmatter block parses as YAML;
- every manifest path exists;
- skill names are unique;
- tool names are unique;
- every skill has a non-empty `description` and non-empty Markdown body;
- every tool exposes `name`, `inputSchema`, and `outputSchema`;
- every `allowed-tools` entry resolves exactly to a registered tool;
- duplicated manifest entries or duplicated `allowed-tools` entries are removed;
- no tool descriptor silently contains a cognitive workflow;
- no skill is merely a renamed tool description;
- no skill combines an entire end-to-end agent into one non-composable body;
- `alwaysAvailable` is used intentionally rather than by default;
- no unrelated file was modified.

Do not claim validation succeeded unless the checks were actually executed.

### 9. Review retrieval and composition quality

Before completion, evaluate each new skill as a catalog item:

- **Distinctness:** Does its body add behavior not already covered by another skill?
- **Applicability:** Can a router infer when to use and reject it from the description and body?
- **Compatibility:** Can it compose with neighboring skills without contradictory instructions?
- **Necessity:** Does it teach more than the model already knows from generic tool schemas?
- **Scope:** Is it reusable without becoming a full workflow?
- **Tool linkage:** Are declared tools operationally relevant rather than included for keyword matching?
- **Completion:** Does the body define a recognizable result?

Revise or omit skills that are redundant, conflicting, outside the intended objective, too broad, or behaviorally empty.

## Completion

Capability authoring is complete when:

- the required skill and tool files exist in the expected paths;
- the distinction between behavioral instruction and executable operation is explicit;
- tool schemas expose stable, observable contracts;
- every skill is coherent, reusable, composable, and concise enough to load in full;
- `allowed-tools` references resolve exactly;
- `manifest.json` registers all files with intentional `alwaysAvailable` values;
- validation has passed or every unresolved validation failure has been reported precisely.
