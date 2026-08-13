import assert from 'node:assert/strict';
import test from 'node:test';

import { revisionExecutionHandoff } from '../src/lib/states/execution/handoff.js';
import type { Graph, Node } from '../src/lib/types/graph.js';

test('hands an exactly retained revision target its prior attempt', () => {
  const target = revisionNode('target', 1, {
    invalidatedAssumption: 'The old structure exists.',
    requestedEffect: 'Use the replacement structure.',
  });
  const unchanged = node('unchanged', 0);
  const changed = node('changed', 2);
  const revisedTarget = node('target', 1);
  const revisedChanged = node('changed', 2);
  revisedChanged.goal = 'Changed pending goal.';
  const graphs: Graph[] = [
    { revision: 1, nodes: [unchanged, target, changed] },
    {
      revision: 2,
      nodes: [node('unchanged', 0), revisedTarget, revisedChanged],
    },
  ];

  assert.deepEqual(revisionExecutionHandoff(revisedTarget, graphs), {
    invalidatedAssumption: 'The old structure exists.',
    requestedEffect: 'Use the replacement structure.',
    falseCriteria: [{ criterionIndex: 0, text: 'Criterion target.' }],
    observations: [
      {
        toolName: 'inspect-target',
        input: '{"target":"target"}',
        output: 'target output',
      },
    ],
    omittedObservationCount: 0,
  });
  assert.equal(
    revisionExecutionHandoff(graphs[1]!.nodes[0]!, graphs),
    undefined,
  );
});

test('walks stacked revisions to hand a replacement the newest applicable request', () => {
  const first = revisionNode('first', 0, {
    invalidatedAssumption: 'First assumption.',
    requestedEffect: 'Revise first.',
  });
  const second = revisionNode('second', 1, {
    invalidatedAssumption: 'Second assumption.',
    requestedEffect: 'Replace second.',
  });
  second.observations.unshift({
    id: 'aaaaaa',
    goalId: second.id,
    toolName: 'unlinked',
    callId: 'unlinked-call',
    input: '{}',
    output: 'unlinked output',
  });
  const replacementAtTwo = node('replacement', 1);
  replacementAtTwo.goal = 'Replacement goal.';
  const replacementAtThree = node('replacement', 0);
  replacementAtThree.goal = replacementAtTwo.goal;
  const graphs: Graph[] = [
    { revision: 1, nodes: [first, second] },
    { revision: 2, nodes: [first, replacementAtTwo] },
    { revision: 3, nodes: [replacementAtThree] },
  ];

  const handoff = revisionExecutionHandoff(replacementAtThree, graphs);

  assert.deepEqual(handoff, {
    invalidatedAssumption: 'Second assumption.',
    requestedEffect: 'Replace second.',
    falseCriteria: [{ criterionIndex: 0, text: 'Criterion second.' }],
    observations: [
      {
        toolName: 'inspect-second',
        input: '{"target":"second"}',
        output: 'second output',
      },
    ],
    omittedObservationCount: 0,
  });
  assert.equal('id' in handoff!.observations[0]!, false);
  assert.equal('callId' in handoff!.observations[0]!, false);
});

test('prefers a replacement lineage over a newer collateral pending edit', () => {
  const first = revisionNode('first', 0, {
    invalidatedAssumption: 'First assumption.',
    requestedEffect: 'Revise first.',
  });
  const second = revisionNode('second', 1, {
    invalidatedAssumption: 'Second assumption.',
    requestedEffect: 'Replace second.',
  });
  const replacementAtTwo = node('replacement', 1);
  replacementAtTwo.goal = 'Initial replacement goal.';
  const replacementAtThree = node('replacement', 1);
  replacementAtThree.goal = 'Collateral edit from the first revision.';

  const handoff = revisionExecutionHandoff(replacementAtThree, [
    { revision: 1, nodes: [first, second] },
    { revision: 2, nodes: [first, replacementAtTwo] },
    { revision: 3, nodes: [node('first', 0), replacementAtThree] },
  ]);

  assert.equal(handoff?.invalidatedAssumption, 'Second assumption.');
  assert.equal(handoff?.requestedEffect, 'Replace second.');
});

