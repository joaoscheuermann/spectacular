import { readFile, readdir } from 'node:fs/promises';
import { join } from 'node:path';

import { scenarioSchema, type EvalAssertion, type Scenario } from './schema.js';

export const normalizeInput = (value: string): string =>
  value.trim().replace(/\s+/gu, ' ').toLocaleLowerCase();

const unique = (values: readonly string[], message: string): void => {
  if (new Set(values).size !== values.length) throw new Error(message);
};

/** Validates suite-level uniqueness, split coverage, and effective assertions. */
export const prepareScenarios = (
  scenarios: readonly Scenario[],
  globals: readonly EvalAssertion[],
): readonly Scenario[] => {
  unique(
    scenarios.map(({ id }) => id),
    'Scenario ids must be unique.',
  );
  unique(
    scenarios.map(({ input }) => normalizeInput(input)),
    'Scenario inputs must be unique.',
  );
  for (const split of ['train', 'validation'] as const) {
    if (!scenarios.some((scenario) => scenario.split === split)) {
      throw new Error(
        `Scenario suite requires at least one ${split} scenario.`,
      );
    }
  }

  return scenarios.map((scenario) => {
    const evals = [...globals, ...scenario.evals];
    if (evals.length === 0) {
      throw new Error(`Scenario "${scenario.id}" has no effective evals.`);
    }
    unique(
      evals.map(({ id }) => id),
      `Scenario "${scenario.id}" has duplicate effective eval ids.`,
    );
    return { ...scenario, evals };
  });
};

/** Loads a complete, manually-authored scenario suite in stable lexical order. */
export const loadScenarios = async (
  directory: string,
  globals: readonly EvalAssertion[] = [],
): Promise<readonly Scenario[]> => {
  let names: readonly string[];
  try {
    names = (await readdir(directory))
      .filter((name) => name.endsWith('.json'))
      .sort();
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      throw new Error('Scenario suite directory is missing.');
    }
    throw error;
  }
  const scenarios = await Promise.all(
    names.map(async (name) =>
      scenarioSchema.parse(
        JSON.parse(await readFile(join(directory, name), 'utf8')),
      ),
    ),
  );
  return prepareScenarios(scenarios, globals);
};
