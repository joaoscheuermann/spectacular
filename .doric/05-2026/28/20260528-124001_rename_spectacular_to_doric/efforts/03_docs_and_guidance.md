# Effort: Docs and guidance

Status: done

## Requirement links

PRD-FR1, PRD-FR5, PRD-AC5, TDD-08-README, TDD-08-ARCH-DOC, TDD-08-SKILLS

## Goal

Update README and active repository guidance to Doric naming, and create the missing architecture document that reflects the renamed package map.

## Target files

- `README.md`
- `docs/architecture-and-packages.md`
- `.agents/skills/current-architecture/**`
- `.agents/skills/iocraft/**`

## Coupled files

- `Cargo.toml`
- `packages/*/Cargo.toml`
- `packages/*/project.json`

## Tests to add or update

- No new executable tests required unless docs are consumed by validation scripts.

## Regression suites

- `rg -n "Spectacular|spectacular|SPECTACULAR|spcetacular" --glob "!target/**" --glob "!dist/**" --glob "!node_modules/**" --glob "!.doric/**"`

## Acceptance criteria

- README uses Doric product naming, `doric` command examples, and new package names.
- `docs/architecture-and-packages.md` exists and describes current renamed architecture.
- Active `.agents` skill docs no longer point to old package paths or product names except explicit historical notes.

## Notes

The architecture document describes observed current architecture after the
hard rename. README command examples use the `cli` Nx project and `doric`
binary.
