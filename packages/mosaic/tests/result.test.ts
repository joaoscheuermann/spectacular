import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MosaicResultSchema,
  NodeOutcomeSchema,
  RuntimeTerminationSchema,
  WorkflowNodeResultSchema,
} from '../src/index.js';

test('preserves every model-authored field for every semantic outcome', () => {
  for (const outcome of [
    completedOutcome(),
    revisionOutcome(),
    blockedOutcome(),
    failedOutcome(),
  ]) {
    assert.deepEqual(NodeOutcomeSchema.parse(outcome), outcome);
  }
});

test('accepts only strict runtime-owned termination variants', () => {
  const terminations = [
    {
      type: 'turn_limit',
      status: 'blocked',
      limit: 8,
      observations: [observation()],
    },
    { type: 'revision_limit', status: 'blocked', limit: 3 },
    {
      type: 'dependency',
      status: 'blocked',
      dependencyIds: ['first', 'second'],
    },
  ];

  for (const termination of terminations) {
    assert.equal(RuntimeTerminationSchema.safeParse(termination).success, true);
    assert.equal(
      RuntimeTerminationSchema.safeParse({ ...termination, reason: 'invented' })
        .success,
      false,
    );
  }
  assert.equal(
    RuntimeTerminationSchema.safeParse({
      type: 'dependency',
      status: 'blocked',
      dependencyIds: ['same', 'same'],
    }).success,
    false,
  );
});

test('validates every allowed node status outcome and termination combination', () => {
  const allowed = [
    node('completed', completedOutcome(), null),
    node('blocked', blockedOutcome(), null),
    node('failed', failedOutcome(), null),
    node('blocked', null, {
      type: 'turn_limit',
      status: 'blocked',
      limit: 8,
      observations: [observation()],
    }),
    node('blocked', revisionOutcome(), {
      type: 'revision_limit',
      status: 'blocked',
      limit: 3,
    }),
    node('blocked', null, {
      type: 'dependency',
      status: 'blocked',
      dependencyIds: ['dependency'],
    }),
  ];

  for (const value of allowed) {
    assert.equal(WorkflowNodeResultSchema.safeParse(value).success, true);
  }
});

test('rejects mismatched node status outcome and termination combinations', () => {
  const invalid = [
    node('completed', null, null),
    node('completed', completedOutcome(), {
      type: 'dependency',
      status: 'blocked',
      dependencyIds: ['dependency'],
    }),
    node('failed', blockedOutcome(), null),
    node('blocked', completedOutcome(), null),
    node('blocked', revisionOutcome(), null),
    node('blocked', null, null),
  ];

  for (const value of invalid) {
    assert.equal(WorkflowNodeResultSchema.safeParse(value).success, false);
  }
});

test('allows delivery only on completed workflow results', () => {
  const completed = {
    status: 'completed',
    delivery: {
      markdown: 'Done.',
      parts: [
        {
          id: 'node',
          goal: 'Produce the result.',
          markdown: 'Done.',
          artifacts: [],
          observations: [],
        },
      ],
    },
    nodes: [node('completed', completedOutcome(), null)],
  };
  const blocked = {
    status: 'blocked',
    nodes: [node('blocked', blockedOutcome(), null)],
  };

  assert.equal(MosaicResultSchema.safeParse(completed).success, true);
  assert.equal(MosaicResultSchema.safeParse(blocked).success, true);
  assert.equal(
    MosaicResultSchema.safeParse({
      ...blocked,
      delivery: { markdown: 'partial', parts: [] },
    }).success,
    false,
  );
  assert.equal(
    MosaicResultSchema.safeParse({
      status: 'blocked',
      nodes: [node('failed', failedOutcome(), null)],
    }).success,
    false,
  );
  assert.equal(
    MosaicResultSchema.safeParse({
      ...completed,
      nodes: [node('blocked', blockedOutcome(), null)],
    }).success,
    false,
  );
});

const node = (
  status: 'completed' | 'blocked' | 'failed',
  outcome: unknown,
  termination: unknown,
) => ({
  id: 'node',
  goal: 'Produce the result.',
  doneWhen: ['The result exists.'],
  status,
  outcome,
  termination,
});

const observation = () => ({
  goalId: 'node',
  toolName: 'lookup',
  callId: 'call-1',
  input: '{"query":"value"}',
  output: '{"found":true}',
});

const criteria = (satisfied: boolean) => [
  { criterionIndex: 0, satisfied, evidence: 'Complete evidence.' },
];

const completedOutcome = () => ({
  status: 'completed' as const,
  criteria: criteria(true),
  result: {
    markdown: 'Done.',
    artifacts: [{ mime: 'text/plain', data: 'complete artifact' }],
  },
  revisionRequest: null,
  reason: null,
  observations: [observation()],
});

const blockedOutcome = () => ({
  status: 'blocked' as const,
  criteria: criteria(false),
  result: null,
  revisionRequest: null,
  reason: 'No useful action remains.',
  observations: [observation()],
});

const failedOutcome = () => ({
  ...blockedOutcome(),
  status: 'failed' as const,
  reason: 'The semantic result is invalid.',
});

const revisionOutcome = () => ({
  ...blockedOutcome(),
  status: 'needs_revision' as const,
  revisionRequest: {
    goalId: 'node',
    invalidatedAssumption: 'The resource exists.',
    requestedEffect: 'Replace the dependent goal.',
  },
  reason: 'The plan requires revision.',
});
