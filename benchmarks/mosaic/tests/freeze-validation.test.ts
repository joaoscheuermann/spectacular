import assert from 'node:assert/strict';
import test from 'node:test';

import { HARNESS_COMMANDS } from '../src/core/commands.js';
import { B0, M1 } from '../src/conditions/index.js';
import { artifactHash } from '../src/core/index.js';
import {
  validateFreezeSchedule,
  validatePilotFreeze,
} from '../src/cli/freeze.js';
import { validateFrozenScoreCorpus } from '../src/cli/score.js';
import {
  CaseV1,
  FreezeManifestV1,
  ScoreRowV1,
  type Case,
  type FreezeManifest,
  type ScoreRow,
} from '../src/schemas/index.js';
import {
  PILOT_CASES,
  confirmatoryCasesHash,
  createSchedule,
  seedLedgerHash,
} from '../src/study/index.js';

const hash = `sha256:${'a'.repeat(64)}`;

const manifest = (overrides: Partial<FreezeManifest> = {}): FreezeManifest =>
  FreezeManifestV1.parse({
    schemaVersion: 1,
    studyId: 'study',
    frozenAt: '2026-08-08T12:00:00.000Z',
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
    alpha: 0.05,
    minimumEffect: 0.1,
    targetPower: 0.8,
    nPower: 240,
    nFinal: 240,
    modelCallBudgetP95: 2,
    semanticReviewFraction: 0.2,
    artifactHashes: {
      protocol: hash,
      schemas: hash,
      catalog: hash,
      tools: hash,
      pilotCases: hash,
      pilotScores: hash,
      confirmatoryCases: hash,
      conditions: hash,
      prompts: hash,
      prices: hash,
      seeds: hash,
      calibration: hash,
      calibrationAudit: hash,
      confirmatoryAudit: hash,
      costApproval: hash,
      powerConfig: hash,
      powerApproval: hash,
      powerResult: hash,
      retrievalIndex: hash,
      analysis: hash,
      renvLock: hash,
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
    containerDigest: hash,
    manifestHash: hash,
    ...overrides,
  });

const pilotScores = (): readonly ScoreRow[] => {
  const byId = new Map(PILOT_CASES.map((entry) => [entry.id, entry]));
  return HARNESS_COMMANDS.pilot('study', 'pilot-seed').map((run) => {
    const benchmarkCase = byId.get(run.caseId)!;
    return ScoreRowV1.parse({
      schemaVersion: 1,
      runId: run.id,
      attempt: 1,
      studyId: run.studyId,
      phase: run.phase,
      caseId: run.caseId,
      familyId: benchmarkCase.familyId,
      conditionId: run.conditionId,
      repetition: run.repetition,
      pairedBlock: run.pairedBlock,
      provider: run.model.provider,
      model: run.model.model,
      effort: run.model.effort,
      freezeHash: null,
      traceRootHash: hash,
      traceDerivedHash: hash,
      evidenceHash: hash,
      worldHash: hash,
      modelCallBudget: null,
      domain: benchmarkCase.domain,
      compositionClass: benchmarkCase.compositionClass,
      adaptive: benchmarkCase.adaptive,
      success: 1,
      primaryEligible: false,
      infrastructure: false,
      failureCode: null,
      retrieval: null,
      bundleExact: null,
      menuExact: null,
      observationExact: null,
      inputTokens: 0,
      outputTokens: 0,
      modelCalls: 2,
      toolCalls: 0,
      costUsd: run.conditionId === 'B0' ? 1 : 2,
      durationMs: 1,
    });
  });
};

const confirmatoryCases = (): readonly Case[] => {
  const domains = [
    'documents-finance',
    'software',
    'artifacts',
    'communication',
  ] as const;
  const classes = ['A', 'B', 'C', 'D', 'E', 'F'] as const;
  return classes.flatMap((compositionClass) =>
    domains.flatMap((domain) => {
      const source = PILOT_CASES.find(
        (entry) =>
          entry.compositionClass === compositionClass &&
          entry.domain === domain,
      )!;
      return Array.from({ length: 10 }, (_, index) => {
        const ordinal = `${compositionClass}.${domain}.${index + 1}`;
        const { contentHash: _contentHash, ...sourceBody } = source;
        void _contentHash;
        const body = {
          ...sourceBody,
          id: `confirmatory.case.${ordinal}`,
          familyId: `confirmatory.family.${ordinal}`,
          phase: 'confirmatory' as const,
          title: `Independent ${ordinal}`,
          request: `${source.request}\nIndependent scenario ${ordinal}.`,
          tags: [
            'confirmatory',
            `domain.${domain}`,
            `class.${compositionClass}`,
          ],
        };
        return CaseV1.parse({ ...body, contentHash: artifactHash(body) });
      });
    }),
  );
};

test('freeze accepts only the complete pilot and recomputed baseline choice', () => {
  const rows = pilotScores();
  assert.doesNotThrow(() => validatePilotFreeze(rows, manifest()));
  assert.throws(
    () => validatePilotFreeze(rows.slice(1), manifest()),
    /complete frozen pilot/u,
  );
  assert.throws(
    () => validatePilotFreeze(rows, manifest({ selectedBaseline: 'B1' })),
    /tie-break rule/u,
  );
});

test('freeze and scoring require the same complete paired confirmatory grid', () => {
  const cases = confirmatoryCases();
  const schedule = createSchedule({
    studyId: 'study',
    cases,
    conditions: [B0, M1],
    seed: 'confirmatory-seed',
    repetitions: 5,
  });
  const finalized = schedule.map((run) => ({ ...run, freezeHash: hash }));
  const frozen = manifest({
    artifactHashes: {
      ...manifest().artifactHashes,
      confirmatoryCases: confirmatoryCasesHash(cases),
      seeds: seedLedgerHash(finalized),
    },
  });
  assert.doesNotThrow(() => validateFreezeSchedule(frozen, cases, schedule));
  assert.doesNotThrow(() =>
    validateFrozenScoreCorpus('primary', frozen, cases, finalized),
  );
  assert.throws(
    () => validateFreezeSchedule(frozen, cases, schedule.slice(1)),
    /complete paired design/u,
  );
  assert.throws(
    () =>
      validateFrozenScoreCorpus('primary', frozen, cases, finalized.slice(1)),
    /complete frozen paired study/u,
  );
});
