import assert from 'node:assert/strict';
import test from 'node:test';
import type { VectorDatabase } from 'victor';

import { decompose } from '../src/lib/decomposition/index.js';
import type { Skill } from '../src/lib/types/skill.js';

test('requests an initial P0 execution graph that satisfies the structured schema', async () => {
  const requests: Array<{ messages: Array<{ role: string; content: string }> }> = [];
  const provider = {
    complete: async (request: {
      messages: Array<{ role: string; content: string }>;
    }) => {
      requests.push(request);
      return {
        structured: {
          revision: 'P0',
          nodes: [
            {
              id: 'release-ready',
              goal: 'The release is ready.',
              doneWhen: ['Release checks pass.'],
              dependsOn: [],
              status: 'pending',
              deliver: true,
            },
          ],
        },
      };
    },
  };
  const logger = {
    debug: () => undefined,
  };
  const vectors: VectorDatabase<Skill> = {
    add: async () => undefined,
    search: async () => [],
  };

  await decompose('Prepare a release.', [], [], {
    provider: provider as never,
    logger: logger as never,
    vectors,
  });

  const systemPrompt = requests[0]?.messages[0]?.content;

  assert.equal(requests[0]?.messages[0]?.role, 'system');
  assert.match(systemPrompt ?? '', /initial\s+(?:P0\s+)?execution plan/i);
  assert.match(systemPrompt ?? '', /\bP0\b/);
  assert.match(systemPrompt ?? '', /outcome/i);
  assert.match(systemPrompt ?? '', /observable|verifiable/i);
  assert.match(systemPrompt ?? '', /\bdoneWhen\b/);
  assert.match(systemPrompt ?? '', /\bdependsOn\b/);
  assert.match(systemPrompt ?? '', /direct dependenc/i);
  assert.match(systemPrompt ?? '', /unique/i);
  assert.match(systemPrompt ?? '', /existing/i);
  assert.match(systemPrompt ?? '', /acyclic|\bDAG\b/i);
  assert.match(systemPrompt ?? '', /\bpending\b/);
  assert.match(systemPrompt ?? '', /\bdeliver(?:y)?\b/i);
  assert.match(systemPrompt ?? '', /structured[- ]output schema|provided structured schema/i);
  assert.doesNotMatch(systemPrompt ?? '', /ordered list of (?:initial execution steps|strings)/i);
});
