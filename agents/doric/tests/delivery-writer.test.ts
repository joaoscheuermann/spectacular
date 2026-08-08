import assert from 'node:assert/strict';
import test from 'node:test';

import { writeResult } from '../src/lib/delivery-writer.js';

test('writes Markdown once with one final newline and no structured delivery data', () => {
  const writes: string[] = [];

  writeResult(
    {
      status: 'completed',
      delivery: {
        markdown: '## Finished',
        parts: [],
      },
      nodes: [],
    },
    { write: (text: string) => writes.push(text) } as never,
    { info: () => undefined },
  );

  assert.deepEqual(writes, ['## Finished\n']);
  assert.doesNotMatch(writes.join(''), /private-id|call-private|secret/u);
});

test('does not add a second newline when Markdown already ends with one', () => {
  const writes: string[] = [];

  writeResult(
    {
      status: 'completed',
      delivery: { markdown: 'Finished\n', parts: [] },
      nodes: [],
    },
    { write: (text: string) => writes.push(text) } as never,
    { info: () => undefined },
  );

  assert.deepEqual(writes, ['Finished\n']);
});

test('logs only status and node IDs for blocked and failed results', () => {
  for (const status of ['blocked', 'failed'] as const) {
    const writes: string[] = [];
    const logs: unknown[] = [];
    writeResult(
      {
        status,
        nodes: [
          {
            id: 'safe-id',
            goal: 'private goal',
            doneWhen: ['private criterion'],
            status,
            outcome: status === 'failed' ? failedOutcome() : blockedOutcome(),
            termination: null,
          },
        ],
      },
      { write: (text: string) => writes.push(text) } as never,
      {
        info: (bindings: unknown, message: string) =>
          logs.push({ bindings, message }),
      },
    );

    assert.deepEqual(writes, []);
    assert.deepEqual(logs, [
      {
        bindings: { status, nodeIds: ['safe-id'] },
        message: 'mosaic workflow did not complete',
      },
    ]);
    assert.doesNotMatch(JSON.stringify(logs), /private/u);
  }
});

const blockedOutcome = () => ({
  status: 'blocked' as const,
  criteria: [{ criterionIndex: 0, satisfied: false, evidence: 'private' }],
  result: null,
  revisionRequest: null,
  reason: 'private',
  observations: [],
});

const failedOutcome = () => ({
  ...blockedOutcome(),
  status: 'failed' as const,
});
