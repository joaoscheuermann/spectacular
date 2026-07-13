import { readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';

import { createCompletionFor, type CompletionFor } from './completion.js';
import { loadConfig } from './config.js';
import { createJudges } from './evaluate.js';
import { evolveModels, mergeScenarios, type TargetResult } from './evolve.js';
import { persistLayout } from './layout.js';
import type { Progress } from './progress.js';
import { initializeScenarios } from './scenario-initialization.js';
import { loadScenarios } from './scenarios.js';
import type { EvolutionConfig } from './schema.js';

export type RunOptions = {
  readonly config: string;
  readonly dryRun: boolean;
};

export type RunSummary = {
  readonly dryRun: boolean;
  readonly root: string;
  readonly scenarioCount: number;
  readonly targets: readonly {
    readonly id: string;
    readonly score: number;
    readonly passed: number;
    readonly total: number;
    readonly epochsRun: number;
    readonly stopReason: TargetResult['stopReason'];
    readonly acceptedScenarioCount: number;
  }[];
};

type RunDependencies = {
  readonly completionFactory?: (config: EvolutionConfig) => CompletionFor;
  readonly progress?: Progress;
};

const optionalFile = async (path: string): Promise<string | undefined> => {
  try {
    return await readFile(path, 'utf8');
  } catch (error) {
    if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
      return undefined;
    }
    throw error;
  }
};

export const loadDefaultPrompt = async (root: string): Promise<string> => {
  const directory = join(root, 'default');
  const [markdown, text] = await Promise.all([
    optionalFile(join(directory, 'SYSTEM_PROMPT.md')),
    optionalFile(join(directory, 'SYSTEM_PROMPT.txt')),
  ]);
  if (markdown !== undefined && text !== undefined) {
    throw new Error(
      'Default prompt is ambiguous: keep only SYSTEM_PROMPT.md or SYSTEM_PROMPT.txt.',
    );
  }
  const prompt = markdown ?? text;
  if (prompt === undefined) {
    throw new Error(
      'Default prompt is missing: expected default/SYSTEM_PROMPT.md or default/SYSTEM_PROMPT.txt.',
    );
  }
  if (prompt.trim() === '')
    throw new Error('Default prompt must not be empty.');
  return prompt;
};

const summary = (
  dryRun: boolean,
  root: string,
  scenarioCount: number,
  targets: readonly TargetResult[],
): RunSummary => ({
  dryRun,
  root,
  scenarioCount,
  targets: targets.map(
    ({ target, evaluation, epochsRun, stopReason, acceptedScenarios }) => ({
      id: target.id,
      score: evaluation.score,
      passed: evaluation.passed,
      total: evaluation.total,
      epochsRun,
      stopReason,
      acceptedScenarioCount: acceptedScenarios.length,
    }),
  ),
});

/** Executes one prompt evolution run and returns a body-free summary. */
export const runEvolution = async (
  options: RunOptions,
  dependencies: RunDependencies = {},
): Promise<RunSummary> => {
  const configPath = resolve(options.config);
  const root = dirname(configPath);
  const [originalPrompt, config, scenarios] = await Promise.all([
    loadDefaultPrompt(root),
    loadConfig(configPath),
    loadScenarios(join(root, 'scenarios')),
  ]);
  const completeFor =
    dependencies.completionFactory?.(config) ?? createCompletionFor(config);
  const baseline =
    scenarios.length > 0
      ? scenarios
      : await initializeScenarios({
          optimizer: completeFor(config.optimizer),
          judges: createJudges(config.judges, completeFor),
          originalPrompt,
          maxAttempts: config.evolution.plateauPatience,
          progress: dependencies.progress,
        });
  const targets = await evolveModels(
    config,
    originalPrompt,
    baseline,
    completeFor,
    dependencies.progress,
  );
  const merged = mergeScenarios(baseline, targets);
  await persistLayout({
    root,
    initialScenarios: baseline,
    scenarios: merged,
    targets,
    dryRun: options.dryRun,
  });
  return summary(options.dryRun, root, merged.length, targets);
};
