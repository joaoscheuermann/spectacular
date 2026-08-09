import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import {
  ExecutionRecordV1,
  ConditionV1,
  type ExecutionRecord,
} from '../schemas/index.js';
import { SKILLS, validateCatalog } from '../catalog/index.js';
import { TOOL_NAMES } from '../config/index.js';
import {
  CONDITIONS,
  M0,
  M1,
  ORACLES,
  boundaryPolicy,
  loadConditionPrompts,
  oracleEligible,
  validateAblationMatrix,
  validatePrimaryControlParity,
} from '../conditions/index.js';
import { artifactHash } from '../core/hash.js';
import { canonicalJson } from '../core/json.js';
import { createPrng } from '../core/prng.js';
import {
  TOOLS,
  createEventStore,
  createWorld,
  executeTool,
} from '../runtime/index.js';
import {
  PILOT_CASES,
  validateCases,
  validateFamilyIsolation,
} from './cases.js';
import { writeFreeze, type FreezeInput } from './freeze.js';
import {
  SCHEDULE_SEED_SCOPE,
  createSchedule,
  resumeAction,
} from './scheduler.js';
import { selectBaseline } from './selection.js';
import { DORIC_SMOKE_CASES, DORIC_SMOKE_CONDITIONS } from './smoke.js';
import { DECISIONS, type Decision } from './decisions.js';

export { DECISIONS, type Decision } from './decisions.js';

const HASH = `sha256:${'a'.repeat(64)}`;

const record = (
  attempt: number,
  firstModelCallStarted: boolean,
): ExecutionRecord =>
  ExecutionRecordV1.parse({
    schemaVersion: 1,
    attempt,
    run: {
      schemaVersion: 1,
      id: 'run.resume',
      studyId: 'study',
      phase: 'pilot',
      caseId: 'case',
      conditionId: 'M1',
      repetition: 1,
      seed: 1,
      pairedBlock: 'block',
      order: 0,
      model: {
        provider: 'openai',
        model: 'openai/gpt-5.6-luna',
        effort: 'medium',
      },
      capture: 'structure',
      freezeHash: null,
    },
    status: 'infrastructure',
    startedAt: '2026-08-08T00:00:00.000Z',
    finishedAt: '2026-08-08T00:00:01.000Z',
    durationMs: 1000,
    firstModelCallStarted,
    trace: {
      rootHash: HASH,
      derivedHash: HASH,
      eventCount: 1,
      relativePath: 'trace.json',
    },
    outcome: null,
    worldHash: HASH,
    usage: {
      inputTokens: 0,
      outputTokens: 0,
      modelCalls: firstModelCallStarted ? 1 : 0,
      toolCalls: 0,
      costUsd: 0,
    },
    infrastructureFailure: {
      stage: firstModelCallStarted ? 'model' : 'prepare',
      code: 'interrupted',
      beforeFirstModelCall: !firstModelCallStarted,
    },
  });

const freezeInput = (): FreezeInput => ({
  schemaVersion: 1,
  studyId: 'study',
  frozenAt: '2026-08-08T00:00:00.000Z',
  gitCommit: 'a'.repeat(40),
  cleanWorktree: true,
  primaryModel: {
    provider: 'openai',
    model: 'openai/gpt-5.6-luna',
    effort: 'medium',
  },
  reranker: 'voyageai/rerank-2.5-lite',
  embedder: { model: 'voyageai/voyage-4-large', dimensions: 2048 },
  selectedBaseline: 'B0',
  repetitions: 5,
  modelCallBudgetP95: 12,
  alpha: 0.05,
  minimumEffect: 0.1,
  targetPower: 0.8,
  nPower: 24,
  nFinal: 240,
  semanticReviewFraction: 0.2,
  artifactHashes: {
    protocol: HASH,
    schemas: HASH,
    catalog: HASH,
    tools: HASH,
    pilotCases: HASH,
    pilotScores: HASH,
    confirmatoryCases: HASH,
    conditions: HASH,
    prompts: HASH,
    prices: HASH,
    seeds: HASH,
    calibration: HASH,
    calibrationAudit: HASH,
    confirmatoryAudit: HASH,
    costApproval: HASH,
    powerConfig: HASH,
    powerApproval: HASH,
    powerResult: HASH,
    retrievalIndex: HASH,
    analysis: HASH,
    renvLock: HASH,
  },
  replication: {
    candidate: {
      provider: 'openrouter',
      model: 'qwen/qwen3.7-flash',
      effort: 'medium',
    },
    neutralCases: 60,
    repetitions: 3,
    difference: 0,
    confidence95: { lower: -0.01, upper: 0.01 },
    approved: true,
  },
  containerDigest: HASH,
});

