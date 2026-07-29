import assert from 'node:assert/strict';
import test from 'node:test';

import embedding from '../src/lib/embeedings/index.js';

const model = 'text-embedding-qwen3-embedding-0.6b';

test('requests an embedding through its injected provider and returns the vector', async () => {
  const vector = [1, 2, 3];
  const requests: unknown[] = [];
  const provider = {
    embedding: async (request: unknown) => {
      requests.push(request);
      return vector;
    },
  };

  assert.strictEqual(
    await embedding('Create a release workflow.', provider),
    vector,
  );
  assert.deepEqual(requests, [
    {
      model,
      input: 'Create a release workflow.',
    },
  ]);
});

test('propagates embedding errors from its injected provider', async () => {
  const error = new Error('LM Studio is unavailable.');
  const provider = {
    embedding: async () => {
      throw error;
    },
  };

  await assert.rejects(
    embedding('Create a release workflow.', provider),
    (received) => received === error,
  );
});
