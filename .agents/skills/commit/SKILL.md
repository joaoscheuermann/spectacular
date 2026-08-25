---
name: commit
description: Create a Git commit using a Conventional Commit message for the changes from the current conversation, or all worktree changes when no conversation-specific history is available, then push the resulting commit to origin. Use when the user asks to commit, conventional commit, or commit and push repository changes.
---

# Commit

## Overview

Use this skill to turn the current repository changes into one Conventional Commit and push it to `origin`. The commit scope is the current conversation's changes when they can be identified from chat history and local diffs; if no usable conversation history is available, commit all current worktree changes after review.

## Workflow

1. Inspect the repository state before staging anything:
   - Run `git status --short`.
   - Review `git diff --stat`, `git diff`, and any staged diff with `git diff --cached`.
   - Check the current branch with `git branch --show-current`.

2. Decide the commit scope:
   - If the current conversation clearly identifies files or edits made for this request, include only those changes.
   - If there is no accessible or reliable conversation-specific history, include all non-ignored worktree changes.
   - If unrelated changes are visible while conversation history is available, do not stage them.
   - If secrets, credentials, generated artifacts without provenance, or destructive changes appear in scope, stop and ask the user before committing.

3. Validate before committing when practical:
   - Prefer checks already run during the conversation.
   - For docs-only or skill-only changes, run `git diff --check`.
   - For code changes, run the narrowest relevant configured test, typecheck, build, or formatter check.
   - If validation is blocked or would be disproportionate, state that in the final response.

4. Stage the selected changes:
   - Use explicit paths or patch staging when committing conversation-specific changes.
   - Use `git add -A` only when committing all changes is correct under step 2.
   - Re-run `git status --short` and `git diff --cached --stat`.

5. Write a Conventional Commit message:
   - Format: `<type>(<scope>): <description>`.
   - Use a body when the change needs context, risk notes, or validation details.
   - Common types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
   - Use `!` before `:` and a `BREAKING CHANGE:` footer only for intentional breaking changes.
   - Keep the subject imperative, lower-case after the type, and under 72 characters when reasonable.

6. Commit and push:
   - Run `git commit -m "<type>(<scope>): <description>"` or use multiple `-m` arguments for a body.
   - Push with `git push origin HEAD` unless the user explicitly requested a different remote or branch.
   - If push is rejected because the remote moved, stop and report the rejection instead of rebasing, merging, or force-pushing without user approval.

## Message Selection

Choose the type and scope from the actual diff, not from the user's wording alone.

- Use `feat` for new user-visible behavior or capability.
- Use `fix` for bug fixes or behavior corrections.
- Use `docs` for documentation-only changes, including skill instructions.
- Use `test` for test-only changes.
- Use `refactor` for behavior-preserving code restructuring.
- Use `chore` for repository maintenance that does not fit a narrower type.

Prefer a scope that names the package, app, workflow, or artifact changed, such as `agent`, `sandbox`, `docker`, `skills`, or `commit`.

## Final Response

After pushing, report:

- The commit hash and exact subject.
- The remote push target.
- The validation command and result, or the reason validation was not run.
