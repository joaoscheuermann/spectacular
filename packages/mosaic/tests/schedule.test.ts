import assert from 'node:assert/strict';
import test from 'node:test';

import { schedule } from '../src/lib/states/schedule/index.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type { WorkflowState } from '../src/lib/types/workflow.js';

test('fails when the workflow has no active graph', async () => {
  const action = await schedule({ graphs: [] }, {} as never, handlers());

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    (action.error as Error).message,
    'Impossible to continue, missing active graph!',
  );
});

test('finishes when every node in the active graph is completed', async () => {
  const graph = createGraph([
    createNode('first', 0, [], 'completed'),
    createNode('second', 1, [], 'completed'),
  ]);

  const action = await schedule({ graphs: [graph] }, {} as never, handlers());

  assert.deepEqual(action, { type: 'finish', value: undefined });
});

test('schedules only the last graph in the workflow', async () => {
  const older = createGraph([createNode('older', 0)]);
  const active = createGraph([createNode('active', 0)]);
  const graphs = [older, active];

  const action = await schedule({ graphs }, {} as never, handlers());

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'bundle',
    state: { graphs },
  });
  assert.equal(older.nodes[0]?.status, 'pending');
  assert.equal(active.nodes[0]?.status, 'ready');
});

test('marks a dependent node ready when all its dependencies are completed', async () => {
  const completed = createNode('completed', 0, [], 'completed');
  const dependent = createNode('dependent', 1, ['completed']);
  const graph = createGraph([completed, dependent]);

  const action = await schedule({ graphs: [graph] }, {} as never, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(completed.status, 'completed');
  assert.equal(dependent.status, 'ready');
});

test('marks at most five eligible nodes ready in descending index order', async () => {
  const nodes = Array.from({ length: 6 }, (_, index) =>
    createNode(`node-${index}`, index),
  );
  const graph = createGraph(nodes);

  const action = await schedule({ graphs: [graph] }, {} as never, handlers());

  assert.equal(action.type, 'transition');
  assert.deepEqual(
    nodes.map(({ status }) => status),
    ['pending', 'ready', 'ready', 'ready', 'ready', 'ready'],
  );
});

test('fails when pending nodes cannot become ready', async () => {
  const prerequisite = createNode('prerequisite', 0, [], 'ready');
  const dependent = createNode('dependent', 1, ['prerequisite']);
  const graph = createGraph([prerequisite, dependent]);

  const action = await schedule({ graphs: [graph] }, {} as never, handlers());

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    (action.error as Error).message,
    'Impossible to continue, missing ready nodes!',
  );
  assert.equal(dependent.status, 'pending');
});

const handlers = () =>
  ({
    transition: (handler: string, state: WorkflowState) => ({
      type: 'transition' as const,
      handler,
      state,
    }),
    finish: () => ({ type: 'finish' as const, value: undefined }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  }) as never;

const createGraph = (nodes: Node[]): Graph => ({ nodes });

const createNode = (
  id: string,
  index: number,
  dependsOn: string[] = [],
  status: Node['status'] = 'pending',
): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn,
  status,
  deliver: true,
  index,
  skills: [],
  tools: [],
  artifacts: [],
});
