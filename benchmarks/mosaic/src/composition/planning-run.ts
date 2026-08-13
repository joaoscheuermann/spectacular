import { mkdir, stat, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

import type { ReasoningEffort } from 'llms';
import type { MosaicOptions } from 'mosaic';
import pino from 'pino';

import { planningCases } from './cases/index.js';
import { planningBenchmarkManifest } from './planning-artifacts.js';
import {
  aggregatePlanningRuns,
  type PlanningAggregateMetrics,
} from './planning-metrics.js';
import {
  createPlanningModelAdapter,
  type PlanningModelEvent,
} from './planning-model.js';
import { runPlanningConditions } from './planning-runner.js';
import type { CompositionProfile } from './runner.js';

export interface PlanningBenchmarkRunOptions {
  readonly profile: CompositionProfile;
  readonly outputDir: string;
  readonly caseIds?: readonly string[];
  readonly effort?: ReasoningEffort;
  readonly maxTurns?: number;
  readonly signal?: AbortSignal;
  readonly progress?: (event: PlanningRunProgress) => void | Promise<void>;
}

export type PlanningRunProgress =
  | { readonly type: 'run.started'; readonly caseCount: number }
  | {
      readonly type: 'case.started';
      readonly caseId: string;
      readonly caseIndex: number;
      readonly caseCount: number;
    }
  | PlanningModelEvent
  | {
      readonly type: 'case.completed';
      readonly caseId: string;
      readonly caseIndex: number;
      readonly caseCount: number;
      readonly conditionCount: number;
      readonly modelCallCount: number;
    }
  | {
      readonly type: 'run.completed';
      readonly caseCount: number;
      readonly modelCallCount: number;
    };

export interface PlanningBenchmarkRunSummary {
  readonly benchmark: 'mosaic-p0-p1-controlled';
  readonly outputDir: string;
  readonly model: string;
  readonly caseCount: number;
  readonly modelCallCount: number;
  readonly metrics: PlanningAggregateMetrics;
}

/** Runs a fresh, closed controlled planning campaign and persists every trace. */
export const runPlanningBenchmark = async (
  options: PlanningBenchmarkRunOptions,
): Promise<PlanningBenchmarkRunSummary> => {
  const outputDir = resolve(options.outputDir);
  await requireAbsent(outputDir);
  const selected = selectCases(options.caseIds);
  const manifest = planningBenchmarkManifest();
  const runIdentity = {
    schemaVersion: 1,
    benchmark: manifest.benchmark,
    casesSha256: manifest.casesSha256,
    caseIds: selected.map(({ id }) => id),
    model: options.profile.model,
    effort: options.effort ?? 'low',
    maxTurns: options.maxTurns ?? 3,
    judge: 'same-model-rubric-observation',
    retrieval: 'deterministic-positive-lexical-v1',
  } as const;
  await mkdir(join(outputDir, 'cases'), { recursive: true });
  await writeJson(join(outputDir, 'run.json'), runIdentity);
  await options.progress?.({ type: 'run.started', caseCount: selected.length });

  const runs = [];
  let modelCallCount = 0;
  for (const [index, benchmarkCase] of selected.entries()) {
    const caseIndex = index + 1;
    await options.progress?.({
      type: 'case.started',
      caseId: benchmarkCase.id,
      caseIndex,
      caseCount: selected.length,
    });
    const events: PlanningModelEvent[] = [];
    let caseModelCallCount = 0;
    const adapter = createPlanningModelAdapter({
      provider: options.profile.provider,
      model: options.profile.model,
      effort: options.effort ?? 'low',
      maxTurns: options.maxTurns ?? 3,
      ...(options.signal === undefined ? {} : { signal: options.signal }),
      onEvent: async (event) => {
        events.push(event);
        if (event.type === 'model.call') {
          modelCallCount += 1;
          caseModelCallCount += 1;
        }
        await options.progress?.(event);
      },
    });
    const results = await runPlanningConditions({
      case: benchmarkCase,
      mosaic: engineOptions(options.profile),
      adapter,
    });
    const run = {
      case: {
        id: benchmarkCase.id,
        domain: benchmarkCase.domain,
        compositionClass: benchmarkCase.compositionClass,
      },
      results,
    };
    runs.push(run);
    await writeJson(join(outputDir, 'cases', `${benchmarkCase.id}.json`), {
      ...run,
      events,
    });
    await options.progress?.({
      type: 'case.completed',
      caseId: benchmarkCase.id,
      caseIndex,
      caseCount: selected.length,
      conditionCount: results.length,
      modelCallCount: caseModelCallCount,
    });
  }

  const summary: PlanningBenchmarkRunSummary = {
    benchmark: 'mosaic-p0-p1-controlled',
    outputDir,
    model: options.profile.model,
    caseCount: runs.length,
    modelCallCount,
    metrics: aggregatePlanningRuns(runs),
  };
  await writeJson(join(outputDir, 'summary.json'), summary);
  await options.progress?.({
    type: 'run.completed',
    caseCount: runs.length,
    modelCallCount,
  });
  return summary;
};

const selectCases = (ids: readonly string[] | undefined) => {
  if (ids === undefined) return planningCases;
  if (ids.length === 0 || new Set(ids).size !== ids.length)
    throw new Error('Planning case selection must be non-empty and unique.');
  const byId = new Map(
    planningCases.map((benchmarkCase) => [benchmarkCase.id, benchmarkCase]),
  );
  return ids.map((id) => {
    const benchmarkCase = byId.get(id);
    if (benchmarkCase === undefined)
      throw new Error(`Unknown planning case: ${id}`);
    return benchmarkCase;
  });
};

const engineOptions = (profile: CompositionProfile): MosaicOptions => ({
  logger: pino({ enabled: false }),
  providers: {
    planning: profile.provider,
    revision: profile.provider,
    execution: profile.provider,
    reranker: profile.rerankerProvider ?? profile.provider,
  },
  models: {
    planning: { model: profile.model, effort: 'low' },
    revision: { model: profile.model, effort: 'low' },
    execution: { model: profile.model, effort: 'low' },
    reranker: profile.rerankerModel ?? profile.model,
    embedder: profile.model,
  },
  routing: {
    maxHintCandidates: 1,
    maxRetrievedCandidates: 1,
    maxSkills: 1,
  },
  execution: { maxTurns: 1 },
  revision: { max: 0 },
  skills: { required: [], menu: [], retriever: noSearch },
  tools: { required: [], menu: [], retriever: noSearch },
});

const noSearch = { search: async (): Promise<readonly never[]> => [] };

const requireAbsent = async (path: string): Promise<void> => {
  try {
    await stat(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return;
    throw error;
  }
  throw new Error(`Planning output directory already exists: ${path}`);
};

const writeJson = (path: string, value: unknown): Promise<void> =>
  writeFile(path, `${JSON.stringify(value, null, 2)}\n`, {
    encoding: 'utf8',
    flag: 'wx',
  });
