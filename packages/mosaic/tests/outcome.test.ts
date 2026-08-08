import assert from 'node:assert/strict';
import test from 'node:test';

import { createNodeDecisionSchema } from '../src/lib/schemas/outcome.js';
import type { Node } from '../src/lib/types/graph.js';

const node = createNode();

test('accepts a completed outcome with every criterion and a result', () => {
  const parsed = createNodeDecisionSchema(node).safeParse(completed());

  assert.equal(parsed.success, true);
});

test('rejects completed outcomes with an unsatisfied criterion or no result', () => {
  const unsatisfied = completed();
  unsatisfied.criteria[1]!.satisfied = false;
  const missingResult = { ...completed(), result: null };

  assert.equal(
    createNodeDecisionSchema(node).safeParse(unsatisfied).success,
    false,
  );
  assert.equal(
    createNodeDecisionSchema(node).safeParse(missingResult).success,
    false,
  );
});

test('requires null revision and reason fields for completed outcomes', () => {
  const withReason = { ...completed(), reason: 'Unexpected reason.' };
  const withRevision = {
    ...completed(),
    revisionRequest: {
      goalId: node.id,
      invalidatedAssumption: 'An assumption.',
      requestedEffect: 'A plan effect.',
    },
  };

  assert.equal(
    createNodeDecisionSchema(node).safeParse(withReason).success,
    false,
  );
  assert.equal(
    createNodeDecisionSchema(node).safeParse(withRevision).success,
    false,
  );
});

test('requires exactly one ordered evaluation per doneWhen criterion', () => {
  const missing = completed();
  missing.criteria.pop();
  const reordered = completed();
  reordered.criteria.reverse();

  assert.equal(
    createNodeDecisionSchema(node).safeParse(missing).success,
    false,
  );
  assert.equal(
    createNodeDecisionSchema(node).safeParse(reordered).success,
    false,
  );
});

test('accepts needs_revision only with a node-local semantic request and reason', () => {
  const request = {
    goalId: node.id,
    invalidatedAssumption: 'The target exists.',
    requestedEffect: 'Replace the target-dependent goal.',
  };
  const valid = nonCompleted('needs_revision', {
    revisionRequest: request,
  });
  const wrongGoal = {
    ...valid,
    revisionRequest: { ...request, goalId: 'another-node' },
  };
  const missingRequest = { ...valid, revisionRequest: null };
  const missingReason = { ...valid, reason: null };

  assert.equal(createNodeDecisionSchema(node).safeParse(valid).success, true);
  assert.equal(
    createNodeDecisionSchema(node).safeParse(wrongGoal).success,
    false,
  );
  assert.equal(
    createNodeDecisionSchema(node).safeParse(missingRequest).success,
    false,
  );
  assert.equal(
    createNodeDecisionSchema(node).safeParse(missingReason).success,
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
        invalidatedAssumption: 'An assumption.',
        requestedEffect: 'A plan effect.',
      },
    };

    assert.equal(createNodeDecisionSchema(node).safeParse(valid).success, true);
    assert.equal(
      createNodeDecisionSchema(node).safeParse(withResult).success,
      false,
    );
    assert.equal(
      createNodeDecisionSchema(node).safeParse(withoutReason).success,
      false,
    );
    assert.equal(
      createNodeDecisionSchema(node).safeParse(withRevision).success,
      false,
    );
  }
});

test('rejects legacy observation reference fields and other unknown fields', () => {
  const legacyOutcome = {
    ...completed(),
    observationRefs: ['call-1'],
  };
  const legacyRevision = nonCompleted('needs_revision', {
    revisionRequest: {
      goalId: node.id,
      triggerObservationRef: 'call-1',
      invalidatedAssumption: 'An assumption.',
      requestedEffect: 'A plan effect.',
    },
  });
  const extra = { ...completed(), commentary: 'outside the contract' };

  assert.equal(
    createNodeDecisionSchema(node).safeParse(legacyOutcome).success,
    false,
  );
  assert.equal(
    createNodeDecisionSchema(node).safeParse(legacyRevision).success,
    false,
  );
  assert.equal(createNodeDecisionSchema(node).safeParse(extra).success, false);
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
    outcome: null,
    termination: null,
  };
}
