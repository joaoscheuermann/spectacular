import assert from 'node:assert/strict';
import test from 'node:test';
import { z } from 'zod';

import {
  createNodeDecisionSchema,
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
