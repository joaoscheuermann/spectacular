import type { Stats } from 'node:fs';
import { lstat, mkdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

import type { EvolutionConfig } from './schema.js';

const configName = 'evolution.config.json';
const scenariosName = 'scenarios';

const defaultConfig = {
  providers: [{ id: 'openai', type: 'openai', tokenEnv: 'OPENAI_API_KEY' }],
  models: [{ id: 'target-model', provider: 'openai', model: 'target-model' }],
  optimizer: { provider: 'openai', model: 'optimizer-model' },
  judge: { provider: 'openai', model: 'judge-model' },
  evals: [
    {
      id: 'correct-output',
      assertion: 'The output correctly fulfills the requested behavior.',
    },
  ],
  evolution: {
    accuracy: 0.9,
    patience: { epochs: 3 },
    epochs: 20,
    history: { limit: 30 },
  },
} satisfies EvolutionConfig;

const configText = JSON.stringify(defaultConfig, null, 2) + '\n';

export type InitSummary = {
  readonly root: string;
  readonly configCreated: boolean;
  readonly scenariosCreated: boolean;
};

const inspect = async (path: string): Promise<Stats | undefined> => {
  try {
    return await lstat(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return undefined;
    }
    throw error;
  }
};

const assertConfig = (path: string, stats: Stats | undefined): void => {
  if (stats !== undefined && !stats.isFile()) {
    throw new Error(`${path} must be a regular file.`);
  }
};

const assertScenarios = (path: string, stats: Stats | undefined): void => {
  if (stats !== undefined && (!stats.isDirectory() || stats.isSymbolicLink())) {
    throw new Error(`${path} must be a real directory.`);
  }
};

const createConfig = async (path: string): Promise<boolean> => {
  try {
    await writeFile(path, configText, { encoding: 'utf8', flag: 'wx' });
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') {
      throw error;
    }
    assertConfig(path, await inspect(path));
    return false;
  }
};

const createScenarios = async (path: string): Promise<boolean> => {
  try {
    await mkdir(path);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') {
      throw error;
    }
    assertScenarios(path, await inspect(path));
    return false;
  }
};

/** Ensures a workspace has a default config and real scenarios directory without overwriting either. */
export const initializeWorkspace = async (
  directory = '.',
): Promise<InitSummary> => {
  const root = resolve(directory);
  const configPath = join(root, configName);
  const scenariosPath = join(root, scenariosName);
  const [config, scenarios] = await Promise.all([
    inspect(configPath),
    inspect(scenariosPath),
  ]);
  assertConfig(configPath, config);
  assertScenarios(scenariosPath, scenarios);

  if (config === undefined || scenarios === undefined) {
    await mkdir(root, { recursive: true });
  }

  const configCreated =
    config === undefined ? await createConfig(configPath) : false;
  const scenariosCreated =
    scenarios === undefined ? await createScenarios(scenariosPath) : false;

  return { root, configCreated, scenariosCreated };
};
