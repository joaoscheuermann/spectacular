import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import { createBundleSelectionSchema } from '../src/lib/schemas/bundle.js';
import { bundle } from '../src/lib/states/bundle/index.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type { MosaicOptions } from '../src/lib/types/mosaic-options.js';
import type { WorkflowState } from '../src/lib/types/workflow.js';

test('routes with transitive ancestor artifacts and omits unrelated branches', async () => {
  const root = node('root', 'completed', 'root artifact');
  const direct = node('direct', 'completed', 'direct artifact', ['root']);
  const unrelated = node('unrelated', 'completed', 'unrelated artifact');
  const current = node('current', 'ready', undefined, ['direct']);
  const harness = createHarness({ matches: [] });

  const action = await run(
    { revision: 1, nodes: [root, unrelated, direct, current] },
    harness.options,
  );

  assert.equal(action.type, 'transition');
  assert.match(harness.searches[0] ?? '', /root artifact/u);
  assert.match(harness.searches[0] ?? '', /direct artifact/u);
  assert.doesNotMatch(harness.searches[0] ?? '', /unrelated artifact/u);
});

test('uses an empty skill bundle and base-only tools without model calls', async () => {
  const current = node('current');
  const base = tool('base');
  const harness = createHarness({ matches: [], requiredTools: [base] });

  const action = await run({ revision: 1, nodes: [current] }, harness.options);

  assert.equal(action.type, 'transition');
  assert.deepEqual(current.candidates, []);
  assert.deepEqual(current.bundle, {
    goalId: 'current',
    skills: [],
    selectionRationale: 'No routable skill candidates were available.',
  });
  assert.deepEqual(current.tools, [{ name: 'base', description: 'base tool' }]);
  assert.equal(harness.reranks.length, 0);
  assert.equal(harness.completions.length, 0);
});

test('skips reranking and selection when maxSkills is zero', async () => {
  const current = node('current');
  const candidate = skill('candidate');
  const harness = createHarness({ matches: [candidate], maxSkills: 0 });

  const action = await run({ revision: 1, nodes: [current] }, harness.options);

  assert.equal(action.type, 'transition');
  assert.deepEqual(current.candidates, []);
  assert.deepEqual(current.bundle, {
    goalId: 'current',
    skills: [],
    selectionRationale: 'Skill routing is disabled because maxSkills is zero.',
  });
  assert.equal(harness.searches.length, 0);
  assert.equal(harness.reranks.length, 0);
  assert.equal(harness.completions.length, 0);
});

test('normalizes selected references to score and canonical-name order', async () => {
  const current = node('current');
  const beta = skill('beta', ['shared', 'beta-tool']);
  const alpha = skill('alpha', ['alpha-tool']);
  const gamma = skill('gamma', ['shared', 'gamma-tool']);
  const tools = [
    tool('base'),
    tool('shared'),
    tool('alpha-tool'),
    tool('beta-tool'),
    tool('gamma-tool'),
  ];
  const harness = createHarness({
    matches: [beta, alpha, gamma],
    ranking: [
      { index: 0, relevanceScore: 1 },
      { index: 2, relevanceScore: 2 },
      { index: 1, relevanceScore: 1 },
    ],
    evaluations: [
      {
        skillName: 'beta',
        selected: true,
        rationale: 'Beta behavior is needed.',
      },
      {
        skillName: 'alpha',
        selected: false,
        rationale: 'Alpha behavior is redundant.',
      },
      {
        skillName: 'gamma',
        selected: true,
        rationale: 'Gamma behavior is needed.',
      },
    ],
    selectionRationale: 'Gamma and beta form the smallest sufficient bundle.',
    requiredTools: [tools[0]!],
    toolMenu: tools,
  });

  const action = await run({ revision: 1, nodes: [current] }, harness.options);

  assert.equal(action.type, 'transition');
  assert.deepEqual(current.candidates, [
    {
      skillName: 'gamma',
      score: 2,
      rank: 1,
      rationale: 'Gamma behavior is needed.',
    },
    {
      skillName: 'alpha',
      score: 1,
      rank: 2,
      rationale: 'Alpha behavior is redundant.',
    },
    {
      skillName: 'beta',
      score: 1,
      rank: 3,
      rationale: 'Beta behavior is needed.',
    },
  ]);
  assert.deepEqual(current.bundle, {
    goalId: 'current',
    skills: ['gamma', 'beta'],
    selectionRationale: 'Gamma and beta form the smallest sufficient bundle.',
  });
  assert.deepEqual(
    current.tools.map(({ name }) => name),
    ['base', 'shared', 'gamma-tool', 'beta-tool'],
  );
  assert.match(harness.completions[0]?.system ?? '', /at most 5 skills/u);
});

