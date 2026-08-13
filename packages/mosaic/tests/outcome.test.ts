import assert from 'node:assert/strict';
import test from 'node:test';
import { z } from 'zod';

import {
  NodeDecisionSchema,
  NodeOutcomeSchema,
  createNodeDecisionSchema,
  createExecutionDecisionSchema,
  createNodeOutcomeSchema,
} from '../src/lib/schemas/outcome.js';
import type { Graph, Node } from '../src/lib/types/graph.js';

const node = createNode();

test('accepts a completed outcome with every criterion and a result', () => {
  const parsed = createNodeDecisionSchema(node).safeParse(completed());

  assert.equal(parsed.success, true);
});

test('accepts unique opaque observation IDs including an empty proof set', () => {
  const decision = completed();
  decision.criteria[0]!.observationIds = ['observation-a', 'observation-c'];
  decision.criteria[1]!.observationIds = [];

  assert.equal(
    createNodeDecisionSchema(node).safeParse(decision).success,
    true,
  );
});

test('rejects empty or duplicate observation IDs', () => {
  for (const observationIds of [[''], ['observation-a', 'observation-a']]) {
    const decision = completed();
    decision.criteria[0]!.observationIds = observationIds;

    assert.equal(
      createNodeDecisionSchema(node).safeParse(decision).success,
      false,
    );
  }
});

test('rejects an observation ID that was not presented to the node', () => {
  const observedNode = createNode();
  observedNode.observations = [observation('observation-1', observedNode.id)];
  const decision = completed();
  decision.criteria[0]!.observationIds = ['observation-unknown'];

  assert.equal(
    createNodeOutcomeSchema(observedNode).safeParse(decision).success,
    false,
  );
});

test('requires a fresh local citation only for post-revision completion', () => {
  const freshId = 'fresh-local';
  const ancestorId = 'projected-ancestor';
  const schema = createExecutionDecisionSchema(
    node,
    () => [freshId, ancestorId],
    () => [freshId],
  );
  const withoutCitation = completed();
  const ancestorOnly = completed();
  ancestorOnly.criteria[0]!.observationIds = [ancestorId];
  const withFreshCitation = completed();
  withFreshCitation.criteria[0]!.observationIds = [freshId];
  const blocked = nonCompleted('blocked');
  blocked.criteria[0]!.satisfied = false;

  assert.equal(schema.safeParse(withoutCitation).success, false);
  assert.equal(schema.safeParse(ancestorOnly).success, false);
  assert.equal(schema.safeParse(withFreshCitation).success, true);
  assert.equal(schema.safeParse(blocked).success, true);
});

test('describes invalid observation IDs with bounded similarity-ranked authorized IDs', () => {
  const decision = completed();
  decision.criteria[0]!.observationIds = ['private-observation'];
  const authorized = [
    'observation-00',
    'observation-10',
    'observation-20',
    'observation-30',
    'observation-40',
    'observation-50',
    'observation-60',
    'observation-70',
    'observation-80',
    'observation-90',
    'observation-100',
    'observation-110',
  ];
  const parsed = createExecutionDecisionSchema(
    createNode(),
    () => authorized,
  ).safeParse(decision);

  assert.equal(parsed.success, false);
  if (parsed.success) return;
  assert.deepEqual(parsed.error.issues[0]?.path, [
    'criteria',
    0,
    'observationIds',
    0,
  ]);
  const message = parsed.error.issues[0]?.message ?? '';
  assert.match(message, /^The observation ID is not authorized\./u);
  assert.match(message, /Most similar valid observation ID: observation-00/u);
  assert.match(
    message,
    /Other valid observation IDs: observation-10, observation-20, observation-30, observation-40, observation-50, observation-60, observation-70, observation-80, observation-90/u,
  );
  assert.match(message, /Showing 10 of 12 valid observation IDs; 2 omitted\./u);
  assert.doesNotMatch(
    message,
    /observation-100|observation-110|private-observation/u,
  );
});

test('uses causal order to break equal observation ID distances', () => {
  const decision = completed();
  decision.criteria[0]!.observationIds = ['observation-aa'];
  const parsed = createExecutionDecisionSchema(createNode(), () => [
    'observation-ab',
    'observation-ac',
  ]).safeParse(decision);

  assert.equal(parsed.success, false);
  if (parsed.success) return;
  assert.match(
    parsed.error.issues[0]?.message ?? '',
    /Most similar valid observation ID: observation-ab\nOther valid observation IDs: observation-ac/u,
  );
});

test('resolves authorized observation IDs during every execution parse', () => {
  const decision = completed();
  decision.criteria[0]!.observationIds = ['observation-later'];
  const authorized: string[] = [];
  const schema = createExecutionDecisionSchema(createNode(), () => authorized);

  assert.equal(schema.safeParse(decision).success, false);
  authorized.push('observation-later');
  assert.equal(schema.safeParse(decision).success, true);
});

test('describes single and empty authorized observation ID sets', () => {
  const decision = completed();
  decision.criteria[0]!.observationIds = ['rejected-private-id'];

  const single = createExecutionDecisionSchema(createNode(), () => [
    'only-authorized-id',
  ]).safeParse(decision);
  assert.equal(single.success, false);
  if (!single.success) {
    assert.match(
      single.error.issues[0]?.message ?? '',
      /Most similar valid observation ID: only-authorized-id\nOther valid observation IDs: none\./u,
    );
  }

  const empty = createExecutionDecisionSchema(createNode(), () => []).safeParse(
    decision,
  );
  assert.equal(empty.success, false);
  if (!empty.success) {
    assert.match(
      empty.error.issues[0]?.message ?? '',
      /Most similar valid observation ID: none\.\nOther valid observation IDs: none\.\nUse \[\]\./u,
    );
    assert.doesNotMatch(
      empty.error.issues[0]?.message ?? '',
      /rejected-private-id/u,
    );
  }
});

