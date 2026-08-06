import assert from 'node:assert/strict';
import test from 'node:test';

import { createNodeOutcomeSchema } from '../src/lib/schemas/outcome.js';
import type { Node } from '../src/lib/types/graph.js';

const node = createNode();

test('accepts a completed outcome with every criterion and a result', () => {
  const parsed = createNodeOutcomeSchema(node).safeParse(completed());

  assert.equal(parsed.success, true);
});

test('rejects completed outcomes with an unsatisfied criterion or no result', () => {
  const unsatisfied = completed();
  unsatisfied.criteria[1]!.satisfied = false;
  const missingResult = { ...completed(), result: null };

  assert.equal(
    createNodeOutcomeSchema(node).safeParse(unsatisfied).success,
    false,
  );
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(missingResult).success,
    false,
  );
});

test('requires null revision and reason fields for completed outcomes', () => {
  const withReason = { ...completed(), reason: 'Unexpected reason.' };
  const withRevision = {
    ...completed(),
    observationRefs: ['call-1'],
    revisionRequest: {
      goalId: node.id,
      triggerObservationRef: 'call-1',
      invalidatedAssumption: 'An assumption.',
      requestedEffect: 'A plan effect.',
    },
  };

  assert.equal(
    createNodeOutcomeSchema(node).safeParse(withReason).success,
    false,
  );
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(withRevision).success,
    false,
  );
});

test('requires exactly one ordered evaluation per doneWhen criterion', () => {
  const missing = completed();
  missing.criteria.pop();
  const reordered = completed();
  reordered.criteria.reverse();

  assert.equal(createNodeOutcomeSchema(node).safeParse(missing).success, false);
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(reordered).success,
    false,
  );
});

test('accepts needs_revision only with a node-local observed trigger and reason', () => {
  const request = {
    goalId: node.id,
    triggerObservationRef: 'call-1',
    invalidatedAssumption: 'The target exists.',
    requestedEffect: 'Replace the target-dependent goal.',
  };
  const valid = nonCompleted('needs_revision', {
    observationRefs: ['call-1'],
    revisionRequest: request,
  });
  const wrongGoal = {
    ...valid,
    revisionRequest: { ...request, goalId: 'another-node' },
  };
  const missingTrigger = {
    ...valid,
    observationRefs: [],
  };
  const missingRequest = { ...valid, revisionRequest: null };
  const missingReason = { ...valid, reason: null };

  assert.equal(createNodeOutcomeSchema(node).safeParse(valid).success, true);
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(wrongGoal).success,
    false,
  );
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(missingTrigger).success,
    false,
  );
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(missingRequest).success,
    false,
  );
  assert.equal(
    createNodeOutcomeSchema(node).safeParse(missingReason).success,
    false,
  );
});

test('accepts blocked and failed only without a promoted result and with a reason', () => {
  for (const status of ['blocked', 'failed'] as const) {
    const valid = nonCompleted(status);
    const withResult = { ...valid, result: completed().result };
    const withoutReason = { ...valid, reason: null };
    const withRevision = {
      ...valid,
      revisionRequest: {
        goalId: node.id,
        triggerObservationRef: 'call-1',
        invalidatedAssumption: 'An assumption.',
        requestedEffect: 'A plan effect.',
      },
    };

    assert.equal(createNodeOutcomeSchema(node).safeParse(valid).success, true);
    assert.equal(
      createNodeOutcomeSchema(node).safeParse(withResult).success,
      false,
    );
    assert.equal(
      createNodeOutcomeSchema(node).safeParse(withoutReason).success,
      false,
    );
    assert.equal(
      createNodeOutcomeSchema(node).safeParse(withRevision).success,
      false,
    );
  }
});

test('rejects duplicate observation references and unknown fields', () => {
  const duplicate = {
    ...completed(),
    observationRefs: ['call-1', 'call-1'],
  };
  const extra = { ...completed(), commentary: 'outside the contract' };

  assert.equal(
    createNodeOutcomeSchema(node).safeParse(duplicate).success,
    false,
  );
  assert.equal(createNodeOutcomeSchema(node).safeParse(extra).success, false);
});

function completed() {
  return {
    status: 'completed' as const,
    criteria: [
      { criterionIndex: 0, satisfied: true, evidence: 'First proof.' },
      { criterionIndex: 1, satisfied: true, evidence: 'Second proof.' },
    ],
    result: {
      markdown: 'Completed result.',
      artifacts: [{ mime: 'text/plain', data: 'artifact' }],
    },
    observationRefs: [] as string[],
    revisionRequest: null,
    reason: null,
  };
}

function nonCompleted(
  status: 'needs_revision' | 'blocked' | 'failed',
  overrides: Record<string, unknown> = {},
) {
  return {
    ...completed(),
    status,
    result: null,
    revisionRequest: null,
    reason: `${status} reason`,
    ...overrides,
  };
}

function createNode(): Node {
  return {
    id: 'node-current',
    goal: 'Produce the requested outcome.',
    doneWhen: ['The first condition holds.', 'The second condition holds.'],
    dependsOn: [],
    status: 'ready',
    deliver: true,
    index: 0,
    skills: [],
    tools: [],
    artifacts: [],
  };
}