test('materializes rejected candidates and an empty selected bundle', async () => {
  const current = node('current');
  const candidate = skill('candidate');
  const harness = createHarness({
    matches: [candidate],
    ranking: [{ index: 0, relevanceScore: 0.123456789 }],
    evaluations: [
      {
        skillName: 'candidate',
        selected: false,
        rationale: 'The candidate does not add required behavior.',
      },
    ],
    selectionRationale: 'General capability is sufficient for this goal.',
  });

  const action = await run({ revision: 1, nodes: [current] }, harness.options);

  assert.equal(action.type, 'transition');
  assert.deepEqual(current.candidates, [
    {
      skillName: 'candidate',
      score: 0.123456789,
      rank: 1,
      rationale: 'The candidate does not add required behavior.',
    },
  ]);
  assert.deepEqual(current.bundle, {
    goalId: 'current',
    skills: [],
    selectionRationale: 'General capability is sufficient for this goal.',
  });
});

test('fails on incomplete duplicate and out-of-range reranker results', async () => {
  const candidateA = skill('a');
  const candidateB = skill('b');
  const rankings = [
    [{ index: 0, relevanceScore: 1 }],
    [
      { index: 0, relevanceScore: 1 },
      { index: 0, relevanceScore: 0 },
    ],
    [
      { index: 0, relevanceScore: 1 },
      { index: 2, relevanceScore: 0 },
    ],
    [
      { index: 0, relevanceScore: Number.NaN },
      { index: 1, relevanceScore: 0 },
    ],
  ];

  for (const ranking of rankings) {
    const harness = createHarness({
      matches: [candidateA, candidateB],
      ranking,
    });
    const action = await run(
      { revision: 1, nodes: [node('current')] },
      harness.options,
    );
    assert.equal(action.type, 'fail');
  }
});

test('binds the selection schema to the node candidates uniqueness and limit', () => {
  const schema = createBundleSelectionSchema('node-1', ['alpha', 'beta'], 1);
  const valid = {
    goalId: 'node-1',
    evaluations: [
      { skillName: 'alpha', selected: true, rationale: 'Needed.' },
      { skillName: 'beta', selected: false, rationale: 'Not needed.' },
    ],
    selectionRationale: 'Alpha is sufficient.',
  };

  assert.equal(schema.safeParse(valid).success, true);
  assert.equal(schema.safeParse({ ...valid, goalId: 'other' }).success, false);
  assert.equal(
    schema.safeParse({
      goalId: 'node-1',
      evaluations: [
        { skillName: 'unknown', selected: true, rationale: 'Needed.' },
        { skillName: 'beta', selected: false, rationale: 'Not needed.' },
      ],
      selectionRationale: 'Unknown is sufficient.',
    }).success,
    false,
  );
  assert.equal(
    schema.safeParse({
      goalId: 'node-1',
      evaluations: [valid.evaluations[0], valid.evaluations[0]],
      selectionRationale: 'Alpha is sufficient.',
    }).success,
    false,
  );
  assert.equal(
    schema.safeParse({
      goalId: 'node-1',
      evaluations: [valid.evaluations[0]],
      selectionRationale: 'Alpha is sufficient.',
    }).success,
    false,
  );
  assert.equal(
    schema.safeParse({
      ...valid,
      evaluations: valid.evaluations.map((evaluation) => ({
        ...evaluation,
        selected: true,
      })),
    }).success,
    false,
  );
  assert.equal(
    schema.safeParse({ ...valid, selectionRationale: ' ' }).success,
    false,
  );
  assert.equal(
    schema.safeParse({
      ...valid,
      evaluations: [
        { ...valid.evaluations[0], rationale: ' ' },
        valid.evaluations[1],
      ],
    }).success,
    false,
  );
});

test('excludes required and stale indexed skills and never reads the tool retriever', async () => {
  const current = node('current');
  const required = skill('required');
  const selected = skill('selected');
  const stale = skill('stale');
  const harness = createHarness({
    matches: [required, stale, selected],
    requiredSkills: [required],
    skillMenu: [required, selected],
    evaluations: [
      { skillName: 'selected', selected: true, rationale: 'Needed.' },
    ],
  });

  const action = await run({ revision: 1, nodes: [current] }, harness.options);

  assert.equal(action.type, 'transition');
  assert.deepEqual(current.bundle?.skills, ['selected']);
  assert.equal(harness.reranks[0]?.documents.length, 1);
  assert.match(harness.reranks[0]?.documents[0] ?? '', /selected body/u);
});

test('uses and defensively enforces the independent execution retrieval limit', async () => {
  const current = node('current');
  const matches = [skill('first'), skill('second'), skill('third')];
  const harness = createHarness({
    matches,
    maxRetrievedCandidates: 2,
    maxSkills: 2,
    evaluations: matches.slice(0, 2).map(({ name }) => ({
      skillName: name,
      selected: false,
      rationale: `${name} is unnecessary.`,
    })),
  });

  const action = await run({ revision: 1, nodes: [current] }, harness.options);

  assert.equal(action.type, 'transition');
  assert.deepEqual(harness.topKs, [2]);
  assert.equal(harness.reranks[0]?.documents.length, 2);
  assert.deepEqual(
    current.candidates.map(({ skillName }) => skillName),
    ['first', 'second'],
  );
});

