# bundle

`bundle` discovers and validates executable Doric bundles. It turns strict
manifests, compiled tool factories, and Markdown skills into the canonical
catalog consumed by the Direct host.

## Load bundles

```ts
import { loadBundles } from 'bundle';

const bundles = await loadBundles('agents/doric/dist/bundles');

for (const bundle of bundles) {
  console.log(bundle.name, bundle.tools.length, bundle.skills.length);
}
```

Only immediate child directories are loaded, in lexical order. Bundle, tool,
and skill names must be globally unique.

## Bundle layout

Each child directory needs a `manifest.json`:

```json
{
  "name": "example",
  "description": "Example capabilities.",
  "tools": [{ "path": "tools/read.js", "alwaysAvailable": true }],
  "skills": [
    {
      "path": "skills/inspect/SKILL.md",
      "alwaysAvailable": false
    }
  ]
}
```

Tool paths must match `tools/<name>.js`. Each module must default-export a
compatible `ToolFactory`. Runtime TypeScript is rejected.

Skill paths must match `skills/<name>/SKILL.md`, with YAML frontmatter followed
by a non-empty Markdown body:

```markdown
---
name: inspect
description: Inspect the workspace before making a change.
allowed-tools:
  - read
---

# Inspect

Use the read tool to gather the smallest necessary context.
```

An `allowed-tools` entry must resolve to a tool in the same bundle.
`SkillSchema` is also exported for consumers that need the canonical,
JSON-Schema-compatible `SkillRecord` representation.

Invalid manifests, paths, modules, frontmatter, duplicate names, and missing
local tool references fail loading with contextual errors.

## Development

```console
npx nx build bundle
npx nx test bundle
```
