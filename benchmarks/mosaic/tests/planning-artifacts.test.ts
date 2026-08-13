import assert from 'node:assert/strict';
import test from 'node:test';

import { planningBenchmarkManifest } from '../src/composition/planning-artifacts.js';

test('pins the complete controlled 6-by-4 planning matrix', () => {
  const first = planningBenchmarkManifest();
  const second = planningBenchmarkManifest();

  assert.equal(first.caseCount, 24);
  assert.deepEqual(first.domains, {
    'documents-finance': 6,
    software: 6,
    artifacts: 6,
    communications: 6,
  });
  assert.deepEqual(first.compositionClasses, {
    A: 4,
    B: 4,
    C: 4,
    D: 4,
    E: 4,
    F: 4,
  });
  assert.match(first.casesSha256, /^[a-f0-9]{64}$/u);
  assert.deepEqual(second, first);
});
