import type { Skill } from 'bundle';
import type { Tool, ToolMetadata } from 'tool';

/** Resolves ordered node references against the canonical skill catalog. */
export const resolveSkills = (
  selected: readonly string[],
  menu: readonly Skill[],
): Skill[] => {
  // Node snapshots keep compact names; execution bodies remain catalog-owned.
  const catalog = new Map(menu.map((skill) => [skill.name, skill]));

  return selected.map((name) => {
    // Missing definitions invalidate the bundle instead of silently dropping it.
    const skill = catalog.get(name);
    if (skill !== undefined) return skill;
    throw new Error(`Selected skill is unavailable: ${name}`);
  });
};

/** Composes the exact base-plus-selected tool menu in stable first-use order. */
export const composeTools = (
  skills: readonly Skill[],
  required: readonly Tool[],
  menu: readonly Tool[],
): Tool[] => {
  // T_registry resolves both base tools and skill-declared menu entries.
  const catalog = new Map(menu.map((tool) => [tool.name, tool]));

  // Section 4.6 defines T(g) as base tools followed by selected-skill tools.
  const names = unique([
    ...required.map(({ name }) => name),
    ...skills.flatMap(({ allowedTools }) => allowedTools),
  ]);

  return names.map((name) => {
    // allowed-tools is declarative here, but every declaration must still resolve.
    const tool = catalog.get(name);
    if (tool !== undefined) return tool;
    throw new Error(`Selected tool is unavailable: ${name}`);
  });
};

/** Projects executable tools into the model-generated graph's inspectable state. */
export const metadata = (tools: readonly Tool[]): ToolMetadata[] =>
  tools.map(({ name, description }) => ({
    name,
    description: description ?? '',
  }));

/** Preserves first-use order while enforcing one visible entry per tool name. */
const unique = (values: readonly string[]): string[] => {
  const seen = new Set<string>();
  return values.filter((value) => !seen.has(value) && Boolean(seen.add(value)));
};
