import assert from 'node:assert/strict';
import test from 'node:test';

import { createMachine } from '../src/lib/workflow/machine.js';
import type { Node } from '../src/lib/types/graph.js';

test('finishes completed graphs through delivery without provider calls', async () => {
  let providerCalls = 0;
  const result = await createMachine().run({
    initial: 'schedule',
    state: { graphs: [{ nodes: [node('final')] }] },
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
  if (result.status !== 'finished') return;
  assert.deepEqual(result.value, {
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
  skills: [],
  tools: [],
  artifacts: [{ mime: 'text/markdown', data: 'Done.' }],
  observations: [],
  revisionRequest: null,
});
