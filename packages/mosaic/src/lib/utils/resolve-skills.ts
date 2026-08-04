import type { Skill } from 'bundle';

import { unique } from './unique.js';

export const resolveSkills = (
  required: readonly Skill[],
  selected: readonly string[],
  menu: readonly Skill[],
): Skill[] =>
  unique([
    ...required,
    ...selected.flatMap(
      (name) => menu.find((skill) => skill.name === name) ?? [],
    ),
  ]);
