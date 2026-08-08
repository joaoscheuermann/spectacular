import assert from 'node:assert/strict';
import test from 'node:test';

import { schedule } from '../src/lib/states/schedule/index.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type { WorkflowState } from '../src/lib/types/workflow.js';

test('fails when the workflow has no active graph', async () => {
  const action = await schedule(state([]), {} as never, handlers());

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    (action.error as Error).message,
    'Impossible to continue, missing active graph!',
  );
});

test('routes every completed graph through delivery', async () => {
  const graph = createGraph([
    createNode('first', 0, [], 'completed'),
    createNode('second', 1, [], 'completed'),
  ]);

  const action = await schedule(state([graph]), {} as never, handlers());

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'delivery',
    state: state([graph]),
  });
});

test('schedules only the last graph in the workflow', async () => {
  const older = createGraph([createNode('older', 0)]);
  const active = createGraph([createNode('active', 0)]);
  const graphs = [older, active];

  const workflow = state(graphs);
  const action = await schedule(workflow, {} as never, handlers());

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'bundle',
    state: workflow,
  });
  assert.equal(older.nodes[0]?.status, 'pending');
  assert.equal(active.nodes[0]?.status, 'ready');
});

test('marks a dependent node ready when all its dependencies are completed', async () => {
  const completed = createNode('completed', 0, [], 'completed');
  const dependent = createNode('dependent', 1, ['completed']);
  const graph = createGraph([completed, dependent]);

  const action = await schedule(state([graph]), {} as never, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(completed.status, 'completed');
  assert.equal(dependent.status, 'ready');
});

test('blocks descendants causally and continues an independent branch', async () => {
  const blocked = createNode('blocked', 0, [], 'blocked');
  blocked.outcome = {
    status: 'blocked',
    criteria: [
      { criterionIndex: 0, satisfied: false, evidence: 'Unavailable.' },
    ],
    result: null,
    revisionRequest: null,
    reason: 'No useful action remains.',
    observations: [],
  };
  const child = createNode('child', 1, ['blocked']);
  const grandchild = createNode('grandchild', 2, ['child']);
  const independent = createNode('independent', 3);
  const graph = createGraph([blocked, child, grandchild, independent]);

  const action = await schedule(state([graph]), {} as never, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(action.type === 'transition' ? action.handler : '', 'bundle');
  assert.equal(independent.status, 'ready');
  assert.deepEqual(child.termination, {
    type: 'dependency',
    status: 'blocked',
    dependencyIds: ['blocked'],
  });
  assert.deepEqual(grandchild.termination, {
    type: 'dependency',
    status: 'blocked',
    dependencyIds: ['child'],
  });
});

test('routes a node-owned localized revision to revision before scheduling work', async () => {
  const target = createNode('target', 0, [], 'needs_revision');
  const graph = createGraph([target]);
  const workflow = state([graph]);
  const observation = {
    goalId: 'target',
    toolName: 'lookup',
    callId: 'call-target',
    input: '{}',
    output: '{}',
  };
  target.outcome = {
    status: 'needs_revision',
    criteria: [
      { criterionIndex: 0, satisfied: false, evidence: 'Not complete.' },
    ],
    result: null,
    revisionRequest: {
      goalId: 'target',
      invalidatedAssumption: 'Original structure.',
      requestedEffect: 'Revise it.',
    },
    reason: 'Revision required.',
    observations: [observation],
  };

  const action = await schedule(workflow, {} as never, handlers());

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'revision',
    state: workflow,
  });
});

test('fails safely when needs_revision has no runtime request', async () => {
  const target = createNode('target', 0, [], 'needs_revision');
  const workflow = state([createGraph([target])]);

  const action = await schedule(workflow, {} as never, handlers());

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    (action.error as Error).message,
    'Revision node is missing its runtime request.',
  );
  assert.equal(target.status, 'needs_revision');
});

test('marks at most five eligible nodes ready in descending index order', async () => {
  const nodes = Array.from({ length: 6 }, (_, index) =>
    createNode(`node-${index}`, index),
  );
  const graph = createGraph(nodes);

  const action = await schedule(state([graph]), {} as never, handlers());

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

  const action = await schedule(state([graph]), {} as never, handlers());

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

const state = (graphs: Graph[]): WorkflowState => ({
  graphs,
});

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
  outcome: status === 'completed' ? completedOutcome(id) : null,
  termination: null,
});

const completedOutcome = (id: string) => ({
  status: 'completed' as const,
  criteria: [
    { criterionIndex: 0, satisfied: true, evidence: `${id} is complete.` },
  ],
  result: { markdown: `${id} result`, artifacts: [] },
  revisionRequest: null,
  reason: null,
  observations: [],
});
