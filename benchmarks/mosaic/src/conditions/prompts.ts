import { readFile } from 'node:fs/promises';

import { artifactHash } from '../core/hash.js';

export interface ConditionPrompts {
  readonly baseline: string;
  readonly planner: string;
  readonly executor: string;
  readonly revision: string;
  readonly hash: string;
}

const readPrompt = async (relative: string): Promise<string> => {
  const candidates = [
    new URL(`../../prompts/${relative}`, import.meta.url),
    new URL(`../../../prompts/${relative}`, import.meta.url),
  ];
  for (const candidate of candidates) {
    try {
      return await readFile(candidate, 'utf8');
    } catch (error) {
      if (
        typeof error !== 'object' ||
        error === null ||
        !('code' in error) ||
        error.code !== 'ENOENT'
      )
        throw error;
    }
  }
  throw new Error('frozen condition prompt is unavailable');
};

/** Loads the exact human-readable prompt set and computes its freeze hash. */
export const loadConditionPrompts = async (): Promise<ConditionPrompts> => {
  const [baseline, planner, executor, revision] = await Promise.all([
    readPrompt('baseline/SYSTEM_PROMPT.md'),
    readPrompt('planner/SYSTEM_PROMPT.md'),
    readPrompt('executor/SYSTEM_PROMPT.md'),
    readPrompt('revision/SYSTEM_PROMPT.md'),
  ]);
  const values = { baseline, planner, executor, revision };
  return { ...values, hash: artifactHash(values) };
};
