import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import type { MosaicOptions } from './types/mosaic-options.js';

/** Validates Mosaic's catalog and routing invariants before a workflow starts. */
export const validateOptions = (options: MosaicOptions): void => {
  const { models, routing, execution, revision, skills, tools } = options;

  requireModel('default', models.default);
  requireModel('reranker', models.reranker);
  requireInteger('routing.maxCandidates', routing.maxCandidates, 1);
  requireInteger('routing.maxSkills', routing.maxSkills, 0);
  requireInteger('execution.maxTurns', execution?.maxTurns, 1);
  requireInteger('revision.max', revision?.max, 0);

  if (routing.maxSkills > routing.maxCandidates) {
    throw new TypeError(
      'Mosaic routing.maxSkills must not exceed routing.maxCandidates.',
    );
  }

  requireRetriever('skills.retriever', skills.retriever);
  requireRetriever('tools.retriever', tools.retriever);
  requireUnique('skills.menu', skills.menu);
  requireUnique('skills.required', skills.required);
  requireUnique('tools.menu', tools.menu);
  requireUnique('tools.required', tools.required);

  validateCatalogs(options);
};

const validateCatalogs = ({ skills, tools }: MosaicOptions): void => {
  const skillNames = new Set(skills.menu.map(({ name }) => name));
  const toolNames = new Set(tools.menu.map(({ name }) => name));
  const baseToolNames = new Set(tools.required.map(({ name }) => name));

  requireSubset('skills.required', skills.required, skillNames);
  requireSubset('tools.required', tools.required, toolNames);
  validateAllowedTools(skills.menu, toolNames);
  validateRequiredSkillTools(skills.required, baseToolNames);
};

const validateAllowedTools = (
  skills: readonly Skill[],
  toolNames: ReadonlySet<string>,
): void => {
  for (const skill of skills) {
    for (const tool of skill.allowedTools) {
      if (!toolNames.has(tool)) {
        throw new TypeError(
          `Mosaic skill ${skill.name} references unavailable tool ${tool}.`,
        );
      }
    }
  }
};

const validateRequiredSkillTools = (
  skills: readonly Skill[],
  baseToolNames: ReadonlySet<string>,
): void => {
  for (const skill of skills) {
    for (const tool of skill.allowedTools) {
      if (!baseToolNames.has(tool)) {
        throw new TypeError(
          `Mosaic required skill ${skill.name} references non-base tool ${tool}.`,
        );
      }
    }
  }
};

const requireModel = (name: string, value: string): void => {
  if (value.trim().length > 0) return;
  throw new TypeError(`Mosaic models.${name} must be a non-empty string.`);
};

const requireInteger = (
  name: string,
  value: number | undefined,
  minimum: number,
): void => {
  if (value !== undefined && Number.isSafeInteger(value) && value >= minimum)
    return;
  throw new TypeError(`Mosaic ${name} must be an integer >= ${minimum}.`);
};

const requireRetriever = (name: string, value: object): void => {
  if (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { search?: unknown }).search === 'function'
  ) {
    return;
  }
  throw new TypeError(`Mosaic ${name} must provide search.`);
};

const requireUnique = (
  name: string,
  values: readonly (Skill | Tool)[],
): void => {
  const seen = new Set<string>();

  for (const value of values) {
    if (!seen.has(value.name)) {
      seen.add(value.name);
      continue;
    }

    throw new TypeError(`Mosaic ${name} contains duplicate ${value.name}.`);
  }
};

const requireSubset = (
  name: string,
  values: readonly (Skill | Tool)[],
  menu: ReadonlySet<string>,
): void => {
  const missing = values.find(({ name: value }) => !menu.has(value));
  if (missing === undefined) return;
  throw new TypeError(`Mosaic ${name} contains unavailable ${missing.name}.`);
};
