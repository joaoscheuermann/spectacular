import assert from 'node:assert/strict';
import test from 'node:test';

import { DECISIONS, runConformance, runDecision } from '../src/study/index.js';

for (const decision of DECISIONS) {
  test(decision.testName, async () => {
    assert.equal(await runDecision(decision.id), true, decision.requirement);
  });
}

test('conformance reports no missing D01-D25 decision or executable test', async () => {
  assert.deepEqual(await runConformance(), []);
});
