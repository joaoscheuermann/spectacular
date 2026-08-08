import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

import { createFetchTransport, createOpenAiProvider } from 'llms';
import pino from 'pino';
import { z } from 'zod';

import {
  CONDITIONS,
  createConditionExecutor,
  createProviderBaselineModel,
  loadConditionPrompts,
  type MosaicDependencies,
} from '../conditions/index.js';
import {
  EMBEDDING_MODEL,
  PRIMARY_MODEL,
  RERANKER_MODEL,
} from '../config/index.js';
import { runSchedule } from '../core/commands.js';
import { artifactHash } from '../core/index.js';
import {
  createEventStore,
  createRecordStore,
  executeRun,
} from '../runtime/index.js';
import {
  CaseV1,
  FreezeManifestV1,
  RunSpecV1,
  type Case,
  type FreezeManifest,
  type RunSpec,
} from '../schemas/index.js';
import {
  DORIC_SMOKE_CONDITIONS,
  PILOT_CASES,
  confirmatoryCasesHash,
  localInstrumentHashes,
  seedLedgerHash,
  validateConfirmatoryCases,
} from '../study/index.js';
import type { CliInvocation } from './args.js';
import {
  booleanFlag,
  optionalFlag,
  rejectUnknownFlags,
  requiredFlag,
} from './args.js';
import { progress, readJson } from './io.js';
import { createMeteredProvider } from './provider.js';
import { parsePrices, pricesHash } from './pricing.js';
import { buildProductionRetrievers } from './retrievers.js';

const executeFile = promisify(execFile);
const OPENROUTER_BASE_URL = 'https://openrouter.ai/api/v1';

export const loadBenchmarkCases = async (
  path: string | undefined,
): Promise<readonly Case[]> => {
  const cases =
    path === undefined
      ? PILOT_CASES
      : z.array(CaseV1).parse(await readJson(path));
  cases.forEach((entry) => {
    const { contentHash, ...body } = entry;
    if (artifactHash(body) !== contentHash) {
      throw new Error(`case content hash mismatch: ${entry.id}`);
    }
  });
  return cases;
};

export const loadFreezeManifest = async (
  path: string | undefined,
): Promise<FreezeManifest | undefined> => {
  if (path === undefined) return undefined;
  const manifest = FreezeManifestV1.parse(await readJson(path));
  const { manifestHash, ...body } = manifest;
  if (artifactHash(body) !== manifestHash)
    throw new Error('freeze manifest hash mismatch');
  return manifest;
};

const cleanWorktree = async (): Promise<boolean> => {
  const result = await executeFile('git', [
    'status',
    '--porcelain=v1',
    '--untracked-files=all',
  ]);
  return result.stdout.trim().length === 0;
};