test('prefers a merge lineage over a newer collateral pending edit', () => {
  const first = revisionNode('first', 0, {
    invalidatedAssumption: 'First assumption.',
    requestedEffect: 'Revise first.',
  });
  const second = revisionNode('second', 1, {
    invalidatedAssumption: 'Second assumption.',
    requestedEffect: 'Merge second into pending.',
  });
  const pendingAtOne = node('pending', 2);
  const mergedAtTwo = node('pending', 1);
  mergedAtTwo.goal = 'Pending now absorbs second.';
  const editedAtThree = node('pending', 1);
  editedAtThree.goal = 'Collateral edit while revising first.';

  const handoff = revisionExecutionHandoff(editedAtThree, [
    { revision: 1, nodes: [first, second, pendingAtOne] },
    { revision: 2, nodes: [first, mergedAtTwo] },
    { revision: 3, nodes: [node('first', 0), editedAtThree] },
  ]);

  assert.equal(handoff?.invalidatedAssumption, 'Second assumption.');
  assert.equal(handoff?.requestedEffect, 'Merge second into pending.');
});

test('omits unlinked local observations instead of copying the prior ledger', () => {
  const target = revisionNode('target', 0, {
    invalidatedAssumption: 'Assumption.',
    requestedEffect: 'Effect.',
  });
  target.outcome!.criteria[0]!.observationIds = ['ancestor-only'];
  target.observations.push({
    id: 'bbbbbb',
    goalId: target.id,
    toolName: 'confirm-target',
    callId: 'confirm-call',
    input: '{}',
    output: 'confirmation output',
  });
  const revised = node('target', 0);

  const handoff = revisionExecutionHandoff(revised, [
    { revision: 1, nodes: [target] },
    { revision: 2, nodes: [revised] },
  ]);

  assert.deepEqual(handoff?.observations, []);
  assert.equal(handoff?.omittedObservationCount, 0);
});

test('bounds linked historical observations and their payloads', () => {
  const target = revisionNode('target', 0, {
    invalidatedAssumption: 'Assumption.',
    requestedEffect: 'Effect.',
  });
  target.observations = Array.from({ length: 7 }, (_, index) => ({
    id: `${index + 1}`.repeat(6),
    goalId: target.id,
    toolName: `inspect-${index}`,
    callId: `call-${index}`,
    input: `input-${index}-${'i'.repeat(900)}`,
    output: `output-${index}-${'o'.repeat(900)}`,
  }));
  target.outcome!.criteria[0]!.observationIds = target.observations.map(
    ({ id }) => id,
  );
  const revised = node('target', 0);

  const handoff = revisionExecutionHandoff(revised, [
    { revision: 1, nodes: [target] },
    { revision: 2, nodes: [revised] },
  ]);

  assert.deepEqual(
    handoff?.observations.map(({ toolName }) => toolName),
    ['inspect-2', 'inspect-3', 'inspect-4', 'inspect-5', 'inspect-6'],
  );
  assert.equal(handoff?.omittedObservationCount, 2);
  assert.ok(
    handoff?.observations.every(
      ({ input, output }) =>
        input.length < 900 &&
        output.length < 900 &&
        input.includes('historical characters omitted') &&
        output.includes('historical characters omitted'),
    ),
  );
});

const node = (id: string, index: number): Node => ({
  id,
  goal: `Goal ${id}.`,
  doneWhen: [`Criterion ${id}.`],
  dependsOn: [],
  status: 'pending',
  deliver: true,
  index,
  candidates: [],
  bundle: null,
  tools: [],
  artifacts: [],
  observations: [],
  outcome: null,
  termination: null,
});

const revisionNode = (
  id: string,
  index: number,
  request: {
    readonly invalidatedAssumption: string;
    readonly requestedEffect: string;
  },
): Node => {
  const target = node(id, index);
  target.status = 'needs_revision';
  target.observations = [
    {
      id: 'cccccc',
      goalId: id,
      toolName: `inspect-${id}`,
      callId: `${id}-call`,
      input: `{"target":"${id}"}`,
      output: `${id} output`,
    },
  ];
  target.outcome = {
    status: 'needs_revision',
    criteria: [
      {
        criterionIndex: 0,
        satisfied: false,
        evidence: 'Criterion is not satisfied.',
        observationIds: ['cccccc'],
      },
    ],
    result: null,
    revisionRequest: { goalId: id, ...request },
    reason: 'The plan must change.',
  };
  return target;
};
