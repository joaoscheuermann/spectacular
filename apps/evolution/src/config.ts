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

const modelKey = ({ provider, model }: ModelRef): string =>
  `${provider}:${model}`;

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
  unique(config.judges.map(modelKey), 'Judge models');

  const providers = new Set(config.providers.map(({ id }) => id));
  const refs = [...config.models, config.optimizer, ...config.judges];
  const missing = refs.find(({ provider }) => !providers.has(provider));
  if (missing !== undefined) {
    throw new Error(`Unknown provider reference: ${missing.provider}`);
  }

  return config;
};

export const loadConfig = async (path: string): Promise<EvolutionConfig> =>
  parseConfig(JSON.parse(await readFile(path, 'utf8')));
