import assert from 'node:assert/strict';
import test from 'node:test';

import type { Node } from '../src/lib/types/graph.js';
import { createMachine } from '../src/lib/workflow/machine.js';

test('finishes completed graphs through delivery without provider calls', async () => {
  let providerCalls = 0;

  const result = await createMachine().run({
    initial: 'schedule',
    state: { graphs: [{ revision: 1, nodes: [node('final')] }] },
    context: {
      input: 'Complete the request.',
      options: {
        logger: { info: () => undefined } as never,
        provider: {
          complete: async () => {
            providerCalls += 1;

            throw new Error('Delivery must not call the provider.');
          },
        } as never,
      },
    } as never,
  });

  assert.equal(result.status, 'finished');

  if (result.status !== 'finished') {return;}

  assert.deepEqual(result.value, {
    status: 'completed',
    delivery: {
      markdown: 'Done.',
      parts: [
        {
          id: 'final',
          goal: 'Goal final',
          markdown: 'Done.',
          artifacts: [],
          observations: [],
        },
      ],
    },
    nodes: [
      {
        id: 'final',
        goal: 'Goal final',
        doneWhen: ['final is complete.'],
        status: 'completed',
        candidates: [],
        bundle: null,
        observations: [],
        outcome: completedOutcome(),
        termination: null,
      },
    ],
  });

  assert.equal(providerCalls, 0);
});

const node = (id: string): Node => ({
  id,
  goal: `Goal ${id}`,
  doneWhen: [`${id} is complete.`],
  dependsOn: [],
  deliver: true,
  status: 'completed',
  index: 0,
  candidates: [],
  bundle: null,
  tools: [],
  artifacts: [{ kind: 'inline', mime: 'text/markdown', data: 'Done.' }],
  observations: [],
  outcome: completedOutcome(),
  termination: null,
});

const completedOutcome = () => ({
  status: 'completed' as const,
  criteria: [
    {
      criterionIndex: 0,
      satisfied: true,
      evidence: 'Done.',
      observationIds: [],
    },
  ],
  result: { markdown: 'Done.', artifacts: [] },
  revisionRequest: null,
  reason: null,
});
