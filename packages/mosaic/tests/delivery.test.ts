import assert from 'node:assert/strict';
import test from 'node:test';

import { delivery } from '../src/lib/states/delivery/index.js';
import type { Artifact } from '../src/lib/types/artifact.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type { WorkflowState } from '../src/lib/types/workflow.js';

test('assembles one terminal deliverable with additional artifacts and observations', async () => {
  const node = completed('final', 0, [], true, '## Result', [
    { kind: 'inline', mime: 'text/plain', data: 'extra' },
    {
      kind: 'reference',
      mime: 'application/octet-stream',
      reference: 'urn:artifact:1',
    },
  ]);
  node.candidates = [
    {
      skillName: 'selected',
      score: 0.75,
      rank: 1,
      rationale: 'Selected behavior is required.',
    },
  ];
  node.bundle = {
    goalId: 'final',
    skills: ['selected'],
    selectionRationale: 'The selected skill is sufficient.',
  };
  node.observations = [observation('final')];

  const action = await delivery(
    state([{ revision: 1, nodes: [node] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.equal(action.value.status, 'completed');
  if (action.value.status !== 'completed') return;
  assert.deepEqual(action.value.delivery, {
    markdown: '## Result',
    parts: [
      {
        id: 'final',
        goal: 'Goal final',
        markdown: '## Result',
        artifacts: [
          { kind: 'inline', mime: 'text/plain', data: 'extra' },
          {
            kind: 'reference',
            mime: 'application/octet-stream',
            reference: 'urn:artifact:1',
          },
        ],
        observations: [observation('final')],
      },
    ],
  });
  assert.deepEqual(action.value.nodes[0]?.candidates, node.candidates);
  assert.deepEqual(action.value.nodes[0]?.bundle, node.bundle);
  assert.notStrictEqual(action.value.nodes[0]?.candidates, node.candidates);
  assert.notStrictEqual(action.value.nodes[0]?.bundle, node.bundle);
});

test('orders deliverables by stable topological order and preserves Markdown', async () => {
  const source = completed('source', 3, [], false, 'private');
  const first = completed('first', 2, ['source'], true, ' first\n');
  const second = completed('second', 1, ['source'], true, 'second');
  const action = await delivery(
    state([{ revision: 1, nodes: [source, first, second] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.deepEqual(
    action.value.status === 'completed'
      ? action.value.delivery.parts.map(({ id }) => id)
      : [],
    ['first', 'second'],
  );
  assert.equal(
    action.value.status === 'completed' ? action.value.delivery.markdown : '',
    ' first\n\n\nsecond',
  );
});

test('excludes completed nodes that are not deliverable', async () => {
  const preparation = completed('preparation', 0, [], false, 'private');
  const final = completed('final', 1, ['preparation'], true, 'public');
  const action = await delivery(
    state([{ revision: 1, nodes: [preparation, final] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.deepEqual(
    action.value.status === 'completed'
      ? action.value.delivery.parts.map(({ id }) => id)
      : [],
    ['final'],
  );
  assert.equal(
    action.value.status === 'completed' ? action.value.delivery.markdown : '',
    'public',
  );
});

test('returns blocked without partial delivery and keeps topological node order', async () => {
  const root = completed('root', 2, [], false, 'private');
  const blocked = terminal('blocked', 0, ['root'], 'blocked');
  const action = await delivery(
    state([{ revision: 1, nodes: [root, blocked] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.deepEqual(action.value, {
    status: 'blocked',
    nodes: [
      {
        id: 'root',
        goal: 'Goal root',
        doneWhen: ['root is complete.'],
        status: 'completed',
        candidates: [],
        bundle: null,
        observations: [],
        outcome: completedOutcome('root', 'private'),
        termination: null,
      },
      {
        id: 'blocked',
        goal: 'Goal blocked',
        doneWhen: ['blocked is complete.'],
        status: 'blocked',
        candidates: [],
        bundle: null,
        observations: [],
        outcome: terminalOutcome('blocked', 'blocked'),
        termination: null,
      },
    ],
  });
  assert.equal('delivery' in action.value, false);
});

test('failed takes precedence over blocked in a terminal workflow', async () => {
  const blocked = terminal('blocked', 0, [], 'blocked');
  const failed = terminal('failed', 1, [], 'failed');
  const action = await delivery(
    state([{ revision: 1, nodes: [blocked, failed] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.equal(action.value.status, 'failed');
  assert.equal('delivery' in action.value, false);
});

test('fails delivery for invalid graph and deliverable contracts', async () => {
  const cases: readonly {
    readonly graph?: Graph;
    readonly message: RegExp;
  }[] = [
    { message: /active graph/u },
    {
      graph: { revision: 1, nodes: [pending('final', 0)] },
      message: /incomplete graph/u,
    },
    {
      graph: {
        revision: 1,
        nodes: [completed('internal', 0, [], false, 'private')],
      },
      message: /delivery/u,
    },
    {
      graph: { revision: 1, nodes: [completed('final', 0, [], true, '')] },
      message: /Markdown result/u,
    },
    {
      graph: {
        revision: 1,
        nodes: [
          { ...completed('final', 0, [], true, 'result'), artifacts: [] },
        ],
      },
      message: /Markdown result/u,
    },
    {
      graph: {
        revision: 1,
        nodes: [
          completed('final', 0, [], true, 'result', [], {
            kind: 'inline',
            mime: 'text/plain',
            data: 'wrong primary',
          }),
        ],
      },
      message: /Markdown result/u,
    },
  ];

  for (const { graph, message } of cases) {
    const action = await delivery(
      state(graph === undefined ? [] : [graph]),
      context(),
      handlers(),
    );
    assert.equal(action.type, 'fail');
    if (action.type === 'fail')
      assert.match((action.error as Error).message, message);
  }
});

test('copies delivered artifacts and observations instead of retaining graph references', async () => {
  const node = completed('final', 0, [], true, 'result', [
    { kind: 'inline', mime: 'application/json', data: '{"ok":true}' },
  ]);
  node.observations = [observation('final')];
  const action = await delivery(
    state([{ revision: 1, nodes: [node] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  if (action.value.status !== 'completed') return;
  const part = action.value.delivery.parts[0];
  assert.ok(part);
  assert.notStrictEqual(part.artifacts, node.artifacts);
  assert.notStrictEqual(part.artifacts[0], node.artifacts[1]);
  assert.notStrictEqual(part.observations, node.observations);
  assert.notStrictEqual(part.observations[0], node.observations[0]);
});

const state = (graphs: Graph[]): WorkflowState => ({ graphs });

const context = () =>
  ({ options: { logger: { info: () => undefined } } }) as never;

const handlers = () =>
  ({
    finish: (value: unknown) => ({ type: 'finish' as const, value }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  }) as never;

const observation = (goalId: string) => ({
  id: `observation-${goalId}`,
  goalId,
  toolName: 'lookup',
  callId: `call-${goalId}`,
  input: '{"query":"evidence"}',
  output: '{"found":true}',
});

const pending = (id: string, index: number): Node => ({
  ...completed(id, index, [], true, 'result'),
  status: 'pending',
  artifacts: [],
  outcome: null,
});

const completed = (
  id: string,
  index: number,
  dependsOn: readonly string[],
  deliver: boolean,
  markdown: string,
  artifacts: readonly Artifact[] = [],
  primary?: Artifact,
): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn: [...dependsOn],
  deliver,
  status: 'completed',
  index,
  candidates: [],
  bundle: null,
  tools: [],
  artifacts: [
    primary ?? { kind: 'inline', mime: 'text/markdown', data: markdown },
    ...artifacts,
  ],
  observations: [],
  outcome: completedOutcome(id, markdown || 'semantic result'),
  termination: null,
});

const terminal = (
  id: string,
  index: number,
  dependsOn: readonly string[],
  status: 'blocked' | 'failed',
): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn: [...dependsOn],
  deliver: true,
  status,
  index,
  candidates: [],
  bundle: null,
  tools: [],
  artifacts: [],
  observations: [],
  outcome: terminalOutcome(id, status),
  termination: null,
});

const terminalOutcome = (id: string, status: 'blocked' | 'failed') => ({
  status,
  criteria: [
    {
      criterionIndex: 0,
      satisfied: false,
      evidence: `${id} incomplete.`,
      observationIds: [],
    },
  ],
  result: null,
  revisionRequest: null,
  reason: `${id} ${status}.`,
});

const completedOutcome = (id: string, markdown: string) => ({
  status: 'completed' as const,
  criteria: [
    {
      criterionIndex: 0,
      satisfied: true,
      evidence: `${id} is complete.`,
      observationIds: [],
    },
  ],
  result: { markdown, artifacts: [] },
  revisionRequest: null,
  reason: null,
});
