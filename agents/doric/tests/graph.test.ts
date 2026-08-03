import assert from 'node:assert/strict';
import test from 'node:test';

import { NodeSchema, PlanSchema } from '../src/lib/schemas/graph.js';
import type { Node, Plan } from '../src/lib/types/graph.js';

test('parses graph nodes and plans with the declared structural contract', () => {
  const node: Node = NodeSchema.parse({
    id: 'research',
    goal: 'Collect the required facts.',
    doneWhen: ['Sources are collected.'],
    dependsOn: [],
    status: 'ready',
    deliver: true,
  });

  const plan: Plan = PlanSchema.parse({
    nodes: [node],
    revision: 'P1',
  });

  assert.deepEqual(plan, {
    nodes: [node],
    revision: 'P1',
  });
});

test('rejects graph nodes with an unsupported status', () => {
  assert.throws(() =>
    NodeSchema.parse({
      id: 'research',
      goal: 'Collect the required facts.',
      doneWhen: ['Sources are collected.'],
      dependsOn: [],
      status: 'cancelled',
      deliver: true,
    }),
  );
});
