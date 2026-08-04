import { readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';

import type { Logger } from 'pino';
import { createCompletionFor, type CompletionFor } from './completion.js';
import { loadConfig } from './config.js';
import { evolveModels, type TargetResult } from './evolve.js';
import {
  appendHistory,
  historyFingerprint,
  loadHistory,
  preflightHistory,
  type HistoryRecord,
} from './history.js';
import { persistLayout, preflightLayout } from './layout.js';
import type { Progress } from './progress.js';
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
    readonly trainingAccuracy: number;
    readonly validationAccuracy?: number;
    readonly approved: boolean;
    readonly refactored: boolean;
    readonly epochsRun: number;
    readonly stopReason: TargetResult['stopReason'];
  }[];
};

export type RunDependencies = {
  readonly logger: Logger;
  readonly completionFactory?: (config: EvolutionConfig) => CompletionFor;
  readonly progress?: Progress;
  readonly warn?: (message: string) => void;
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

const summarize = (
  dryRun: boolean,
  root: string,
  scenarioCount: number,
  targets: readonly TargetResult[],
): RunSummary => ({
  dryRun,
  root,
  scenarioCount,
  targets: targets.map(
    ({
      target,
      trainingAccuracy,
      validationAccuracy,
      approved,
      refactored,
      epochsRun,
      stopReason,
    }) => ({
      id: target.id,
      trainingAccuracy,
      ...(validationAccuracy === undefined ? {} : { validationAccuracy }),
      approved,
      refactored,
      epochsRun,
      stopReason,
    }),
  ),
});

type LoadedHistory = {
  readonly records: Readonly<Record<string, readonly HistoryRecord[]>>;
  readonly errors: Readonly<Record<string, Error>>;
};

const matchingHistory = async (
  root: string,
  prompt: string,
  config: EvolutionConfig,
  scenarios: Awaited<ReturnType<typeof loadScenarios>>,
): Promise<LoadedHistory> => {
  const training = scenarios.filter(({ split }) => split === 'train');
  const settled = await Promise.all(
    config.models.map(async (target) => {
      const fingerprint = historyFingerprint({
        originalPrompt: prompt,
        training,
        globalEvals: config.evals,
        accuracy: config.evolution.accuracy,
        target,
        judge: config.judge,
      });
      try {
        const records = await loadHistory(
          root,
          target.id,
          fingerprint,
          config.evolution.history.limit,
        );
        return { status: 'loaded', targetId: target.id, records } as const;
      } catch (error) {
        return {
          status: 'failed',
          targetId: target.id,
          error:
            error instanceof Error
              ? error
              : new Error('Unknown history error.'),
        } as const;
      }
    }),
  );
  const records: Record<string, readonly HistoryRecord[]> = {};
  const errors: Record<string, Error> = {};
  for (const entry of settled) {
    if (entry.status === 'loaded') {
      records[entry.targetId] = entry.records;
    } else {
      errors[entry.targetId] = entry.error;
    }
  }
  return { records, errors };
};

/** Executes one assertion-based evolution run and returns a body-free summary. */
export const runEvolution = async (
  options: RunOptions,
  dependencies: RunDependencies,
): Promise<RunSummary> => {
  const configPath = resolve(options.config);
  const root = dirname(configPath);
  const config = await loadConfig(configPath);
  const [originalPrompt, scenarios] = await Promise.all([
    loadDefaultPrompt(root),
    loadScenarios(join(root, 'scenarios'), config.evals),
  ]);
  const history = await matchingHistory(
    root,
    originalPrompt,
    config,
    scenarios,
  );
  const completeFor =
    dependencies.completionFactory?.(config) ??
    createCompletionFor(config, { logger: dependencies.logger });
  const targets = await evolveModels(
    config,
    originalPrompt,
    scenarios,
    completeFor,
    dependencies.progress,
    {
      history: history.records,
      historyErrors: history.errors,
      warn: dependencies.warn,
      onResult: async (target) => {
        if (options.dryRun) return;
        await Promise.all([
          preflightHistory(root, target.target.id),
          preflightLayout({ root, targets: [target], dryRun: false }),
        ]);
        await appendHistory(root, target.target.id, target.history);
        await persistLayout({ root, targets: [target], dryRun: false });
      },
    },
  );
  return summarize(options.dryRun, root, scenarios.length, targets);
};
