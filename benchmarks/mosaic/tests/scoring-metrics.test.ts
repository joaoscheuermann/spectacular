import assert from 'node:assert/strict';
import test from 'node:test';

import {
  holmAdjust,
  ndcgAtK,
  recallAtK,
  scoreBundle,
  scoreExecution,
} from '../src/scoring/index.js';

const hash = `sha256:${'a'.repeat(64)}`;

const freezeManifest = {
  schemaVersion: 1 as const,
  studyId: 'study-1',
  frozenAt: '2026-08-08T12:00:00.000Z',
  gitCommit: 'a'.repeat(40),
  cleanWorktree: true as const,
  primaryModel: {
    provider: 'openai',
    model: 'openai/gpt-5.6-luna' as const,
    effort: 'medium' as const,
  },
  reranker: 'voyageai/rerank-2.5-lite' as const,
  embedder: {
    model: 'voyageai/voyage-4-large' as const,
    dimensions: 2048 as const,
  },
  selectedBaseline: 'B2' as const,
  repetitions: 5 as const,
  alpha: 0.05 as const,
  minimumEffect: 0.1 as const,
  targetPower: 0.8 as const,
  nPower: 240,
  nFinal: 240,
  modelCallBudgetP95: 7,
  semanticReviewFraction: 0.2 as const,
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
      effort: 'medium' as const,
    },
    neutralCases: 60 as const,
    repetitions: 3 as const,
    difference: 0,
    confidence95: { lower: -0.05, upper: 0.05 },
    approved: true as const,
  },
  containerDigest: hash,
  manifestHash: hash,
};

const benchmarkCase = {
  schemaVersion: 1 as const,
  id: 'case-1',
  familyId: 'family-1',
  phase: 'confirmatory' as const,
  title: 'Case one',
  domain: 'software' as const,
  compositionClass: 'E' as const,
  focusGoalRole: 'primary',
  composition: {
    skillCount: 'many' as const,
    toolRequirement: 'mixed' as const,
    toolCount: 'many' as const,
    requiresRevision: false,
    requiresExternalEffect: true,
  },
  adaptive: true,
  request: 'Perform the frozen task.',
  fixtureIds: ['fixture-1'],
  tags: [],
  gold: {
    criteria: [
      {
        id: 'artifact',
        description: 'Artifact is correct.',
        evidenceRefs: ['delivery.artifact'],
      },
    ],
    requiredSkills: ['skill-a'],
    relevantSkills: ['skill-a', 'skill-b'],
    forbiddenSkills: [],
    requiredTools: ['read', 'publish'],
    forbiddenTools: [],
    expectedState: {
      fixtureId: 'fixture-1',
      worldHash: hash,
      toolEvidence: [],
      requiredEffects: ['publish'],
    },
    expectedDelivery: {
      document: { artifact: 'ready' },
      fields: [{ id: 'delivery.artifact', path: ['artifact'] }],
    },
    requiresRevision: false,
  },
  contentHash: hash,
};

const executionRecord = {
  schemaVersion: 1 as const,
  attempt: 1,
  run: {
    schemaVersion: 1 as const,
    id: 'run-1',
    studyId: 'study-1',
    phase: 'confirmatory' as const,
    caseId: 'case-1',
    conditionId: 'M1',
    repetition: 1,
    seed: 42,
    pairedBlock: 'block-1',
    order: 0,
    model: {
      provider: 'openai',
      model: 'openai/gpt-5.6-luna',
      effort: 'medium' as const,
    },
    capture: 'structure' as const,
    freezeHash: hash,
  },
  status: 'succeeded' as const,
  startedAt: '2026-09-01T12:00:00.000Z',
  finishedAt: '2026-09-01T12:00:01.000Z',
  durationMs: 1000,
  firstModelCallStarted: true,
  trace: {
    rootHash: hash,
    derivedHash: hash,
    eventCount: 10,
    relativePath: 'traces/run-1.json',
  },
  outcome: { completed: true },
  worldHash: hash,
  usage: {
    inputTokens: 1000,
    outputTokens: 200,
    modelCalls: 4,
    toolCalls: 2,
    costUsd: 999,
  },
  infrastructureFailure: null,
};

