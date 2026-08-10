import assert from 'node:assert/strict';
import test from 'node:test';

import type { MosaicEvent, MosaicResult } from 'mosaic';

import { directEvent, mosaicEvent, mosaicResult } from '../src/events.js';

test('direct mapping emits visible text and excludes reasoning deltas', () => {
  assert.deepEqual(directEvent({ type: 'text.delta', delta: 'visible' }), {
    type: 'message_delta',
    delta: 'visible',
  });
  assert.equal(
    directEvent({ type: 'reasoning.delta', delta: 'private' }),
    undefined,
  );
});

test('direct mapping normalizes tool lifecycle without failure details', () => {
  const call = { id: 'call-1', name: 'terminal', payload: { command: 'pwd' } };

  assert.deepEqual(directEvent({ type: 'tool.started', call }), {
    type: 'tool_started',
    callId: 'call-1',
    name: 'terminal',
    input: { command: 'pwd' },
  });
  assert.deepEqual(
    directEvent({ type: 'tool.failed', call, error: new Error('private') }),
    { type: 'tool_failed', callId: 'call-1', name: 'terminal' },
  );
});

test('mosaic mapping uses only public visible response and tool hooks', () => {
  const response = event({
    type: 'model.response',
    stage: 'execution',
    providerId: 'fake',
    operation: 'complete',
    model: 'model',
    durationMs: 1,
    content: { text: 'visible', reasoning: 'not inspected' },
  });
  const tool = event({
    type: 'tool.finished',
    stage: 'execution',
    nodeId: 'node-1',
    revision: 1,
    callId: 'call-1',
    toolName: 'terminal',
    durationMs: 1,
    output: { exit_code: 0 },
  });

  assert.deepEqual(mosaicEvent(response), {
    type: 'message_delta',
    delta: 'visible',
  });
  assert.deepEqual(mosaicEvent(tool), {
    type: 'tool_completed',
    callId: 'call-1',
    name: 'terminal',
    output: { exit_code: 0 },
  });
});

test('mosaic result maps only a completed final delivery', () => {
  const completed: MosaicResult = {
    status: 'completed',
    delivery: { markdown: 'Final answer.', parts: [] },
    nodes: [],
  };
  const blocked: MosaicResult = { status: 'blocked', nodes: [] };

  assert.deepEqual(mosaicResult(completed), {
    type: 'message_delta',
    delta: 'Final answer.',
  });
  assert.equal(mosaicResult(blocked), undefined);
});

const event = (value: object): MosaicEvent =>
  ({ schemaVersion: 2, runId: 'run-1', sequence: 1, ...value }) as MosaicEvent;
