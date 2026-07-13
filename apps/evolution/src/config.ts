import { readFile } from 'node:fs/promises';

import {
  evolutionConfigSchema,
  type EvolutionConfig,
  type ModelRef,
} from './schema.js';

const unique = (values: readonly string[], label: string): void => {
  if (new Set(values).size !== values.length) {
    throw new Error(`${label} must be unique.`);
  }
};

export const parseConfig = (value: unknown): EvolutionConfig => {
  const config = evolutionConfigSchema.parse(value);
  unique(
    config.providers.map(({ id }) => id),
    'Provider ids',
  );
  unique(
    config.models.map(({ id }) => id),
    'Model ids',
  );
  unique(
    config.evals.map(({ id }) => id),
    'Global eval ids',
  );

  const providers = new Set(config.providers.map(({ id }) => id));
  const refs: readonly ModelRef[] = [
    ...config.models,
    config.optimizer,
    config.judge,
  ];
  const missing = refs.find(({ provider }) => !providers.has(provider));
  if (missing !== undefined) {
    throw new Error(`Unknown provider reference: ${missing.provider}`);
  }
  return config;
};

export const loadConfig = async (path: string): Promise<EvolutionConfig> =>
  parseConfig(JSON.parse(await readFile(path, 'utf8')));
