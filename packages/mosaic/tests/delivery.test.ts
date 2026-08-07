import assert from 'node:assert/strict';
import test from 'node:test';

import { delivery } from '../src/lib/states/delivery/index.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type { WorkflowState } from '../src/lib/types/workflow.js';

test('assembles one terminal deliverable with additional artifacts and observations', async () => {
  const node = completed('final', 0, [], true, '## Result', [
    { mime: 'text/plain', data: 'extra' },
  ]);
  node.observations = [observation('final')];

  const action = await delivery(
    state([{ nodes: [node] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.deepEqual(action.value, {
    markdown: '## Result',
    parts: [
      {
        id: 'final',
        goal: 'Goal final',
        markdown: '## Result',
        artifacts: [{ mime: 'text/plain', data: 'extra' }],
        observations: [observation('final')],
      },
    ],
  });
});

test('orders deliverables by stable topological order and preserves Markdown', async () => {
  const source = completed('source', 3, [], false, 'private');
  const first = completed('first', 2, ['source'], true, ' first\n');
  const second = completed('second', 1, ['source'], true, 'second');
  const action = await delivery(
    state([{ nodes: [source, first, second] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.deepEqual(
    action.value.parts.map(({ id }) => id),
    ['first', 'second'],
  );
  assert.equal(action.value.markdown, ' first\n\n\nsecond');
});

test('excludes completed nodes that are not deliverable', async () => {
  const preparation = completed('preparation', 0, [], false, 'private');
  const final = completed('final', 1, ['preparation'], true, 'public');
  const action = await delivery(
    state([{ nodes: [preparation, final] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  assert.deepEqual(
    action.value.parts.map(({ id }) => id),
    ['final'],
  );
  assert.equal(action.value.markdown, 'public');
});

test('fails delivery for invalid graph and deliverable contracts', async () => {
  const cases: readonly {
    readonly graph?: Graph;
    readonly message: RegExp;
  }[] = [
    { message: /active graph/u },
    {
      graph: { nodes: [pending('final', 0)] },
      message: /incomplete graph/u,
    },
    {
      graph: { nodes: [completed('internal', 0, [], false, 'private')] },
      message: /delivery/u,
    },
    {
      graph: { nodes: [completed('final', 0, [], true, '')] },
      message: /Markdown result/u,
    },
    {
      graph: {
        nodes: [
          { ...completed('final', 0, [], true, 'result'), artifacts: [] },
        ],
      },
      message: /Markdown result/u,
    },
    {
      graph: {
        nodes: [
          completed('final', 0, [], true, 'result', [], {
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
    { mime: 'application/json', data: '{"ok":true}' },
  ]);
  node.observations = [observation('final')];
  const action = await delivery(
    state([{ nodes: [node] }]),
    context(),
    handlers(),
  );

  assert.equal(action.type, 'finish');
  if (action.type !== 'finish' || action.value === undefined) return;
  const part = action.value.parts[0];
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
});

const completed = (
  id: string,
  index: number,
  dependsOn: readonly string[],
  deliver: boolean,
  markdown: string,
  artifacts: readonly { readonly mime: string; readonly data: string }[] = [],
  primary?: { readonly mime: string; readonly data: string },
): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn: [...dependsOn],
  deliver,
  status: 'completed',
  index,
  skills: [],
  tools: [],
  artifacts: [
    primary ?? { mime: 'text/markdown', data: markdown },
    ...artifacts,
  ],
  observations: [],
  revisionRequest: null,
});
