---
name: generate-project-index
description: Regenerate or update the Spectacular repository index skill. Use when the user asks to create, refresh, update, rebuild, regenerate, or verify the project index, repository map, file tree, or file descriptions under `.agents/skills/project-index`.
---

# Generate Project Index

## Overview

Use this skill to regenerate `.agents/skills/project-index/SKILL.md` from the current repository. The output should stay concise enough to be loaded as startup context while still describing every source-controlled and unignored project file.

## Workflow

1. Inspect the current repo state with `git status --short`.
2. List files with `git ls-files --cached --others --exclude-standard`; use `rg --files` only if available and configured with equivalent ignores.
3. Read high-signal files before writing descriptions: `README.md`, root manifests (`Cargo.toml`, `package.json`, `nx.json`), project manifests, crate entry points, and existing skill frontmatter.
4. Exclude generated/dependency/local-only directories from the index: `.git/`, `.nx/cache/`, `.nx/workspace-data/`, `dist/`, `tmp/`, `out-tsc/`, `coverage/`, `node_modules/`, and ignored `.journal/`.
5. Update `.agents/skills/project-index/SKILL.md` with valid skill frontmatter, a repository summary, an ASCII tree, one short description per indexed file, and brief working notes.
6. Preserve unrelated user changes. Do not revert dirty worktree files.
7. Run `python C:\Users\jvito\.codex\skills\.system\skill-creator\scripts\quick_validate.py .agents/skills/project-index` when available, then check `git status --short`.

## Reusable Prompt

Use this prompt when asking an agent to regenerate the index:

```text
Regenerate the Spectacular project index skill at `.agents/skills/project-index/SKILL.md`.

Requirements:
- Keep the file as a valid Codex skill with YAML frontmatter containing only `name` and `description`.
- Use `name: project-index`.
- Write a trigger-focused description that says the skill is the Spectacular repository map and should be used for repo context, layout, files, planning, coding, reviewing, or debugging.
- Include `Last updated: YYYY-MM-DD` using the current date.
- Include a short repository summary.
- Include an ASCII tree of all source-controlled and unignored project files.
- Add a small, accurate description beside each file or meaningful directory.
- Omit generated/dependency/local-only paths such as `.git/`, `.nx/cache/`, `.nx/workspace-data/`, `dist/`, `tmp/`, `out-tsc/`, `coverage/`, `node_modules/`, and ignored `.journal/`.
- Mention omitted paths explicitly near the top.
- Include short working notes for agents, including preferred Nx commands, Cargo workspace awareness, real vs fake agent integration, and when to read `.journal`.
- Do not invent files. Use `git ls-files --cached --others --exclude-standard` as the source of truth unless a better repo-native file listing is available.
- Read high-signal files before writing descriptions: `README.md`, `Cargo.toml`, `package.json`, `nx.json`, project manifests, crate entry points, and existing skill frontmatter.
- Preserve unrelated worktree changes.
- Validate with the skill validator if available and report any validation issues.
```

## Output Shape

Use this section order for `.agents/skills/project-index/SKILL.md`:

1. YAML frontmatter.
2. `# Project Index`.
3. `Last updated: ...`.
4. One-paragraph purpose.
5. Omitted paths list.
6. `## Repository Summary`.
7. `## Tree` with a fenced `text` block.
8. `## Working Notes For Agents`.
