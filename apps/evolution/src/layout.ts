import { randomUUID } from 'node:crypto';
import { lstat, mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative } from 'node:path';

import type { TargetResult } from './evolve.js';
import { scenarioSchema, type Scenario } from './schema.js';

export type Layout = {
  readonly root: string;
  readonly initialScenarios: readonly Scenario[];
  readonly scenarios: readonly Scenario[];
  readonly targets: readonly TargetResult[];
  readonly dryRun: boolean;
};

const current = async (path: string): Promise<string | undefined> => {
  try {
    return await readFile(path, 'utf8');
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      return undefined;
    }
    throw error;
  }
};

const atomicWrite = async (path: string, body: string): Promise<void> => {
  await mkdir(dirname(path), { recursive: true });
  const temporary = path + '.' + randomUUID() + '.tmp';
  await writeFile(temporary, body, 'utf8');
  await rename(temporary, path);
};

const serializedScenario = (scenario: Scenario): string =>
  JSON.stringify(scenarioSchema.parse(scenario), null, 2) + '\n';

const assertSafeWritePath = async (
  root: string,
  path: string,
): Promise<void> => {
  const child = relative(root, path);
  if (child.startsWith('..') || isAbsolute(child)) {
    throw new Error('Write path escapes the evolution root: ' + path);
  }
  const rootStats = await lstat(root);
  if (rootStats.isSymbolicLink()) {
    throw new Error('Write path contains a symbolic link or junction: ' + root);
  }
  const components = [root, ...child.split(/[\\/]/u).filter(Boolean)];
  let currentPath = components[0] ?? root;
  for (const component of components.slice(1)) {
    currentPath = join(currentPath, component);
    try {
      const stats = await lstat(currentPath);
      if (stats.isSymbolicLink()) {
        throw new Error(
          'Write path contains a symbolic link or junction: ' + currentPath,
        );
      }
    } catch (error) {
      if (
        error instanceof Error &&
        'code' in error &&
        error.code === 'ENOENT'
      ) {
        return;
      }
      throw error;
    }
  }
};

const assertScenarioCompatible = async (
  path: string,
  scenario: Scenario,
): Promise<void> => {
  const existing = await current(path);
  if (existing === undefined) return;
  const parsed = scenarioSchema.parse(JSON.parse(existing));
  if (serializedScenario(parsed) !== serializedScenario(scenario)) {
    throw new Error('Scenario id collision: ' + scenario.id);
  }
};

const preflight = async (layout: Layout): Promise<void> => {
  const defaultPaths = layout.initialScenarios.map((scenario) =>
    join(layout.root, 'default', 'scenarios', scenario.id + '.json'),
  );
  const scenarioPaths = layout.scenarios.map((scenario) =>
    join(layout.root, 'scenarios', scenario.id + '.json'),
  );
  const modelPaths = layout.targets.map(({ target }) =>
    join(layout.root, target.id, 'SYSTEM_PROMPT.md'),
  );
  await Promise.all(
    [...defaultPaths, ...scenarioPaths, ...modelPaths].map((path) =>
      assertSafeWritePath(layout.root, path),
    ),
  );
  await Promise.all([
    ...layout.initialScenarios.map((scenario) =>
      assertScenarioCompatible(
        join(layout.root, 'default', 'scenarios', scenario.id + '.json'),
        scenario,
      ),
    ),
    ...layout.scenarios.map((scenario) =>
      assertScenarioCompatible(
        join(layout.root, 'scenarios', scenario.id + '.json'),
        scenario,
      ),
    ),
  ]);
};

const writeScenario = async (
  path: string,
  scenario: Scenario,
): Promise<void> => {
  if ((await current(path)) !== undefined) return;
  await atomicWrite(path, serializedScenario(scenario));
};

/** Persists the complete result only after collision checks pass. */
export const persistLayout = async (layout: Layout): Promise<void> => {
  if (layout.dryRun) return;
  await preflight(layout);
  await Promise.all([
    ...layout.initialScenarios.map((scenario) =>
      writeScenario(
        join(layout.root, 'default', 'scenarios', scenario.id + '.json'),
        scenario,
      ),
    ),
    ...layout.scenarios.map((scenario) =>
      writeScenario(
        join(layout.root, 'scenarios', scenario.id + '.json'),
        scenario,
      ),
    ),
    ...layout.targets.map(({ target, prompt }) =>
      atomicWrite(
        join(layout.root, target.id, 'SYSTEM_PROMPT.md'),
        prompt.trimEnd() + '\n',
      ),
    ),
  ]);
};
