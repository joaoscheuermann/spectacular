import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import {
  OrderedBundleSchema,
  SkillCandidateSchema,
} from '../../schemas/routing.js';
import type { SkillMatch } from '../../types/evaluation.js';
import type { Node } from '../../types/graph.js';
import type { RoutingTrace } from '../../types/routing.js';

/** Validates an intercepted route before it can update graph state. */
export const validateTrace = (
  trace: RoutingTrace,
  node: Node,
  catalog: readonly Skill[],
  candidateLimit: number,
  selectionLimit: number,
): RoutingTrace => {
  const candidates = SkillCandidateSchema.array().parse(trace.candidates);
  const bundle = OrderedBundleSchema.parse(trace.bundle);
  const known = new Set(catalog.map(({ name }) => name));
  const names = candidates.map(({ skillName }) => skillName);
  const selectedInOrder = names.filter((name) => bundle.skills.includes(name));

  if (
    candidates.length > candidateLimit ||
    candidates.some(
      ({ skillName, rank }, index) =>
        !known.has(skillName) || rank !== index + 1,
    ) ||
    new Set(names).size !== names.length ||
    bundle.goalId !== node.id ||
    bundle.skills.length > selectionLimit ||
    bundle.skills.some((name) => !names.includes(name)) ||
    !sameItems(bundle.skills, selectedInOrder)
  ) {
    throw new Error('Evaluation routing returned an invalid trace.');
  }

  return { candidates, bundle };
};

/** Resolves intercepted matches back to canonical catalog identities. */
export const validateMatches = (
  matches: readonly SkillMatch[],
  catalog: ReadonlyMap<string, Skill>,
  limit: number,
): readonly SkillMatch[] => {
  const seen = new Set<string>();

  return matches
    .flatMap(({ skill, score }) => {
      if (!Number.isFinite(score)) {
        throw new Error(
          'Evaluation retrieval returned an invalid skill score.',
        );
      }

      const current = catalog.get(skill.name);

      if (current === undefined || seen.has(current.name)) {return [];}

      seen.add(current.name);

      return [{ skill: current, score }];
    })
    .slice(0, limit);
};

/** Resolves intercepted tools to known executable catalog entries. */
export const validateTools = (
  tools: readonly Tool[],
  required: readonly Tool[],
  menu: readonly Tool[],
): Tool[] => {
  const catalog = new Map(
    [...required, ...menu].map((tool) => [tool.name, tool]),
  );
  const seen = new Set<string>();

  return tools.map(({ name }) => {
    const current = catalog.get(name);

    if (current === undefined || seen.has(name)) {
      throw new Error('Evaluation menu returned an invalid tool.');
    }

    seen.add(name);

    return current;
  });
};

const sameItems = (
  left: readonly string[],
  right: readonly string[],
): boolean =>
  left.length === right.length &&
  left.every((item, index) => item === right[index]);