const temporary = async <T>(
  operation: (directory: string) => Promise<T>,
): Promise<T> => {
  const directory = await mkdtemp(join(tmpdir(), 'mosaic-conformance-'));
  try {
    return await operation(directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
};

type Check = () => boolean | Promise<boolean>;

const CHECKS: Readonly<Record<Decision['id'], Check>> = {
  D01: () => SKILLS.length === 60,
  D02: () =>
    ['documents-finance', 'software', 'artifacts', 'communication'].every(
      (domain) =>
        SKILLS.filter((skill) => skill.domain === domain).length === 15,
    ),
  D03: () => validateCatalog(SKILLS).length === 0,
  D04: () =>
    TOOLS.length === 24 &&
    TOOLS.map((tool) => tool.name).join() === TOOL_NAMES.join(),
  D05: () => {
    const changed = executeTool(createWorld(), 'write', {
      path: 'isolated.txt',
      content: 'changed',
    }).world;
    return (
      'isolated.txt' in changed.files &&
      !('isolated.txt' in createWorld().files)
    );
  },
  D06: () => {
    const input = { operation: 'sum', values: [1, 2, 3] } as const;
    return (
      canonicalJson(executeTool(createWorld(), 'calculate', input).output) ===
      canonicalJson(executeTool(createWorld(), 'calculate', input).output)
    );
  },
  D07: () =>
    validateCases(PILOT_CASES).every(
      (issue) => issue.code !== 'pilot_cardinality',
    ),
  D08: () =>
    ['A', 'B', 'C', 'D', 'E', 'F'].every(
      (value) =>
        PILOT_CASES.filter((entry) => entry.compositionClass === value)
          .length === 10,
    ) &&
    validateCases(PILOT_CASES).every(
      (issue) =>
        issue.code !== 'composition_class' &&
        issue.code !== 'composition_signature',
    ),
  D09: () =>
    ['documents-finance', 'software', 'artifacts', 'communication'].every(
      (domain) =>
        PILOT_CASES.filter((entry) => entry.domain === domain).length === 15,
    ),
  D10: () =>
    validateFamilyIsolation(PILOT_CASES, [
      PILOT_CASES[0] as (typeof PILOT_CASES)[number],
    ]).length === 1,
  D11: () => CONDITIONS.every((entry) => ConditionV1.safeParse(entry).success),
  D12: () => validateAblationMatrix().length === 0,
  D13: () =>
    boundaryPolicy(M0).feedbackPlan === 'unchanged' &&
    boundaryPolicy(M1).feedbackPlan === 'default',
  D14: () =>
    ORACLES.every(
      (entry) =>
        entry.eligibility === 'failures-only' &&
        !oracleEligible(
          entry,
          PILOT_CASES[0] as (typeof PILOT_CASES)[number],
          undefined,
        ),
    ),
  D15: () => {
    const left = createPrng('seed');
    const right = createPrng('seed');
    return Array.from({ length: 20 }, () => left.next()).every(
      (value) => value === right.next(),
    );
  },
  D16: () => artifactHash({ b: 2, a: 1 }) === artifactHash({ a: 1, b: 2 }),
  D17: () =>
    temporary(async (directory) => {
      const store = createEventStore(directory);
      await Promise.all([
        store.append('run.concurrent', 1, { value: 'a' }),
        store.append('run.concurrent', 1, { value: 'b' }),
      ]);
      const events = await store.read('run.concurrent', 1);
      return (
        events.length === 2 &&
        events[0]?.sequence === 1 &&
        events[1]?.sequence === 2
      );
    }),
  D18: () =>
    temporary(async (directory) => {
      const store = createEventStore(directory);
      await store.append('run.chain', 1, { value: 1 });
      await store.append('run.chain', 1, { value: 2 });
      const events = await store.read('run.chain', 1);
      return events[1]?.previousHash === events[0]?.eventHash;
    }),
  D19: () =>
    temporary(async (directory) => {
      const store = createEventStore(directory);
      await store.append('run.derived', 1, { value: 1 });
      const first = await store.derive('run.derived', 1);
      const second = await store.derive('run.derived', 1);
      return canonicalJson(first) === canonicalJson(second);
    }),
  D20: () => {
    const schedule = createSchedule({
      studyId: 'study',
      cases: PILOT_CASES.slice(0, 2),
      conditions: [M0, M1],
      seed: 'paired',
      repetitions: 2,
    });
    const blocks = [...new Set(schedule.map((run) => run.pairedBlock))];
    return blocks.every((block) => {
      const runs = schedule.filter((run) => run.pairedBlock === block);
      return (
        runs.length === 2 &&
        new Set(runs.map((run) => run.seed)).size === 1 &&
        new Set(runs.map((run) => run.conditionId)).size === 2 &&
        SCHEDULE_SEED_SCOPE === 'condition-order-and-deterministic-hooks-only'
      );
    });
  },
  D21: () =>
    resumeAction([record(1, false)]) === 'retry-technical' &&
    resumeAction([record(1, false), record(2, false)]) === 'skip-terminal',
  D22: () => resumeAction([record(1, true)]) === 'skip-terminal',
  D23: () =>
    selectBaseline([
      { conditionId: 'B0', success: 1, costUsd: 2 },
      { conditionId: 'B1', success: 1, costUsd: 1 },
      { conditionId: 'B2', success: 0, costUsd: 0 },
      { conditionId: 'B3', success: 0, costUsd: 0 },
    ]).conditionId === 'B1',
  D24: () =>
    temporary(async (directory) => {
      const path = join(directory, 'freeze.json');
      await writeFreeze(path, freezeInput());
      return writeFreeze(path, freezeInput()).then(
        () => false,
        () => true,
      );
    }),
  D25: () =>
    DORIC_SMOKE_CASES.length === 6 &&
    DORIC_SMOKE_CONDITIONS.length === 6 &&
    new Set(DORIC_SMOKE_CASES.map((entry) => entry.compositionClass)).size ===
      6 &&
    DORIC_SMOKE_CASES.every((entry) => entry.phase === 'smoke') &&
    DORIC_SMOKE_CONDITIONS.every(
      (entry) => entry.kind === 'smoke' && entry.eligibility === 'opt-in',
    ),
};

export interface ConformanceIssue {
  readonly id: string;
  readonly testName: string;
  readonly detail: string;
}

/** Executes one named decision check for the test suite and focused diagnostics. */
export const runDecision = async (id: Decision['id']): Promise<boolean> => {
  const check = CHECKS[id];
  if (check === undefined || !DECISIONS.some((entry) => entry.id === id))
    return false;
  try {
    return await check();
  } catch {
    return false;
  }
};

/** Executes D01-D25 and fails closed on missing IDs, checks, or test names. */
export const runConformance = async (): Promise<
  readonly ConformanceIssue[]
> => {
  const expected = Array.from(
    { length: 25 },
    (_, index) => `D${String(index + 1).padStart(2, '0')}`,
  );
  const ids = DECISIONS.map((entry) => entry.id);
  const registryIssues = expected.flatMap((id) => {
    const entry = DECISIONS.find((decisionEntry) => decisionEntry.id === id);
    if (entry === undefined)
      return [{ id, testName: '', detail: 'missing decision' }];
    if (entry.testName.trim() === '')
      return [{ id, testName: '', detail: 'missing test name' }];
    return id in CHECKS
      ? []
      : [{ id, testName: entry.testName, detail: 'missing executable check' }];
  });
  if (new Set(ids).size !== 25 || ids.length !== 25) {
    registryIssues.push({
      id: '$',
      testName: '',
      detail: 'decision registry must contain exactly D01-D25',
    });
  }
  if (validatePrimaryControlParity().length > 0) {
    registryIssues.push({
      id: '$condition-parity',
      testName:
        'primary conditions freeze common retrieval, turn, and repair controls',
      detail:
        'undeclared experimental controls differ between primary conditions',
    });
  }
  try {
    const prompts = await loadConditionPrompts();
    if (
      ![
        prompts.baseline,
        prompts.planner,
        prompts.executor,
        prompts.revision,
      ].every((prompt) => prompt.startsWith('#'))
    ) {
      registryIssues.push({
        id: '$prompts',
        testName: 'built conformance loads frozen Markdown prompts',
        detail: 'prompt artifact is not human-readable Markdown',
      });
    }
  } catch {
    registryIssues.push({
      id: '$prompts',
      testName: 'built conformance loads frozen Markdown prompts',
      detail: 'frozen prompt artifact is unavailable',
    });
  }
  const results: readonly (ConformanceIssue | null)[] = await Promise.all(
    DECISIONS.map(
      async (entry): Promise<ConformanceIssue | null> =>
        (await runDecision(entry.id))
          ? null
          : {
              id: entry.id,
              testName: entry.testName,
              detail: entry.requirement,
            },
    ),
  );
  return [
    ...registryIssues,
    ...results.filter((issue): issue is ConformanceIssue => issue !== null),
  ];
};
