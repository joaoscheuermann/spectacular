import type { Skill } from 'bundle';
import type { Tool } from 'tool';

export type Menus = {
  readonly skills: readonly Skill[];
  readonly tools: readonly Tool[];
};

type MenuOptions = {
  readonly requiredSkills: readonly Skill[];
  readonly selected: readonly string[];
  readonly skillMenu: readonly Skill[];
  readonly requiredTools: readonly Tool[];
  readonly toolMenu: readonly Tool[];
};

export const candidateSkills = (
  required: readonly Skill[],
  reranked: readonly Skill[],
): Skill[] => unique([...required, ...reranked]);

/** Resolves selected skills and their allowed tools in stable catalog order. */
export const composeMenus = ({
  requiredSkills,
  selected,
  skillMenu,
  requiredTools,
  toolMenu,
}: MenuOptions): Menus => {
  const skills = unique([
    ...requiredSkills,
    ...selected.flatMap(
      (name) => skillMenu.find((skill) => skill.name === name) ?? [],
    ),
  ]);
  const allowed = new Set(skills.flatMap((skill) => skill.allowedTools));
  const tools = unique([
    ...requiredTools,
    ...toolMenu.filter(({ name }) => allowed.has(name)),
  ]);

  return { skills, tools };
};

const unique = <Value extends { readonly name: string }>(
  values: readonly Value[],
): Value[] => {
  const names = new Set<string>();

  return values.filter(
    ({ name }) => !names.has(name) && Boolean(names.add(name)),
  );
};
