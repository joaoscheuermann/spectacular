import assert from 'node:assert/strict';
import test from 'node:test';

import type { LlmProvider } from 'llms';
import type { MosaicAgent, MosaicOptions, MosaicResult } from 'mosaic';
import type { MosaicEvaluationOptions } from 'mosaic/evaluation';

import {
  createCompositionWorkflow,
  defaultCompositionRerankerModel,
  runCompositionCase,
  type CompositionCase,
  type CompositionSkill,
} from '../src/composition/runner.js';

const provider = {
  metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://fake.invalid' },
} as LlmProvider;

const skills: readonly CompositionSkill[] = [
  {
    id: 'champ_001',
    name: 'Count constrained sequences',
    description: 'Build recurrences for constrained sequences.',
    body: 'Define states from the final symbol and derive a recurrence.',
  },
  {
    id: 'champ_002',
    name: 'Apply inclusion-exclusion',
    description: 'Count overlapping forbidden properties.',
    body: 'Enumerate intersections before alternating their contributions.',
  },
  {
    id: 'distractor_001',
    name: 'Summarize prose',
    description: 'Summarize long prose passages.',
    body: 'Identify the thesis and compress supporting details.',
  },
];

const benchmarkCase: CompositionCase = {
  id: 'champ_case',
  dataset: 'champ',
  request: 'Count strings under two simultaneous adjacency constraints.',
  goldSkillIds: ['champ_001', 'champ_002'],
};

const ranking = [
  { skillId: 'champ_001', score: 3 },
  { skillId: 'distractor_001', score: 2 },
  { skillId: 'champ_002', score: 1 },
] as const;

test('no-skills keeps P0 unchanged and exposes no skill catalog', async () => {
  const captured = captureWorkflow(completed([]));
  const workflow = createCompositionWorkflow(
    {
      arm: 'no-skills',
      benchmarkCase,
      skills,
      ranking,
      profile: { provider, model: 'model' },
    },
    captured.create,
  );

  assert.deepEqual(captured.options?.skills.required, []);
  assert.deepEqual(captured.options?.skills.menu, []);
  assert.equal(captured.options?.routing.maxSkills, 0);
  assert.equal(
    await captured.evaluation?.hooks?.feedbackPlan?.(
      { request: 'request', graph: {} as never },
      async () => {
        throw new Error('next must not be called');
      },
    ),
    'unchanged',
  );
  assert.ok(workflow);
});

test('fixed-top-k injects the frozen retrieval prefix without selection', () => {
  const captured = captureWorkflow(completed([]));
  createCompositionWorkflow(
    {
      arm: 'fixed-top-k',
      benchmarkCase,
      skills,
      ranking,
      topK: 2,
      profile: { provider, model: 'model' },
    },
    captured.create,
  );

  assert.deepEqual(
    captured.options?.skills.required.map(({ description }) => description),
    [skills[0]?.description, skills[2]?.description],
  );
  assert.equal(captured.options?.routing.maxSkills, 0);
});

test('oracle injects every annotated skill in gold order', () => {
  const captured = captureWorkflow(completed([]));
  createCompositionWorkflow(
    {
      arm: 'oracle',
      benchmarkCase,
      skills,
      ranking,
      profile: { provider, model: 'model' },
    },
    captured.create,
  );

  assert.deepEqual(
    captured.options?.skills.required.map(({ description }) => description),
    [skills[0]?.description, skills[1]?.description],
  );
});

test('mosaic receives the frozen shortlist and clips retrieval per hook limit', async () => {
  const captured = captureWorkflow(completed([]));
  createCompositionWorkflow(
    {
      arm: 'mosaic',
      benchmarkCase,
      skills,
      ranking,
      maxRetrievedCandidates: 3,
      maxHintCandidates: 2,
      maxSkills: 2,
      profile: { provider, model: 'model' },
    },
    captured.create,
  );

  const matches = await captured.evaluation?.hooks?.retrieval?.(
    {
      request: 'request',
      graph: {} as never,
      node: {} as never,
      query: 'query',
      limit: 2,
      catalog: captured.options?.skills.menu ?? [],
    },
    async () => [],
  );

  assert.equal(captured.options?.routing.maxSkills, 2);
  assert.equal(
    captured.options?.models.reranker,
    defaultCompositionRerankerModel,
  );
  assert.equal(captured.options?.skills.menu.length, 3);
  assert.deepEqual(
    matches?.map(({ score }) => score),
    [3, 2],
  );
});

test('run result preserves evaluator output and maps opaque bundle names to ids', async () => {
  const selectedNames: string[] = [];
  const captured = captureWorkflow(async (_input, _options) => {
    const names = captured.options?.skills.menu.map(({ name }) => name) ?? [];
    selectedNames.push(names[0]!, names[2]!);
    return completed(selectedNames)('request');
  });

  const result = await runCompositionCase(
    {
      arm: 'mosaic',
      benchmarkCase,
      skills,
      ranking,
      profile: { provider, model: 'model' },
    },
    { createWorkflow: captured.create },
  );

  assert.equal(result.rawOutput, 'ANSWER: 42');
  assert.deepEqual(result.skillIdsUsed, ['champ_001', 'champ_002']);
  assert.equal(result.status, 'completed');
});

type Prompt = MosaicAgent['prompt'];

const completed =
  (selected: readonly string[]): Prompt =>
  async (): Promise<MosaicResult> => ({
    status: 'completed',
    delivery: { markdown: 'ANSWER: 42', parts: [] },
    nodes: [
      {
        id: 'n01:answer',
        goal: 'Answer.',
        doneWhen: ['Answer is present.'],
        status: 'completed',
        candidates: [],
        bundle: {
          goalId: 'n01:answer',
          skills: selected,
          selectionRationale: 'Selected for the case.',
        },
        observations: [],
        outcome: null,
        termination: null,
      },
    ],
  });

const captureWorkflow = (prompt: Prompt) => {
  const captured: {
    options?: MosaicOptions;
    evaluation?: MosaicEvaluationOptions;
    create: (
      options: MosaicOptions,
      evaluation?: MosaicEvaluationOptions,
    ) => MosaicAgent;
  } = {
    create: (options, evaluation) => {
      captured.options = options;
      captured.evaluation = evaluation;
      return { prompt };
    },
  };
  return captured;
};
