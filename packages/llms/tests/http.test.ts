import assert from 'node:assert/strict';
import test from 'node:test';

import { createFetchTransport } from '../src/index.js';

test('includes redacted response diagnostics when opening a stream fails', async () => {
  const transport = createFetchTransport(async () => {
    return new Response('failed with Bearer secret-token-value', {
      status: 400,
    });
  });

  await assert.rejects(
    collect(transport.stream({ method: 'POST', url: 'https://example.test' })),
    /HTTP 400 while opening provider stream: failed with Bearer \[redacted\]/u,
  );
});

const collect = async <T>(items: AsyncIterable<T>): Promise<readonly T[]> => {
  const collected: T[] = [];

  for await (const item of items) {
    collected.push(item);
  }

  return collected;
};