test('accepts local and cited transitive-ancestor observation IDs', () => {
  const root = createNode();
  root.id = 'root';
  root.status = 'completed';
  root.observations = [observation('observation-root', root.id)];
  const rootOutcome = completed();
  rootOutcome.criteria[0]!.observationIds = ['observation-root'];
  root.outcome = rootOutcome;

  const middle = createNode();
  middle.id = 'middle';
  middle.status = 'completed';
  middle.dependsOn = ['root'];
  const middleOutcome = completed();
  middleOutcome.criteria[0]!.observationIds = ['observation-root'];
  middle.outcome = middleOutcome;

  const current = createNode();
  current.id = 'current';
  current.dependsOn = ['middle'];
  current.observations = [observation('observation-local', current.id)];
  const graph: Graph = { revision: 1, nodes: [root, middle, current] };
  const decision = completed();
  decision.criteria[0]!.observationIds = [
    'observation-root',
    'observation-local',
  ];

  assert.equal(
    createNodeOutcomeSchema(current, graph).safeParse(decision).success,
    true,
  );
});

test('rejects uncited ancestor cross-branch descendant and retired observation IDs', () => {
  const ancestor = createNode();
  ancestor.id = 'ancestor';
  ancestor.status = 'completed';
  ancestor.observations = [observation('observation-ancestor', ancestor.id)];
  ancestor.outcome = completed();

  const current = createNode();
  current.id = 'current';
  current.dependsOn = ['ancestor'];

  const sibling = createNode();
  sibling.id = 'sibling';
  sibling.status = 'completed';
  sibling.observations = [observation('observation-sibling', sibling.id)];
  const siblingOutcome = completed();
  siblingOutcome.criteria[0]!.observationIds = ['observation-sibling'];
  sibling.outcome = siblingOutcome;

  const descendant = createNode();
  descendant.id = 'descendant';
  descendant.dependsOn = ['current'];
  descendant.observations = [
    observation('observation-descendant', descendant.id),
  ];

  const graph: Graph = {
    revision: 2,
    nodes: [ancestor, current, sibling, descendant],
  };

  for (const id of [
    'observation-ancestor',
    'observation-sibling',
    'observation-descendant',
    'observation-retired',
  ]) {
    const decision = completed();
    decision.criteria[0]!.observationIds = [id];
    assert.equal(
      createNodeOutcomeSchema(current, graph).safeParse(decision).success,
      false,
      id,
    );
  }
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

test('rejects blocked decisions and outcomes when every criterion is satisfied', () => {
  const blocked = nonCompleted('blocked');
  const schemas = [
    NodeDecisionSchema,
    createNodeDecisionSchema(node),
    NodeOutcomeSchema,
    createNodeOutcomeSchema(node),
  ];

  schemas.forEach((schema) => {
    const parsed = schema.safeParse(blocked);

    assert.equal(parsed.success, false);
    if (parsed.success) return;
    assert.deepEqual(parsed.error.issues[0]?.path, ['criteria']);
    assert.equal(
      parsed.error.issues[0]?.message,
      'A blocked outcome requires at least one unsatisfied criterion.',
    );
  });
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
    if (status === 'blocked') valid.criteria[0]!.satisfied = false;
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

test('describes every model-facing decision field with opaque ID scope', () => {
  const schema = z.toJSONSchema(createNodeDecisionSchema(node), {
    target: 'draft-2020-12',
  });

  assertPropertiesAreDescribed(schema);
  assert.match(
    JSON.stringify(schema.properties?.criteria),
    /opaque observation IDs from this node or the projected ancestor evidence/u,
  );
  assert.match(
    JSON.stringify(schema.properties?.criteria),
    /Use \[\] when proof requires no tool result/u,
  );
  assert.match(
    JSON.stringify(schema.properties?.criteria),
    /position in the current node's doneWhen array/u,
  );
});

function assertPropertiesAreDescribed(value: unknown): void {
  if (Array.isArray(value)) {
    value.forEach(assertPropertiesAreDescribed);
    return;
  }
  if (typeof value !== 'object' || value === null) return;

  const record = value as Record<string, unknown>;
  const properties = record.properties;
  if (typeof properties === 'object' && properties !== null) {
    Object.entries(properties).forEach(([name, property]) => {
      assert.equal(
        typeof (property as Record<string, unknown>).description,
        'string',
        `${name} must have a JSON Schema description`,
      );
    });
  }
  Object.values(record).forEach(assertPropertiesAreDescribed);
}

function completed() {
  return {
    status: 'completed' as const,
    criteria: [
      {
        criterionIndex: 0,
        satisfied: true,
        evidence: 'First proof.',
        observationIds: [] as string[],
      },
      {
        criterionIndex: 1,
        satisfied: true,
        evidence: 'Second proof.',
        observationIds: [] as string[],
      },
    ],
    result: {
      markdown: 'Completed result.',
      artifacts: [
        { kind: 'inline' as const, mime: 'text/plain', data: 'artifact' },
      ],
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
    candidates: [],
    bundle: null,
    tools: [],
    artifacts: [],
    observations: [],
    outcome: null,
    termination: null,
  };
}

const observation = (id: string, goalId: string) => ({
  id,
  goalId,
  toolName: 'inspect',
  callId: `call-${id}`,
  input: '{}',
  output: '{}',
});