const validateSchedule = async (
  schedule: readonly RunSpec[],
  cases: readonly Case[],
  freeze: FreezeManifest | undefined,
  allowSmoke: boolean,
): Promise<void> => {
  if (schedule.length === 0) throw new TypeError('run schedule is empty');
  if (new Set(schedule.map(({ id }) => id)).size !== schedule.length) {
    throw new TypeError('run schedule IDs must be unique');
  }
  const caseIds = new Set(cases.map(({ id }) => id));
  const conditions = new Set([
    ...CONDITIONS.map(({ id }) => id),
    ...DORIC_SMOKE_CONDITIONS.map(({ id }) => id),
  ]);
  if (schedule.some(({ caseId }) => !caseIds.has(caseId))) {
    throw new TypeError('run schedule references an unavailable case');
  }
  if (schedule.some(({ conditionId }) => !conditions.has(conditionId))) {
    throw new TypeError('run schedule references an unavailable condition');
  }
  if (schedule.some(({ model }) => model.effort !== 'medium')) {
    throw new TypeError('every empirical run must use medium effort');
  }
  const hasSmoke = schedule.some(({ phase }) => phase === 'smoke');
  if (hasSmoke && !allowSmoke)
    throw new TypeError('Doric smokes require --doric-smoke');
  if (hasSmoke && schedule.some(({ phase }) => phase !== 'smoke')) {
    throw new TypeError('smoke runs cannot be mixed into a study schedule');
  }
  if (
    schedule.some(
      ({ phase, model }) =>
        phase === 'pilot' && model.model !== PRIMARY_MODEL.model,
    )
  ) {
    throw new TypeError('pilot runs require the frozen primary model');
  }

  const frozen = schedule.some(({ phase }) =>
    ['confirmatory', 'replication'].includes(phase),
  );
  if (!frozen) return;
  if (freeze === undefined)
    throw new TypeError('confirmatory runs require --freeze');
  if (!(await cleanWorktree()))
    throw new Error('confirmatory runs require a clean worktree');
  const commit = await executeFile('git', ['rev-parse', 'HEAD']);
  if (commit.stdout.trim() !== freeze.gitCommit) {
    throw new Error('confirmatory runs require the frozen git commit');
  }
  if (schedule.some(({ studyId }) => studyId !== freeze.studyId)) {
    throw new TypeError('run schedule study differs from the freeze');
  }
  if (schedule.some(({ freezeHash }) => freezeHash !== freeze.manifestHash)) {
    throw new TypeError('run schedule does not reference the supplied freeze');
  }
  const localHashes = await localInstrumentHashes();
  for (const name of Object.keys(localHashes) as (keyof typeof localHashes)[]) {
    if (localHashes[name] !== freeze.artifactHashes[name]) {
      throw new Error(`frozen ${name} hash differs from the local instrument`);
    }
  }
  if (
    confirmatoryCasesHash(cases) !== freeze.artifactHashes.confirmatoryCases
  ) {
    throw new Error('frozen confirmatory case hash mismatch');
  }
  const caseIssues = validateConfirmatoryCases(
    cases,
    PILOT_CASES,
    freeze.nFinal,
  );
  if (caseIssues.length > 0) {
    throw new TypeError(
      `frozen confirmatory corpus is invalid: ${caseIssues[0]?.code ?? 'unknown'}`,
    );
  }
  const studyConditions = new Set([freeze.selectedBaseline, 'M1']);
  const isMainSchedule = schedule.every(
    ({ conditionId, oracleParentRunId }) =>
      studyConditions.has(conditionId) && oracleParentRunId === undefined,
  );
  if (
    isMainSchedule &&
    seedLedgerHash(schedule) !== freeze.artifactHashes.seeds
  ) {
    throw new Error('frozen paired seed ledger hash mismatch');
  }
  if (
    isMainSchedule &&
    schedule.some(({ capture }) => capture !== 'structure')
  ) {
    throw new TypeError('confirmatory study runs require structural capture');
  }
  const frozenPhases = new Set(schedule.map(({ phase }) => phase));
  if (frozenPhases.size !== 1) {
    throw new TypeError(
      'primary and replication schedules must remain separate',
    );
  }
  const budgets = new Set(
    schedule.map(({ modelCallBudget }) => modelCallBudget ?? null),
  );
  if (budgets.size !== 1) {
    throw new TypeError('capped and uncapped runs cannot share a schedule');
  }
  schedule.forEach((run) => {
    if (
      run.phase === 'confirmatory' &&
      artifactHash(run.model) !== artifactHash(freeze.primaryModel)
    ) {
      throw new TypeError('confirmatory run model differs from the freeze');
    }
    if (
      run.phase === 'replication' &&
      artifactHash(run.model) !== artifactHash(freeze.replication.candidate)
    ) {
      throw new TypeError('replication run model differs from the freeze');
    }
    if (
      run.phase === 'confirmatory' &&
      run.modelCallBudget !== undefined &&
      run.modelCallBudget !== freeze.modelCallBudgetP95
    ) {
      throw new TypeError(
        'confirmatory cap differs from the frozen p95 budget',
      );
    }
    if (run.phase === 'replication' && run.modelCallBudget !== undefined) {
      throw new TypeError('replication schedule must remain uncapped');
    }
  });
};

const conditionFor = (id: string) => {
  const condition = [...CONDITIONS, ...DORIC_SMOKE_CONDITIONS].find(
    (entry) => entry.id === id,
  );
  if (condition === undefined) throw new TypeError(`unknown condition: ${id}`);
  return condition;
};

const oracleDependencies: NonNullable<MosaicDependencies['oracles']> = {
  state: async (input, benchmarkCase) => ({
    decision: {
      status: 'completed',
      criteria: input.node.doneWhen.map((criterion, index) => ({
        criterionIndex: index,
        satisfied: true,
        evidence: `Oracle state satisfies: ${criterion}`,
      })),
      result: {
        markdown: JSON.stringify(
          benchmarkCase.gold.expectedDelivery ??
            benchmarkCase.gold.expectedState,
        ),
        artifacts: [],
      },
      revisionRequest: null,
      reason: null,
    },
    observations: [],
  }),
  revision: async (input, benchmarkCase) => ({
    nodes: input.graph.nodes.map((node) =>
      node.id === input.target.id
        ? {
            id: node.id,
            goal: `${node.goal}\nOracle correction: ${benchmarkCase.request}`,
            doneWhen: benchmarkCase.gold.criteria.map(
              ({ description }) => description,
            ),
            dependsOn: [...node.dependsOn],
            deliver: node.deliver,
          }
        : {
            id: node.id,
            goal: node.goal,
            doneWhen: [...node.doneWhen],
            dependsOn: [...node.dependsOn],
            deliver: node.deliver,
          },
    ),
  }),
};

