import type { Skill } from 'bundle';
import type { Tool, ToolMetadata } from 'tool';

import type { NodeSkillSelection } from '../../types/node-skill-selection.js';

/** Resolves ordered node references against the canonical skill catalog. */
export const resolveSkills = (
  selected: readonly NodeSkillSelection[],
  menu: readonly Skill[],
): Skill[] => {
  const catalog = new Map(menu.map((skill) => [skill.name, skill]));

  return selected.map(({ skill: name }) => {
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
  const catalog = new Map(menu.map((tool) => [tool.name, tool]));
  const names = unique([
    ...required.map(({ name }) => name),
    ...skills.flatMap(({ allowedTools }) => allowedTools),
  ]);

  return names.map((name) => {
    const tool = catalog.get(name);
    if (tool !== undefined) return tool;
    throw new Error(`Selected tool is unavailable: ${name}`);
  });
};

export const metadata = (tools: readonly Tool[]): ToolMetadata[] =>
  tools.map(({ name, description }) => ({
    name,
    description: description ?? '',
  }));

const unique = (values: readonly string[]): string[] => {
  const seen = new Set<string>();
  return values.filter((value) => !seen.has(value) && Boolean(seen.add(value)));
};
