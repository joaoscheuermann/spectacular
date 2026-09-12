import assert from 'node:assert/strict';
import test from 'node:test';

import type { ToolCallRecord } from 'agent';

import { materializeObservations } from '../src/lib/states/execution/observations.js';

test('materializes agent records in append order with opaque IDs intact', () => {
  const records: readonly ToolCallRecord[] = [
    record('observation-1', 'call-1', 'first', '{"value":1}', 'one'),
    record('observation-2', 'call-2', 'second', '{"value":2}', 'two'),
  ];

  assert.deepEqual(materializeObservations('current', records), [
    {
      id: 'observation-1',
      goalId: 'current',
      toolName: 'first',
      callId: 'call-1',
      input: '{"value":1}',
      output: 'one',
    },
    {
      id: 'observation-2',
      goalId: 'current',
      toolName: 'second',
      callId: 'call-2',
      input: '{"value":2}',
      output: 'two',
    },
  ]);
});

test('returns detached observation objects', () => {
  const recordValue = record(
    'observation-1',
    'call-1',
    'lookup',
    '{}',
    'result',
  );
  const observations = materializeObservations('current', [recordValue]);

  assert.notStrictEqual(observations[0], recordValue);

  assert.equal(observations[0]?.id, recordValue.id);
});

const record = (
  id: string,
  callId: string,
  toolName: string,
  input: string,
  output: string,
): ToolCallRecord => ({ id, callId, toolName, input, output });