test('computes Recall@K from every positively relevant skill', () => {
  assert.equal(
    recallAtK(
      ['distractor', 'acceptable-a', 'acceptable-b'],
      { 'acceptable-a': 2, 'acceptable-b': 1 },
      2,
    ),
    0.5,
  );
});

test('computes nDCG@K with graded relevance and ideal normalization', () => {
  const score = ndcgAtK(
    ['relevant', 'best', 'distractor'],
    { best: 3, relevant: 1 },
    3,
  );

  assert.ok(Math.abs(score - 0.7098097413968655) < 1e-12);
});

test('does not award repeated retrieval hits more than once', () => {
  assert.equal(
    ndcgAtK(['best', 'best'], { best: 3, relevant: 1 }, 2),
    7 / (7 + 1 / Math.log2(3)),
  );
});

test('scores deterministic end-to-end success independently from cost', () => {
  const scored = scoreExecution({
    record: executionRecord,
    case: benchmarkCase,
    evidence: {
      assertions: [
        { id: 'artifact', passed: true },
        { id: 'side-effect', passed: true },
      ],
      retrievals: [
        {
          goalId: 'goal-1',
          ranked: ['skill-a', 'skill-c'],
          relevance: { 'skill-a': 3, 'skill-b': 1 },
          k: 2,
        },
      ],
      selectedSkills: ['skill-a', 'skill-c'],
      actualMenu: ['read', 'write', 'publish'],
      expectedMenu: ['read', 'write', 'publish'],
      observations: {
        actual: [
          { tool: 'read', output: { ok: true } },
          { tool: 'publish', output: { id: 'artifact-1' } },
        ],
        expected: [
          { tool: 'read', output: { ok: true } },
          { tool: 'publish', output: { id: 'artifact-1' } },
        ],
      },
    },
    eligibility: { family: 'primary', freeze: freezeManifest },
  });

  assert.equal(scored.success, 1);
  assert.equal(scored.costUsd, 999);
  assert.equal(scored.retrieval?.recallAtK, 0.5);
  assert.equal(scored.bundleExact, false);
  assert.equal(scored.menuExact, true);
  assert.equal(scored.observationExact, true);
  assert.equal(scored.attempt, 1);
  assert.equal(scored.pairedBlock, 'block-1');
  assert.equal(scored.provider, 'openai');
  assert.equal(scored.model, 'openai/gpt-5.6-luna');
  assert.equal(scored.freezeHash, hash);
  assert.equal(scored.traceRootHash, hash);
  assert.equal(scored.worldHash, hash);
  assert.match(scored.evidenceHash, /^sha256:[a-f0-9]{64}$/);
  assert.equal(scored.modelCallBudget, null);
  assert.equal(scored.primaryEligible, true);

  assert.deepEqual(
    scoreBundle({
      selected: ['skill-a', 'skill-c'],
      requiredBehaviors: ['verify', 'publish'],
      behaviorsBySkill: {
        'skill-a': ['verify'],
        'skill-c': ['verify'],
      },
    }),
    { coverage: 0.5, redundancy: 0.5 },
  );
});

test('does not count an empty assertion set as end-to-end success', () => {
  const scored = scoreExecution({
    record: executionRecord,
    case: benchmarkCase,
    evidence: {
      assertions: [],
    },
    eligibility: { family: 'primary', freeze: freezeManifest },
  });

  assert.equal(scored.success, 0);
});

test('requires exact stable order for bundle and menu metrics', () => {
  const scored = scoreExecution({
    record: executionRecord,
    case: {
      ...benchmarkCase,
      gold: {
        ...benchmarkCase.gold,
        requiredSkills: ['skill-a', 'skill-b'],
      },
    },
    evidence: {
      assertions: [{ id: 'artifact', passed: true }],
      selectedSkills: ['skill-b', 'skill-a'],
      actualMenu: ['publish', 'read'],
      expectedMenu: ['read', 'publish'],
    },
    eligibility: { family: 'primary', freeze: freezeManifest },
  });

  assert.equal(scored.bundleExact, false);
  assert.equal(scored.menuExact, false);
});

