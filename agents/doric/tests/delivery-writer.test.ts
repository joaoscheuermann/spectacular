import assert from 'node:assert/strict';
import test from 'node:test';

import { writeDelivery } from '../src/lib/delivery-writer.js';

test('writes Markdown once with one final newline and no structured delivery data', () => {
  const writes: string[] = [];

  writeDelivery(
    {
      markdown: '## Finished',
      parts: [
        {
          id: 'private-id',
          goal: 'private goal',
          markdown: '## Finished',
          artifacts: [{ mime: 'application/json', data: '{"secret":true}' }],
          observations: [
            {
              goalId: 'private-id',
              toolName: 'lookup',
              callId: 'call-private',
              input: '{"private":true}',
              output: '{"private":true}',
            },
          ],
        },
      ],
    },
    { write: (text: string) => writes.push(text) } as never,
  );

  assert.deepEqual(writes, ['## Finished\n']);
  assert.doesNotMatch(writes.join(''), /private-id|call-private|secret/u);
});

test('does not add a second newline when Markdown already ends with one', () => {
  const writes: string[] = [];

  writeDelivery({ markdown: 'Finished\n', parts: [] }, {
    write: (text: string) => writes.push(text),
  } as never);

  assert.deepEqual(writes, ['Finished\n']);
});
