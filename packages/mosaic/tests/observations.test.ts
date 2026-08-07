import assert from 'node:assert/strict';
import test from 'node:test';

import type { ProviderMessage } from 'llms';

import { materializeObservations } from '../src/lib/states/execution/observations.js';

test('materializes correlated observations in tool-result message order', () => {
  const messages: ProviderMessage[] = [
    assistant([
      call('call-second', 'second', '{"value":2}'),
      call('call-first', 'first', '{"value":1}'),
    ]),
    result('call-first', 'first output'),
    result('call-second', 'second output'),
  ];

  const observations = materializeObservations('current', messages);

  assert.deepEqual(
    observations.map(({ toolName, callId, input, output }) => ({
      toolName,
      callId,
      input,
      output,
    })),
    [
      {
        toolName: 'first',
        callId: 'call-first',
        input: '{"value":1}',
        output: 'first output',
      },
      {
        toolName: 'second',
        callId: 'call-second',
        input: '{"value":2}',
        output: 'second output',
      },
    ],
  );
});

test('rejects duplicate tool call IDs', () => {
  const messages: ProviderMessage[] = [
    assistant([
      call('duplicate', 'first', '{}'),
      call('duplicate', 'second', '{}'),
    ]),
    result('duplicate', 'first output'),
  ];

  assert.throws(
    () => materializeObservations('current', messages),
    /Node current has a duplicate tool call ID\./u,
  );
});

test('rejects duplicate tool results', () => {
  const messages: ProviderMessage[] = [
    assistant([call('call-1', 'lookup', '{}')]),
    result('call-1', 'first output'),
    result('call-1', 'second output'),
  ];

  assert.throws(
    () => materializeObservations('current', messages),
    /Node current has a duplicate tool result\./u,
  );
});

test('rejects uncorrelated tool results', () => {
  assert.throws(
    () =>
      materializeObservations('current', [
        result('unknown-call', 'unexpected output'),
      ]),
    /Node current has an uncorrelated tool result\./u,
  );
});

test('rejects tool calls without results', () => {
  assert.throws(
    () =>
      materializeObservations('current', [
        assistant([call('call-1', 'lookup', '{}')]),
      ]),
    /Node current has a tool call without a result\./u,
  );
});

const assistant = (
  toolCalls: NonNullable<ProviderMessage['toolCalls']>,
): ProviderMessage => ({ role: 'assistant', content: '', toolCalls });

const call = (id: string, name: string, args: string) => ({
  id,
  name,
  arguments: args,
});

const result = (toolCallId: string, content: string): ProviderMessage => ({
  role: 'tool',
  toolCallId,
  content,
});