/** Executes a validated schedule against the frozen provider composition. */
export const runProductionSchedule = async (
  invocation: CliInvocation,
): Promise<unknown> => {
  rejectUnknownFlags(invocation, [
    'schedule',
    'cases',
    'artifacts',
    'prices',
    'freeze',
    'resume',
    'doric-smoke',
  ]);
  const schedule = z
    .array(RunSpecV1)
    .parse(await readJson(requiredFlag(invocation, 'schedule')));
  const cases = await loadBenchmarkCases(optionalFlag(invocation, 'cases'));
  const freeze = await loadFreezeManifest(optionalFlag(invocation, 'freeze'));
  const allowSmoke = booleanFlag(invocation, 'doric-smoke');
  await validateSchedule(schedule, cases, freeze, allowSmoke);

  const prices = parsePrices(
    await readJson(requiredFlag(invocation, 'prices')),
  );
  if (
    freeze !== undefined &&
    pricesHash(prices) !== freeze.artifactHashes.prices
  ) {
    throw new Error('frozen prices hash mismatch');
  }
  const apiKey = process.env['OPENROUTER_API_KEY'];
  if (apiKey === undefined || apiKey.trim().length === 0) {
    throw new Error('OPENROUTER_API_KEY is required');
  }
  const logger = pino({ level: 'silent' });
  const source = createOpenAiProvider({
    transport: createFetchTransport(),
    baseUrl: OPENROUTER_BASE_URL,
    apiKey,
    logger,
  });
  const meter = createMeteredProvider(source, prices);
  const retrievers = await buildProductionRetrievers(meter.provider, logger);
  meter.reset();
  const prompts = await loadConditionPrompts();
  if (freeze !== undefined && prompts.hash !== freeze.artifactHashes.prompts) {
    throw new Error('frozen prompt hash mismatch');
  }

  const root = requiredFlag(invocation, 'artifacts');
  const events = createEventStore(root);
  const records = createRecordStore(root);
  const byCase = new Map(cases.map((entry) => [entry.id, entry]));

  return runSchedule(schedule, {
    resume: booleanFlag(invocation, 'resume'),
    readAttempts: records.read,
    execute: async (run) => {
      const benchmarkCase = byCase.get(run.caseId);
      if (benchmarkCase === undefined)
        throw new TypeError('run case is unavailable');
      const condition = conditionFor(run.conditionId);
      const parent =
        run.oracleParentRunId === undefined
          ? undefined
          : (await records.read(run.oracleParentRunId)).at(-1);
      meter.reset();
      const profile = {
        model: run.model.model,
        effort: run.model.effort,
      } as const;
      const mosaic: MosaicDependencies = {
        base: {
          logger,
          provider: meter.provider,
          models: {
            planning: profile,
            revision: profile,
            execution: profile,
            reranker: RERANKER_MODEL,
            embedder: EMBEDDING_MODEL.model,
          },
          routing: {
            maxHintCandidates: 5,
            maxRetrievedCandidates: 5,
            maxSkills: 3,
          },
          execution: { maxTurns: 16 },
          revision: { max: 3 },
        },
        retrievers,
        retrieverModels: {
          skills: EMBEDDING_MODEL.model,
          metadataSkills: EMBEDDING_MODEL.model,
        },
        usage: meter.snapshot,
        oracles: oracleDependencies,
      };
      const execute = createConditionExecutor({
        baseline: createProviderBaselineModel({
          provider: meter.provider,
          model: run.model.model,
          cost: (usage) => meter.costFor(run.model.model, usage),
        }),
        prompts,
        mosaic,
      });
      progress(`run ${run.order + 1}/${schedule.length}: ${run.id}`);
      return executeRun(
        run,
        benchmarkCase,
        condition,
        {
          events,
          records,
          execute,
          usage: meter.snapshot,
        },
        parent,
      );
    },
  });
};