test('derives primary eligibility from frozen phase, condition, model, and hash', () => {
  const wrongModel = scoreExecution({
    record: {
      ...executionRecord,
      run: {
        ...executionRecord.run,
        model: { ...executionRecord.run.model, model: 'openai/gpt-5.6-sol' },
      },
    },
    case: benchmarkCase,
    evidence: { assertions: [{ id: 'artifact', passed: true }] },
    eligibility: { family: 'primary', freeze: freezeManifest },
  });

  assert.equal(wrongModel.primaryEligible, false);

  const replication = scoreExecution({
    record: {
      ...executionRecord,
      run: {
        ...executionRecord.run,
        phase: 'replication',
        model: freezeManifest.replication.candidate,
      },
    },
    case: benchmarkCase,
    evidence: { assertions: [{ id: 'artifact', passed: true }] },
    eligibility: { family: 'replication', freeze: freezeManifest },
  });

  assert.equal(replication.primaryEligible, true);
});

test('rejects mismatched case provenance before scoring', () => {
  assert.throws(
    () =>
      scoreExecution({
        record: executionRecord,
        case: { ...benchmarkCase, id: 'case-2' },
        evidence: { assertions: [{ id: 'artifact', passed: true }] },
        eligibility: { family: 'primary', freeze: freezeManifest },
      }),
    /identifiers differ/,
  );
});

test('binds the evidence hash to the case, record, and derived trace', () => {
  const input = {
    record: executionRecord,
    case: benchmarkCase,
    evidence: { assertions: [{ id: 'artifact', passed: true }] },
    eligibility: { family: 'primary' as const, freeze: freezeManifest },
  };
  const original = scoreExecution(input);
  const changedTrace = scoreExecution({
    ...input,
    record: {
      ...executionRecord,
      trace: {
        ...executionRecord.trace,
        derivedHash: `sha256:${'b'.repeat(64)}`,
      },
    },
  });

  assert.notEqual(original.evidenceHash, changedTrace.evidenceHash);
});

test('marks capped sensitivity rows eligible only under their frozen policy', () => {
  const scored = scoreExecution({
    record: {
      ...executionRecord,
      run: { ...executionRecord.run, modelCallBudget: 7 },
    },
    case: benchmarkCase,
    evidence: { assertions: [{ id: 'artifact', passed: true }] },
    eligibility: {
      family: 'sensitivity',
      freeze: freezeManifest,
    },
  });

  assert.equal(scored.modelCallBudget, 7);
  assert.equal(scored.primaryEligible, true);
});

test('rejects a sensitivity record with a non-frozen cap', () => {
  assert.throws(
    () =>
      scoreExecution({
        record: {
          ...executionRecord,
          run: { ...executionRecord.run, modelCallBudget: 8 },
        },
        case: benchmarkCase,
        evidence: { assertions: [{ id: 'artifact', passed: true }] },
        eligibility: { family: 'sensitivity', freeze: freezeManifest },
      }),
    /frozen model-call budget/,
  );
});

test('applies Holm adjustment monotonically in hypothesis order', () => {
  assert.deepEqual(
    holmAdjust([
      { id: 'h1', p: 0.01 },
      { id: 'h2', p: 0.04 },
      { id: 'h3', p: 0.03 },
      { id: 'h4', p: 0.2 },
    ]),
    [
      { id: 'h1', p: 0.01, adjusted: 0.04 },
      { id: 'h2', p: 0.04, adjusted: 0.09 },
      { id: 'h3', p: 0.03, adjusted: 0.09 },
      { id: 'h4', p: 0.2, adjusted: 0.2 },
    ],
  );
});
