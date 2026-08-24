import assert from 'node:assert/strict';
import test from 'node:test';

import type { MosaicOptions } from 'mosaic';
import pino from 'pino';

import {
  createPlanningConditionHooks,
  planningCase,
  planningGoldObservation,
  planningGraphFromObservation,
  runPlanningConditions,
  type PlanningCondition,
  type PlanningRunAdapter,
} from '../src/composition/planning.js';
import { fakeProvider } from './support/provider.js';

test('runs all four conditions over one shared P0 without provider calls', async () => {
  const benchmarkCase = planningCase('planning.software.c');
  const p0Observation = planningGoldObservation(benchmarkCase, 'p0');
  const p1Observation = planningGoldObservation(benchmarkCase, 'p1');
  const p0 = planningGraphFromObservation(benchmarkCase, p0Observation);
  const p1 = planningGraphFromObservation(benchmarkCase, p1Observation);
  const profile = fakeProvider();
  let initialPlans = 0;
  let retrievals = 0;
  const revisions: PlanningCondition[] = [];
  let p0Observations = 0;

  const adapter: PlanningRunAdapter = {
    initialPlan: () => {
      initialPlans += 1;
      return p0;
    },
    retrieve: () => {
      retrievals += 1;
      return benchmarkCase.gold.relevantSkillIds;
    },
    revise: ({ condition }) => {
      revisions.push(condition);
      return condition === 'distractor' ? p0 : p1;
    },
    observe: ({ phase, condition }) => {
      if (phase === 'p0') {
        p0Observations += 1;
        assert.equal(condition, null);
        return p0Observation;
      }
      return condition === 'gold' || condition === 'retrieved'
        ? p1Observation
        : p0Observation;
    },
  };
  const results = await runPlanningConditions({
    case: benchmarkCase,
    mosaic: options(profile.provider),
    adapter,
  });

  assert.deepEqual(
    results.map(({ condition }) => condition),
    ['no-hints', 'gold', 'retrieved', 'distractor'],
  );
  assert.equal(initialPlans, 1);
  assert.equal(retrievals, 1);
  assert.equal(p0Observations, 1);
  assert.deepEqual(revisions, ['gold', 'retrieved', 'distractor']);
  assert.ok(results.every(({ p0: graph }) => graph === results[0]!.p0));
  assert.equal(profile.requests.length, 0);
  assert.deepEqual(results[0]!.evidenceSkillIds, []);
  assert.deepEqual(
    results[1]!.evidenceSkillIds,
    benchmarkCase.gold.relevantSkillIds,
  );
  assert.deepEqual(
    results[2]!.evidenceSkillIds,
    benchmarkCase.gold.relevantSkillIds,
  );
  assert.ok(
    results[3]!.evidenceSkillIds.every((id) =>
      benchmarkCase.gold.distractorSkillIds.includes(id),
    ),
  );
  assert.deepEqual(
    results.map(({ score }) => score.passed),
    [false, true, true, false],
  );
});

test('condition hooks reject a request that does not belong to their case', async () => {
  const benchmarkCase = planningCase('planning.artifacts.a');
  const p0 = planningGraphFromObservation(
    benchmarkCase,
    planningGoldObservation(benchmarkCase, 'p0'),
  );
  const hooks = createPlanningConditionHooks({
    case: benchmarkCase,
    condition: 'no-hints',
    p0,
    evidence: [],
    revise: async () => p0,
  });

  await assert.rejects(
    hooks.initialPlan!({ request: 'A different request.' }, async () => p0),
    /does not match/u,
  );
});

test('rejects unknown retrieved skill evidence before a workflow starts', async () => {
  const benchmarkCase = planningCase('planning.documents-finance.c');
  const p0Observation = planningGoldObservation(benchmarkCase, 'p0');
  const p0 = planningGraphFromObservation(benchmarkCase, p0Observation);
  const profile = fakeProvider();

  await assert.rejects(
    runPlanningConditions({
      case: benchmarkCase,
      mosaic: options(profile.provider),
      conditions: ['retrieved'],
      adapter: {
        initialPlan: async () => p0,
        retrieve: async () => ['skill.unknown'],
        revise: async () => p0,
        observe: async () => p0Observation,
      },
    }),
    /unknown skill ID/u,
  );
  assert.equal(profile.requests.length, 0);
});

test('rejects semantic observations that omit or rename plan nodes', async () => {
  const benchmarkCase = planningCase('planning.artifacts.a');
  const p0Observation = planningGoldObservation(benchmarkCase, 'p0');
  const p0 = planningGraphFromObservation(benchmarkCase, p0Observation);
  const profile = fakeProvider();

  await assert.rejects(
    runPlanningConditions({
      case: benchmarkCase,
      mosaic: options(profile.provider),
      conditions: ['no-hints'],
      adapter: {
        initialPlan: async () => p0,
        retrieve: async () => [],
        revise: async (): Promise<'unchanged'> => 'unchanged',
        observe: async () => ({
          nodes: [{ ...p0Observation.nodes[0]!, id: 'renamed-node' }],
        }),
      },
    }),
    /does not preserve the p0 graph identity/,
  );
});

const options = (
  provider: MosaicOptions['providers']['planning'],
): MosaicOptions => ({
  logger: pino({ enabled: false }),
  providers: {
    planning: provider,
    revision: provider,
    execution: provider,
    reranker: provider,
  },
  models: {
    planning: { model: 'offline', effort: 'none' },
    revision: { model: 'offline', effort: 'none' },
    execution: { model: 'offline', effort: 'none' },
    reranker: 'offline',
    embedder: 'offline',
  },
  routing: {
    maxHintCandidates: 1,
    maxRetrievedCandidates: 1,
    maxSkills: 1,
  },
  execution: { maxTurns: 1 },
  revision: { max: 0 },
  skills: {
    required: [],
    menu: [],
    retriever: { search: async () => [] },
  },
  tools: {
    required: [],
    menu: [],
    retriever: { search: async () => [] },
  },
});
