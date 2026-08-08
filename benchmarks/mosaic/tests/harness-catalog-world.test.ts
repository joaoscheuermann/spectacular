import assert from 'node:assert/strict';
import test from 'node:test';

import { SKILLS, validateCatalog } from '../src/catalog/index.js';
import { TOOL_NAMES, type ToolName } from '../src/config/index.js';
import type { HarnessJsonValue } from '../src/core/index.js';
import {
  TOOLS,
  createWorld,
  executeTool,
  worldHash,
} from '../src/runtime/index.js';

test('catalog freezes 60 English procedural skills with every annotation class', () => {
  assert.equal(SKILLS.length, 60);
  assert.deepEqual(validateCatalog(SKILLS), []);
  assert.deepEqual(
    new Set(
      SKILLS.flatMap((skill) =>
        skill.relations.map((relation) => relation.kind),
      ),
    ),
    new Set(['equivalent', 'overlap', 'distractor', 'conflict']),
  );
  assert.ok(
    SKILLS.every(
      (skill) =>
        skill.body.startsWith('# Outcome\n\n') &&
        skill.body.includes('\n\n# Procedure\n\n1. '),
    ),
  );
});

test('tool registry contains only the frozen 24 world operations', () => {
  assert.deepEqual(
    TOOLS.map((tool) => tool.name),
    [...TOOL_NAMES],
  );
  assert.equal(TOOLS.length, 24);
});

test('mutating tools replace only the supplied isolated world', () => {
  const original = createWorld();
  const written = executeTool(original, 'write', {
    path: 'result.md',
    content: 'first',
  }).world;
  const messaged = executeTool(original, 'message_send', {
    recipientId: 'person-alice',
    body: 'hello',
  }).world;
  assert.equal(original.files['result.md'], undefined);
  assert.equal(original.messages['msg-001'], undefined);
  assert.equal(written.files['result.md'], 'first');
  assert.equal(messaged.messages['msg-001']?.body, 'hello');
  assert.equal(worldHash(createWorld()), worldHash(original));
});

test('every tool repeats equal outputs against equal fixture worlds', () => {
  const inputs = new Map<ToolName, HarnessJsonValue>([
    ['list', {}],
    ['read', { path: 'notes/request.md' }],
    ['write', { path: 'out', content: 'x' }],
    ['search', { query: 'world-v1' }],
    ['calculate', { operation: 'sum', values: [1, 2] }],
    ['json_query', { path: 'data/settings.json', query: 'currency' }],
    ['document_search', { query: 'invoice' }],
    ['document_fetch', { id: 'invoice-001' }],
    ['record_lookup', { id: 'ledger-001' }],
    ['timeseries_query', { series: 'cashflow' }],
    ['currency_convert', { from: 'USD', to: 'BRL', amount: 2 }],
    ['portfolio_snapshot', {}],
    ['symbol_lookup', { name: 'reconcile' }],
    ['dependency_query', { component: 'api' }],
    ['test_run', { target: 'api' }],
    ['build_check', { target: 'api' }],
    ['template_get', { name: 'report' }],
    [
      'schema_validate',
      { name: 'report', value: { title: 'x', summary: 'y' } },
    ],
    ['render_preview', { value: { a: 1 } }],
    ['artifact_publish', { value: { a: 1 } }],
    ['channel_list', {}],
    ['recipient_resolve', { query: 'alice' }],
    ['message_send', { recipientId: 'person-alice', body: 'hello' }],
    ['message_status', { id: 'msg-000' }],
  ]);
  for (const tool of TOOLS) {
    const input = inputs.get(tool.name);
    assert.ok(input !== undefined);
    assert.deepEqual(
      tool.execute(createWorld(), input),
      tool.execute(createWorld(), input),
    );
  }
});
