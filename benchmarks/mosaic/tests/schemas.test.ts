import assert from 'node:assert/strict';
import test from 'node:test';

import { z } from 'zod';

import {
  CaseV1,
  ConditionV1,
  FreezeManifestV1,
  schemasV1,
} from '../src/schemas/index.js';

const hash = `sha256:${'0'.repeat(64)}`;

test('exports every frozen V1 contract as JSON Schema', () => {
  assert.deepEqual(Object.keys(schemasV1).sort(), [
    'AnalysisResultV1',
    'AttemptReservationV1',
    'CaseV1',
    'ConditionV1',
    'ExecutionRecordV1',
    'FreezeManifestV1',
    'ReviewAssignmentV1',
    'ReviewV1',
    'RunSpecV1',
    'ScoreRowV1',
  ]);

  for (const schema of Object.values(schemasV1)) {
    assert.equal(z.toJSONSchema(schema, { io: 'input' }).type, 'object');
  }
});

test('accepts a self-contained case and rejects private extra fields', () => {
  const value = {
    schemaVersion: 1,
    id: 'pilot-A-001',
    familyId: 'pilot-family-001',
    phase: 'pilot',
    title: 'Reconcile a quarterly report',
    domain: 'documents-finance',
    compositionClass: 'A',
    focusGoalRole: 'reconcile-report',
    composition: {
      skillCount: 0,
      toolRequirement: 'none',
      toolCount: 0,
      requiresRevision: false,
      requiresExternalEffect: false,
    },
    adaptive: false,
    request: 'Reconcile the supplied quarterly report.',
    fixtureIds: ['fixture-001'],
    tags: ['reconciliation'],
    gold: {
      criteria: [
        {
          id: 'balanced',
          description: 'The totals balance.',
          evidenceRefs: ['delivery.balanced'],
        },
      ],
      requiredSkills: ['finance-reconciliation'],
      relevantSkills: ['finance-reconciliation'],
      forbiddenSkills: [],
      requiredTools: ['calculate'],
      forbiddenTools: [],
      expectedState: {
        fixtureId: 'fixture-001',
        worldHash: hash,
        toolEvidence: [
          {
            id: 'tool.calculate',
            name: 'calculate',
            input: { expression: '1 + 1' },
            evidenceHash: hash,
          },
        ],
        requiredEffects: [],
      },
      expectedDelivery: {
        document: { balanced: true },
        fields: [{ id: 'delivery.balanced', path: ['balanced'] }],
      },
      requiresRevision: false,
    },
    contentHash: hash,
  } as const;

  assert.equal(CaseV1.parse(value).id, value.id);
  assert.equal(
    CaseV1.safeParse({ ...value, credential: 'secret' }).success,
    false,
  );
});

test('condition factors expose the exact ablation surface', () => {
  const condition = ConditionV1.parse({
    schemaVersion: 1,
    id: 'M1',
    label: 'MOSAIC 0.2',
    description: 'Complete MOSAIC treatment.',
    kind: 'mosaic',
    eligibility: 'all',
    factors: {
      decomposition: 'goal',
      catalogFeedback: true,
      skillView: 'body',
      retrieval: 'top-k',
      maxCandidates: 5,
      maxSkills: 5,
      bundle: 'selective',
      bundleOrder: 'ranked',
      menu: 'base-plus-bundle',
      baseTools: true,
      localizedRevision: true,
      maxTurns: 16,
      structuredRepairRetries: 2,
    },
    declaredChange: 'Full treatment.',
  });

  assert.equal(condition.factors.catalogFeedback, true);
});

test('freeze manifest records public provenance without credential slots', () => {
  const manifest = FreezeManifestV1.parse({
    schemaVersion: 1,
    studyId: 'mosaic-confirmatory-1',
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
    modelCallBudgetP95: 12,
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
      confidence95: { lower: -0.05, upper: 0.05 },
      approved: true,
    },
    containerDigest: hash,
    manifestHash: hash,
  });

  assert.equal('token' in manifest, false);
  assert.equal(
    FreezeManifestV1.safeParse({
      ...manifest,
      replication: {
        ...manifest.replication,
        confidence95: { lower: 0.04, upper: -0.04 },
      },
    }).success,
    false,
  );
});
