import { TOOL_NAMES } from '../config/index.js';
import type { CatalogDomain, MicroSkill } from './types.js';

export interface CatalogIssue {
  readonly code: string;
  readonly path: string;
  readonly message: string;
}

const DOMAINS: readonly CatalogDomain[] = [
  'documents-finance',
  'software',
  'artifacts',
  'communication',
];

const countsByDomain = (
  skills: readonly MicroSkill[],
): Readonly<Record<CatalogDomain, number>> =>
  Object.fromEntries(
    DOMAINS.map((domain) => [
      domain,
      skills.filter((skill) => skill.domain === domain).length,
    ]),
  ) as Readonly<Record<CatalogDomain, number>>;

const duplicateIssues = (
  skills: readonly MicroSkill[],
): readonly CatalogIssue[] => {
  const counts = new Map<string, number>();
  skills.forEach((skill) =>
    counts.set(skill.id, (counts.get(skill.id) ?? 0) + 1),
  );
  return [...counts.entries()]
    .filter(([, count]) => count > 1)
    .map(([id]) => ({
      code: 'duplicate_skill',
      path: id,
      message: 'skill id is not unique',
    }));
};

/** Validates frozen catalog counts, references, annotations, and tool names. */
export const validateCatalog = (
  skills: readonly MicroSkill[],
): readonly CatalogIssue[] => {
  const ids = new Set(skills.map((skill) => skill.id));
  const domainCounts = countsByDomain(skills);
  const issues: CatalogIssue[] = [...duplicateIssues(skills)];

  if (skills.length !== 60) {
    issues.push({
      code: 'skill_count',
      path: '$',
      message: `expected 60 skills, received ${skills.length}`,
    });
  }
  DOMAINS.forEach((domain) => {
    if (domainCounts[domain] !== 15) {
      issues.push({
        code: 'domain_count',
        path: domain,
        message: `expected 15 skills, received ${domainCounts[domain]}`,
      });
    }
  });

  skills.forEach((skill) => {
    if (skill.relations.length === 0) {
      issues.push({
        code: 'missing_relation',
        path: skill.id,
        message: 'skill has no annotated relation',
      });
    }
    skill.allowedTools.forEach((tool) => {
      if (!(TOOL_NAMES as readonly string[]).includes(tool)) {
        issues.push({
          code: 'unknown_tool',
          path: `${skill.id}.allowedTools`,
          message: tool,
        });
      }
    });
    skill.relations.forEach((relation, index) => {
      if (!ids.has(relation.skillId)) {
        issues.push({
          code: 'unknown_skill',
          path: `${skill.id}.relations[${index}]`,
          message: relation.skillId,
        });
      }
      if (relation.skillId === skill.id) {
        issues.push({
          code: 'self_relation',
          path: `${skill.id}.relations[${index}]`,
          message: 'relation must reference another skill',
        });
      }
    });
  });

  return issues;
};