test('keeps hostile context delimited and excludes private content from logs', async () => {
  const hostile = 'artifact\n``````\n~~~~~~\nprivate-value';
  const ancestor = node('ancestor', 'completed', hostile);
  const current = node('current', 'ready', undefined, ['ancestor']);
  const selected = skill('selected');
  const harness = createHarness({
    matches: [selected],
    evaluations: [
      {
        skillName: 'selected',
        selected: true,
        rationale: 'private-rationale',
      },
    ],
  });

  const action = await run(
    { revision: 1, nodes: [ancestor, current] },
    harness.options,
  );

  assert.equal(action.type, 'transition');
  assert.match(harness.completions[0]?.user ?? '', /`{7}text\nartifact/u);
  const logs = JSON.stringify(harness.logs);
  assert.doesNotMatch(logs, /private-value|private-rationale|selected body/u);
});

type HarnessInput = {
  readonly matches?: readonly Skill[];
  readonly ranking?: readonly {
    readonly index: number;
    readonly relevanceScore: number;
  }[];
  readonly evaluations?: readonly {
    readonly skillName: string;
    readonly selected: boolean;
    readonly rationale: string;
  }[];
  readonly selectionRationale?: string;
  readonly requiredSkills?: readonly Skill[];
  readonly skillMenu?: readonly Skill[];
  readonly requiredTools?: readonly Tool[];
  readonly toolMenu?: readonly Tool[];
  readonly maxSkills?: number;
  readonly maxRetrievedCandidates?: number;
};

const createHarness = (input: HarnessInput) => {
  const matches = input.matches ?? [];
  const skillMenu = input.skillMenu ?? matches;
  const requiredTools = input.requiredTools ?? [];
  const toolMenu = input.toolMenu ?? requiredTools;
  const searches: string[] = [];
  const topKs: number[] = [];
  const reranks: Array<{
    readonly query: string;
    readonly documents: readonly string[];
  }> = [];
  const completions: Array<{ readonly system: string; readonly user: string }> =
    [];
  const logs: unknown[] = [];
  const toolRetriever = new Proxy(
    {},
    {
      get() {
        throw new Error('tools.retriever must not be read by bundle');
      },
    },
  );
  const options: MosaicOptions = {
    logger: {
      info: (bindings: unknown, message: string) =>
        logs.push({ bindings, message }),
      debug: (bindings: unknown, message: string) =>
        logs.push({ bindings, message }),
    } as never,
    provider: {
      rerank: async (request: {
        readonly query: string;
        readonly documents: readonly string[];
      }) => {
        reranks.push(request);
        return (
          input.ranking ??
          request.documents.map((_, index) => ({
            index,
            relevanceScore: request.documents.length - index,
          }))
        );
      },
      complete: async (request: {
        readonly messages: readonly { readonly content: string }[];
      }) => {
        completions.push({
          system: request.messages[0]?.content ?? '',
          user: request.messages[1]?.content ?? '',
        });
        return {
          structured: {
            goalId: 'current',
            evaluations: input.evaluations ?? [],
            selectionRationale:
              input.selectionRationale ?? 'The selected bundle is sufficient.',
          },
        };
      },
    } as never,
    models: {
      default: 'default-model',
      reranker: 'reranker-model',
    },
    routing: {
      maxHintCandidates: 5,
      maxRetrievedCandidates: input.maxRetrievedCandidates ?? 5,
      maxSkills: input.maxSkills ?? 5,
    },
    execution: { maxTurns: 8 },
    revision: { max: 3 },
    skills: {
      required: input.requiredSkills ?? [],
      menu: skillMenu,
      retriever: {
        search: async (query: string, topK: number) => {
          searches.push(query);
          topKs.push(topK);
          return matches.map((data) => ({ data, score: 1 }));
        },
      },
    },
    tools: {
      required: requiredTools,
      menu: toolMenu,
      retriever: toolRetriever as never,
    },
  };

  return { options, searches, topKs, reranks, completions, logs };
};

const run = (graph: Graph, options: MosaicOptions) =>
  bundle(state([graph]), { input: 'Original request.', options }, handlers());

const state = (graphs: Graph[]): WorkflowState => ({
  graphs,
});

const handlers = () =>
  ({
    transition: (handler: string, state: WorkflowState) => ({
      type: 'transition' as const,
      handler,
      state,
    }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  }) as never;

const node = (
  id: string,
  status: Node['status'] = 'ready',
  artifact?: string,
  dependsOn: readonly string[] = [],
): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn: [...dependsOn],
  status,
  deliver: true,
  index: 0,
  candidates: [],
  bundle: null,
  tools: [],
  artifacts:
    artifact === undefined
      ? []
      : [{ kind: 'inline', mime: 'text/plain', data: artifact }],
  outcome: null,
  termination: null,
});

const skill = (name: string, allowedTools: readonly string[] = []): Skill => ({
  name,
  description: `${name} description`,
  body: `${name} body`,
  allowedTools: [...allowedTools],
  indexText: `${name} | ${name} description | ${allowedTools.join(',')} | ${name} body`,
});

const tool = (name: string): Tool => ({
  name,
  description: `${name} tool`,
  input: {} as Tool['input'],
  output: {} as Tool['output'],
  definition: {
    name,
    description: `${name} tool`,
    inputSchema: {},
    outputSchema: {},
  },
  execute: async () => undefined,
});
