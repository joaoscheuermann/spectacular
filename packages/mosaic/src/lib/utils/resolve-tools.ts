import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import { unique } from './unique.js';

export const resolveTools = (
  required: readonly Tool[],
  skills: readonly Skill[],
  menu: readonly Tool[],
): Tool[] => {
  const allowed = new Set(skills.flatMap((skill) => skill.allowedTools));
  return unique([...required, ...menu.filter(({ name }) => allowed.has(name))]);
};
