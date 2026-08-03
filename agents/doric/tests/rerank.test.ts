import assert from 'node:assert/strict';
import test from 'node:test';
import type { HttpRequest, HttpTransport } from 'llms';

import rerank from '../src/lib/decomposition/rerank/index.js';
import type { Skill } from '../src/lib/types/skill.js';

const skills: readonly Skill[] = [
  {
    name: 'write-release-notes',
    description: 'Writes release notes.',
    body: 'Summarize user-visible changes for a software release.',
    allowedTools: [],
  },
  {
    name: 'review-code',
    description: 'Reviews source code.',
    body: 'Inspect source code for correctness and maintainability issues.',
    allowedTools: [],
  },
];

test('returns skills in the relevance order produced by the rerank model', async () => {
  const requests: HttpRequest[] = [];
  const transport: HttpTransport = {
    request: async (request) => {
      requests.push(request);

      return {
        status: 200,
        headers: {},
        body: JSON.stringify({
          results: [
            { index: 1, relevance_score: 0.97 },
            { index: 0, relevance_score: 0.12 },
          ],
        }),
      };
    },
    stream: async function* () {
      return;
    },
  };

  const result = await rerank('Review this implementation.', skills, 2, {
    apiKey: 'test-key',
    transport,
  });

  assert.deepEqual(result, [skills[1], skills[0]]);
  assert.equal(requests.length, 1);
  assert.equal(requests[0]?.url, 'https://openrouter.ai/api/v1/rerank');
  assert.equal(requests[0]?.headers?.authorization, 'Bearer test-key');
  assert.deepEqual(JSON.parse(requests[0]?.body ?? ''), {
    model: 'cohere/rerank-4-pro',
    query: 'Review this implementation.',
    documents: skills.map((skill) => skill.body),
    top_n: 2,
  });
});

test('does not call the rerank API when no skills are provided', async () => {
  let requested = false;
  const transport: HttpTransport = {
    request: async () => {
      requested = true;
      throw new Error('Unexpected request.');
    },
    stream: async function* () {
      return;
    },
  };

  const result = await rerank('Any query.', [], 1, {
    apiKey: 'test-key',
    transport,
  });

  assert.deepEqual(result, []);
  assert.equal(requested, false);
});
