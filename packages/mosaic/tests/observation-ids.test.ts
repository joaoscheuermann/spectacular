import assert from 'node:assert/strict';
import test from 'node:test';

import { createObservationIdAllocator } from '../src/lib/observation-ids.js';

test('allocates the first six lowercase hexadecimal UUID characters', () => {
  const ids = createObservationIdAllocator(
    () => 'ABCDEF12-3456-4789-8abc-def012345678',
  );

  assert.equal(ids.next(), 'abcdef');
});

test('retries collisions reserved by snapshots and the current wave', () => {
  const values = [
    'aaaaaa00-0000-4000-8000-000000000000',
    'bbbbbb00-0000-4000-8000-000000000000',
    'bbbbbb11-1111-4111-8111-111111111111',
    'cccccc00-0000-4000-8000-000000000000',
  ];
  const ids = createObservationIdAllocator(() => values.shift()!);
  ids.reserve(['aaaaaa']);

  assert.equal(ids.next(), 'bbbbbb');
  assert.equal(ids.next(), 'cccccc');
});

test('fails operationally after 32 consecutive collisions', () => {
  const ids = createObservationIdAllocator(
    () => 'aaaaaa00-0000-4000-8000-000000000000',
  );
  ids.reserve(['aaaaaa']);

  assert.throws(() => ids.next(), /allocation exhausted after 32 collisions/u);
});

test('claims caller-supplied canonical IDs without permitting reuse', () => {
  const ids = createObservationIdAllocator(
    () => 'bbbbbb00-0000-4000-8000-000000000000',
  );

  ids.claim(['aaaaaa']);

  assert.throws(() => ids.claim(['aaaaaa']), /already reserved/u);
  assert.throws(() => ids.claim(['ABCDEF']), /six lowercase hexadecimal/u);
  assert.throws(() => ids.claim(['not-hex']), /six lowercase hexadecimal/u);
  assert.equal(ids.next(), 'bbbbbb');
});

test('claims caller-supplied IDs atomically', () => {
  const values = [
    'bbbbbb00-0000-4000-8000-000000000000',
    'cccccc00-0000-4000-8000-000000000000',
  ];
  const ids = createObservationIdAllocator(() => values.shift()!);

  assert.throws(
    () => ids.claim(['bbbbbb', 'not-hex']),
    /six lowercase hexadecimal/u,
  );
  assert.equal(ids.next(), 'bbbbbb');
});
