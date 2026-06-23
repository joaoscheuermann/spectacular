import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  LlmDebugLogger,
  diagnosticExcerpt,
  parseSseEvents,
} from '../src/index.js';
import { collect } from './fakes.js';

test('writes redacted JSONL debug records when logger is created at a path', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'llms-debug-'));
  const path = join(dir, 'events.jsonl');

  try {
    const logger = LlmDebugLogger.createAtPath(path, {
      clock: () => new Date('2026-06-17T00:00:00.000Z'),
    });

    await logger.log({
      provider: 'openai',
      target: 'responses',
      event: 'http.response',
      fields: {
        excerpt: 'failed with sk-testSecret123 and Bearer token-value-123',
      },
    });

    const record = JSON.parse((await readFile(path, 'utf8')).trim());

    assert.equal(record.timestamp, '2026-06-17T00:00:00.000Z');
    assert.equal(record.provider, 'openai');
    assert.equal(record.target, 'responses');
    assert.equal(record.event, 'http.response');
    assert.equal(
      record.fields.excerpt,
      'failed with sk-[redacted] and Bearer [redacted]',
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('redacts and truncates diagnostic excerpts', () => {
  const excerpt = diagnosticExcerpt(`sk-testSecret123 ${'x'.repeat(20)}`, 16);

  assert.equal(excerpt, 'sk-[redacted] xx...[truncated]');
});

test('parses SSE comments chunk boundaries multi-line data and done markers', async () => {
  const events = await collect(
    parseSseEvents(
      (async function* () {
        yield ': comment\n';
        yield 'event: message\ndata: first\n';
        yield 'data: second\n\n';
        yield 'data: [DONE]\n\n';
      })(),
    ),
  );

  assert.deepEqual(events, [
    { event: 'message', data: 'first\nsecond', done: false },
    { event: undefined, data: '[DONE]', done: true },
  ]);
});
