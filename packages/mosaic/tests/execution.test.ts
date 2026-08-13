import assert from 'node:assert/strict';
import test from 'node:test';

import type { Skill } from 'bundle';
import type { LlmProvider, ProviderFinished, ProviderRequest } from 'llms';
import { z } from 'zod';
import type { Tool } from 'tool';

import { execution } from '../src/lib/states/execution/index.js';
import type { NodeDecision } from '../src/lib/schemas/outcome.js';
import type { Graph, Node } from '../src/lib/types/graph.js';
import type {
  WorkflowContext,
  WorkflowState,
} from '../src/lib/types/workflow.js';
import { mosaicProviders } from './structured.js';
import { createObservationIdAllocator } from '../src/lib/observation-ids.js';

test('fails without executing workflow work when no graph exists', async () => {
  const action = await execution(
    state([]),
    {} as never,
    {
      fail: (error: unknown) => ({ type: 'fail' as const, error }),
    } as never,
  );

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(
    (action.error as Error).message,
    'Impossible to continue, missing active graph!',
  );
});

test('completes a node stores result artifacts and schedules the next wave', async () => {
  const node = createNode('current');
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    terminalFinish(
      completed({
        result: {
          markdown: ' \n## Final result\n',
          artifacts: [{ kind: 'inline', mime: 'text/plain', data: 'extra' }],
        },
      }),
    ),
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.deepEqual(action, {
    type: 'transition',
    handler: 'schedule',
    state: state([graph]),
  });
  assert.equal(node.status, 'completed');
  assert.deepEqual(node.artifacts, [
    {
      kind: 'inline',
      mime: 'text/markdown',
      data: ' \n## Final result\n',
    },
    { kind: 'inline', mime: 'text/plain', data: 'extra' },
  ]);
  assert.equal(provider.requests.length, 1);
  assert.equal(provider.requests[0]?.schema, undefined);
  assert.equal(provider.requests[0]?.tools?.length, 1);
  assert.equal(provider.requests[0]?.messages[0]?.role, 'system');
  assert.ok(provider.requests[0]?.messages.some(({ role }) => role === 'user'));
});

test('automatically records one returned tool observation for a completed node', async () => {
  const node = createNode('current', ['lookup']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolFinish('call-1', 'lookup'),
    terminalFinish(completed()),
  ]);
  const calls: unknown[] = [];
  const harness = createHarness(graph, provider, [
    tool('lookup', async (payload) => {
      calls.push(payload);
      return { found: true };
    }),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(
    action.type,
    'transition',
    action.type === 'fail' ? String(action.error) : undefined,
  );
  assert.equal(node.status, 'completed');
  assert.deepEqual(calls, [{ query: 'evidence' }]);
  assert.equal(provider.requests.length, 2);
  assert.equal(provider.requests[0]?.tools?.[0]?.name, 'lookup');
  assert.equal(provider.requests[0]?.schema, undefined);
  assert.equal(provider.requests[1]?.schema, undefined);
  assert.equal(provider.requests[0]?.tools?.length, 2);
  if (action.type !== 'transition') return;
  assert.deepEqual(
    node.observations.map(({ goalId, toolName, callId, input, output }) => ({
      goalId,
      toolName,
      callId,
      input,
      output,
    })),
    [
      {
        goalId: 'current',
        toolName: 'lookup',
        callId: 'call-1',
        input: '{"query":"evidence"}',
        output: '{"found":true}',
      },
    ],
  );
  assert.match(node.observations[0]?.id ?? '', /^[0-9a-f]{6}$/u);
});

test('does not reuse an observation ID retained by an earlier revision', async () => {
  const historical = createNode('historical');
  historical.observations = [
    {
      id: 'aaaaaa',
      goalId: historical.id,
      toolName: 'lookup',
      callId: 'historical-call',
      input: '{}',
      output: '{}',
    },
  ];
  const node = createNode('current', ['lookup']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolFinish('call-1', 'lookup'),
    terminalFinish(completed()),
  ]);
  const harness = createHarness(graph, provider, [tool('lookup')]);
  const uuids = [
    'aaaaaa00-0000-4000-8000-000000000000',
    'bbbbbb00-0000-4000-8000-000000000000',
  ];

  const action = await execution(
    state([{ revision: 0, nodes: [historical] }, graph]),
    {
      ...harness.context,
      observationIds: createObservationIdAllocator(() => uuids.shift()!),
    },
    handlers(),
  );

  assert.equal(action.type, 'transition');
  assert.equal(node.observations[0]?.id, 'bbbbbb');
});

test('hands a replacement node the newest applicable prior attempt without authorizing old evidence', async () => {
  const first = createNode('first');
  first.index = 0;
  first.doneWhen = ['First revision criterion.'];
  first.status = 'needs_revision';
  first.observations = [
    {
      id: '111111',
      goalId: first.id,
      toolName: 'inspect-first',
      callId: 'call-first-history',
      input: '{}',
      output: 'newer unrelated revision output',
    },
  ];
  first.outcome = {
    ...nonCompleted('needs_revision', 'Revise first.'),
    revisionRequest: {
      goalId: first.id,
      invalidatedAssumption: 'Newer unrelated assumption.',
      requestedEffect: 'Revise the first node.',
    },
  };

  const second = createNode('second');
  second.index = 1;
  second.doneWhen = ['Already satisfied.', 'Old target must be replaced.'];
  second.status = 'needs_revision';
  second.observations = [
    {
      id: '222222',
      goalId: second.id,
      toolName: 'inspect-unlinked',
      callId: 'call-unlinked-history',
      input: '{"scope":"unlinked"}',
      output: 'unlinked historical output',
    },
    {
      id: '333333',
      goalId: second.id,
      toolName: 'inspect-linked',
      callId: 'call-linked-history',
      input: '{"scope":"linked"}',
      output: 'linked historical output',
    },
  ];
  second.outcome = {
    status: 'needs_revision',
    criteria: [
      {
        criterionIndex: 0,
        satisfied: true,
        evidence: 'Already satisfied.',
        observationIds: ['222222'],
      },
      {
        criterionIndex: 1,
        satisfied: false,
        evidence: 'The target structure is unavailable.',
        observationIds: ['333333'],
      },
    ],
    result: null,
    revisionRequest: {
      goalId: second.id,
      invalidatedAssumption: 'The original structure remains available.',
      requestedEffect: 'Replace the target with a supported structure.',
    },
    reason: 'The plan must change.',
  };

  const replacementBefore = createNode('replacement');
  replacementBefore.index = 1;
  replacementBefore.status = 'pending';
  replacementBefore.bundle = null;
  replacementBefore.goal = 'Use the supported replacement.';
  const replacement = createNode('replacement', ['verify-revision']);
  replacement.goal = replacementBefore.goal;
  const active: Graph = { revision: 3, nodes: [replacement] };
  let currentObservationId = '';
  const provider = createProvider([], (request) => {
    const prompt = request.messages
      .filter(({ role }) => role === 'user')
      .map(({ content }) => (typeof content === 'string' ? content : ''))
      .join('\n');

    assert.match(prompt, /The original structure remains available\./u);
    assert.match(prompt, /Replace the target with a supported structure\./u);
    assert.match(prompt, /Old target must be replaced\./u);
    assert.match(prompt, /linked historical output/u);
    assert.doesNotMatch(
      prompt,
      /Already satisfied\.|unlinked historical output|Newer unrelated assumption/u,
    );
    assert.doesNotMatch(
      prompt,
      /222222|333333|call-unlinked-history|call-linked-history/u,
    );
    assert.match(prompt, /not citable evidence/u);
    assert.match(prompt, /cite only fresh observation IDs/u);

    if (provider.requests.length === 1) {
      const invalid = completed();
      invalid.criteria[0]!.observationIds = ['333333'];
      return terminalFinish(invalid)(request);
    }

    if (provider.requests.length === 2) {
      const correction = correctionMessageFrom(request);
      assert.match(correction, /Most similar valid observation ID: none\./u);
      assert.match(correction, /fresh local observation ID/u);
      assert.doesNotMatch(correction, /333333/u);
      return toolFinish('call-current-revision', 'verify-revision');
    }

    currentObservationId = observationIdFrom(request);
    const corrected = completed();
    corrected.criteria[0]!.observationIds = [currentObservationId];
    return terminalFinish(corrected)(request);
  });
  const harness = createHarness(active, provider, [
    tool('verify-revision', async () => ({ corrected: true })),
  ]);

  const action = await execution(
    state([
      { revision: 0, nodes: [createNode('p0')] },
      { revision: 1, nodes: [first, second] },
      { revision: 2, nodes: [first, replacementBefore] },
      active,
    ]),
    harness.context,
    handlers(),
  );

  assert.equal(action.type, 'transition');
  assert.equal(provider.requests.length, 3);
  assert.equal(replacement.status, 'completed');
  assert.match(currentObservationId, /^[0-9a-f]{6}$/u);
  assert.deepEqual(replacement.outcome?.criteria[0]?.observationIds, [
    currentObservationId,
  ]);
  assert.equal(replacement.observations[0]?.id, currentObservationId);
});

test('rejects a post-revision completion hook that does not cite its fresh observation', async () => {
  const prior = revisionTarget('prior');
  const current = createNode('prior');
  current.goal = 'Revised prior goal.';
  const active: Graph = { revision: 2, nodes: [current] };
  const decision = completed() as NodeDecision;
  const provider = createProvider([]);
  const harness = createHarness(active, provider);

  const action = await execution(
    state([{ revision: 1, nodes: [prior] }, active]),
    {
      ...harness.context,
      hooks: {
        execution: async () => ({
          decision,
          observations: [freshObservation(current.id, 'dddddd')],
        }),
      },
    },
    handlers(),
  );

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.match(
    (action.error as Error).message,
    /completed post-revision outcome must cite at least one fresh local observation ID/u,
  );
  assert.equal(current.status, 'running');
  assert.equal(current.outcome, null);
  assert.equal(current.observations.length, 1);
  assert.equal(provider.requests.length, 0);
});

test('accepts a post-revision completion hook that cites its fresh observation', async () => {
  const prior = revisionTarget('prior');
  const current = createNode('prior');
  current.goal = 'Revised prior goal.';
  const active: Graph = { revision: 2, nodes: [current] };
  const decision = completed() as NodeDecision;
  decision.criteria[0]!.observationIds = ['eeeeee'];
  const provider = createProvider([]);
  const harness = createHarness(active, provider);

  const action = await execution(
    state([{ revision: 1, nodes: [prior] }, active]),
    {
      ...harness.context,
      hooks: {
        execution: async () => ({
          decision,
          observations: [freshObservation(current.id, 'eeeeee')],
        }),
      },
    },
    handlers(),
  );

  assert.equal(action.type, 'transition');
  assert.equal(current.status, 'completed');
  assert.deepEqual(current.outcome?.criteria[0]?.observationIds, ['eeeeee']);
  assert.equal(current.observations[0]?.id, 'eeeeee');
  assert.equal(provider.requests.length, 0);
});

test('rejects a hook observation ID retained by an earlier snapshot', async () => {
  const prior = revisionTarget('prior');
  prior.observations = [freshObservation(prior.id, 'aaaaaa')];
  prior.outcome!.criteria[0]!.observationIds = ['aaaaaa'];
  const current = createNode('prior');
  current.goal = 'Revised prior goal.';
  const active: Graph = { revision: 2, nodes: [current] };
  const decision = completed() as NodeDecision;
  decision.criteria[0]!.observationIds = ['aaaaaa'];
  const provider = createProvider([]);
  const harness = createHarness(active, provider);

  const action = await execution(
    state([{ revision: 1, nodes: [prior] }, active]),
    {
      ...harness.context,
      hooks: {
        execution: async () => ({
          decision,
          observations: [freshObservation(current.id, 'aaaaaa')],
        }),
      },
    },
    handlers(),
  );

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.match((action.error as Error).message, /already reserved/u);
  assert.equal(current.outcome, null);
  assert.deepEqual(current.observations, []);
  assert.equal(provider.requests.length, 0);
});

test('allows tool-free blocked and failed hooks after a revision handoff', async () => {
  for (const status of ['blocked', 'failed'] as const) {
    const prior = revisionTarget(`prior-${status}`);
    const current = createNode(`prior-${status}`);
    current.goal = `Revised ${status} goal.`;
    const active: Graph = { revision: 2, nodes: [current] };
    const provider = createProvider([]);
    const harness = createHarness(active, provider);

    const action = await execution(
      state([{ revision: 1, nodes: [prior] }, active]),
      {
        ...harness.context,
        hooks: {
          execution: async () => ({
            decision: nonCompleted(status, `${status} after revision.`),
            observations: [],
          }),
        },
      },
      handlers(),
    );

    assert.equal(action.type, 'transition');
    assert.equal(current.status, status);
    assert.deepEqual(current.observations, []);
    assert.equal(provider.requests.length, 0);
  }
});

test('repairs a similar unauthorized observation ID before completing', async () => {
  const node = createNode('current', ['lookup']);
  const graph: Graph = { revision: 1, nodes: [node] };
  let authorizedId = '';
  let rejectedId = '';
  const provider = createProvider([], (request) => {
    if (provider.requests.length === 1) return toolFinish('call-1', 'lookup');

    authorizedId ||= observationIdFrom(request);
    rejectedId ||= `${authorizedId.slice(0, -1)}x`;
    if (provider.requests.length === 2) {
      const invalid = completed();
      invalid.criteria[0]!.observationIds = [rejectedId];
      return terminalFinish(invalid)(request);
    }

    const correction = correctionMessageFrom(request);
    assert.match(correction, /The observation ID is not authorized\./u);
    assert.match(
      correction,
      new RegExp(`Most similar valid observation ID: ${authorizedId}`, 'u'),
    );
    assert.doesNotMatch(correction, new RegExp(rejectedId, 'u'));
    const corrected = completed();
    corrected.criteria[0]!.observationIds = [authorizedId];
    return terminalFinish(corrected)(request);
  });
  const harness = createHarness(graph, provider, [
    tool('lookup', async () => ({ found: true })),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(provider.requests.length, 3);
  assert.equal(node.status, 'completed');
  assert.deepEqual(node.outcome?.criteria[0]?.observationIds, [authorizedId]);
});

test('repairs the Spring criterion 12 without accepting corruption at criteria 5 and 7', async () => {
  const node = createNode('spring', ['lookup']);
  node.doneWhen = Array.from(
    { length: 13 },
    (_, index) => `Spring migration criterion ${index}.`,
  );
  const graph: Graph = { revision: 1, nodes: [node] };
  let authorizedId = '';
  const provider = createProvider([], (request) => {
    if (provider.requests.length === 1) return toolFinish('call-1', 'lookup');

    authorizedId ||= observationIdFrom(request);
    const criteria = node.doneWhen.map((_, index) => ({
      criterionIndex: index,
      satisfied: true,
      evidence: `criterion-${index}-proof`,
      observationIds: index === 12 ? [authorizedId] : [],
    }));
    if (provider.requests.length === 2) {
      criteria[12]!.observationIds = [`${authorizedId.slice(0, -1)}x`];
    } else {
      criteria[5]!.criterionIndex = 50;
      criteria[7]!.satisfied = 'corrupted' as unknown as boolean;
    }
    return terminalFinish({
      status: 'completed',
      criteria,
      result: { markdown: 'Spring migration completed.', artifacts: [] },
      revisionRequest: null,
      reason: null,
    })(request);
  });
  const harness = createHarness(graph, provider, [tool('lookup')]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(provider.requests.length, 3);
  assert.equal(node.status, 'completed');
  assert.equal(node.outcome?.criteria[5]?.criterionIndex, 5);
  assert.equal(node.outcome?.criteria[7]?.satisfied, true);
  assert.deepEqual(node.outcome?.criteria[12]?.observationIds, [authorizedId]);
});

test('authorizes projected ancestors while excluding sibling observations', async () => {
  const ancestor = createNode('ancestor');
  ancestor.status = 'completed';
  ancestor.deliver = false;
  ancestor.observations = [
    {
      id: 'observation-ancestor',
      goalId: ancestor.id,
      toolName: 'inspect',
      callId: 'call-ancestor',
      input: '{}',
      output: '{"valid":true}',
    },
  ];
  const ancestorOutcome = completed() as NodeDecision;
  ancestorOutcome.criteria[0]!.observationIds = ['observation-ancestor'];
  ancestor.outcome = ancestorOutcome;

  const sibling = createNode('sibling');
  sibling.index = 1;
  sibling.status = 'completed';
  sibling.deliver = false;
  sibling.observations = [
    {
      id: 'observation-sibling',
      goalId: sibling.id,
      toolName: 'inspect',
      callId: 'call-sibling',
      input: '{}',
      output: '{"valid":false}',
    },
  ];
  const siblingOutcome = completed() as NodeDecision;
  siblingOutcome.criteria[0]!.observationIds = ['observation-sibling'];
  sibling.outcome = siblingOutcome;

  const node = createNode('current');
  node.index = 2;
  node.dependsOn = [ancestor.id];
  const graph: Graph = { revision: 1, nodes: [ancestor, sibling, node] };
  const invalid = completed();
  invalid.criteria[0]!.observationIds = ['observation-sibling'];
  const corrected = completed();
  corrected.criteria[0]!.observationIds = ['observation-ancestor'];
  const provider = createProvider([
    terminalFinish(invalid),
    (request) => {
      const correction = correctionMessageFrom(request);
      assert.match(
        correction,
        /Most similar valid observation ID: observation-ancestor/u,
      );
      assert.doesNotMatch(correction, /observation-sibling/u);
      return terminalFinish(corrected)(request);
    },
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(node.status, 'completed');
  assert.deepEqual(node.outcome?.criteria[0]?.observationIds, [
    'observation-ancestor',
  ]);
});

test('rejects an unauthorized observation ID returned by an execution hook', async () => {
  const node = createNode('current');
  const graph: Graph = { revision: 1, nodes: [node] };
  const decision = completed() as NodeDecision;
  decision.criteria[0]!.observationIds = ['ffffff'];
  const provider = createProvider([]);
  const harness = createHarness(graph, provider);

  const action = await execution(
    state([graph]),
    {
      ...harness.context,
      hooks: {
        execution: async () => ({
          decision,
          observations: [
            {
              id: 'eeeeee',
              goalId: node.id,
              toolName: 'lookup',
              callId: 'call-1',
              input: '{}',
              output: '{"found":true}',
            },
          ],
        }),
      },
    },
    handlers(),
  );

  assert.equal(action.type, 'fail');
  assert.equal(node.outcome, null);
  assert.equal(node.observations.length, 1);
  assert.deepEqual(node.artifacts, []);
  assert.notEqual(node.status, 'completed');
  assert.equal(provider.requests.length, 0);
});

test('automatically records every returned observation in tool-result order', async () => {
  const node = createNode('current', ['first', 'second']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolCallsFinish([
      { id: 'call-first', name: 'first', query: 'alpha' },
      { id: 'call-second', name: 'second', query: 'beta' },
    ]),
    terminalFinish(completed()),
  ]);
  const harness = createHarness(graph, provider, [
    tool('first', async ({ query }) => ({ value: `${String(query)} result` })),
    tool('second', async ({ query }) => ({ value: `${String(query)} result` })),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(node.status, 'completed');
  assert.deepEqual(
    node.observations.map(({ toolName, callId, input, output }) => ({
      toolName,
      callId,
      input,
      output,
    })),
    [
      {
        toolName: 'first',
        callId: 'call-first',
        input: '{"query":"alpha"}',
        output: '{"value":"alpha result"}',
      },
      {
        toolName: 'second',
        callId: 'call-second',
        input: '{"query":"beta"}',
        output: '{"value":"beta result"}',
      },
    ],
  );
});

test('blocks on turn exhaustion after retaining ordered observations without artifacts', async () => {
  const node = createNode('bounded', ['first', 'second']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolCallsFinish([
      { id: 'call-first', name: 'first', query: 'alpha' },
      { id: 'call-second', name: 'second', query: 'beta' },
    ]),
  ]);
  const harness = createHarness(
    graph,
    provider,
    [
      tool('first', async () => ({ value: 'first result' })),
      tool('second', async () => ({ value: 'second result' })),
    ],
    [],
    [],
    1,
  );

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(provider.requests.length, 1);
  assert.equal(node.status, 'blocked');
  assert.equal(node.outcome, null);
  assert.deepEqual(node.artifacts, []);
  assert.deepEqual(
    node.observations.map(({ toolName, callId, output }) => ({
      toolName,
      callId,
      output,
    })),
    [
      {
        toolName: 'first',
        callId: 'call-first',
        output: '{"value":"first result"}',
      },
      {
        toolName: 'second',
        callId: 'call-second',
        output: '{"value":"second result"}',
      },
    ],
  );
  assert.deepEqual(harness.logs.at(-1), {
    bindings: { nodeId: 'bounded', status: 'blocked' },
    message: 'node execution did not complete',
  });
});

test('resolves selected skill references without treating rationales as instructions', async () => {
  const node = createNode('current');
  node.candidates = [
    {
      skillName: 'selected',
      score: 1,
      rank: 1,
      rationale: 'private rationale',
    },
    {
      skillName: 'rejected',
      score: 0.5,
      rank: 2,
      rationale: 'private rejected rationale',
    },
  ];
  node.bundle = {
    goalId: 'current',
    skills: ['selected'],
    selectionRationale: 'private global rationale',
  };
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([terminalFinish(completed())]);
  const harness = createHarness(
    graph,
    provider,
    [],
    [skill('selected'), skill('rejected')],
    [skill('universal')],
  );

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  const system = provider.requests[0]?.messages[0]?.content ?? '';
  const user = provider.requests[0]?.messages
    .filter(({ role }) => role === 'user')
    .map(({ content }) => content ?? '')
    .join('\n');
  assert.equal(typeof system, 'string');
  assert.equal(typeof user, 'string');
  if (typeof system !== 'string' || typeof user !== 'string') return;
  assert.match(system, /universal body/u);
  assert.match(user, /selected body/u);
  assert.doesNotMatch(
    user,
    /rejected body|private rationale|private rejected rationale|private global rationale/u,
  );
});

test('preserves each semantic terminal status and resolves the wave', async () => {
  for (const status of ['blocked', 'failed'] as const) {
    const node = createNode(`node-${status}`);
    const graph: Graph = { revision: 1, nodes: [node] };
    const provider = createProvider([
      terminalFinish(nonCompleted(status, `${status} private reason`)),
    ]);
    const harness = createHarness(graph, provider);

    const action = await execution(state([graph]), harness.context, handlers());

    assert.equal(action.type, 'transition');
    if (action.type !== 'transition') continue;
    assert.equal(node.status, status);
    assert.equal(node.outcome?.status, status);
    assert.equal(node.outcome?.reason, `${status} private reason`);
    assert.deepEqual(node.artifacts, []);
  }
});

test('stores needs_revision with every node observation and does not promote a partial result', async () => {
  const node = createNode('current', ['first', 'middle', 'last']);
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    toolCallsFinish([
      { id: 'call-first', name: 'first', query: 'first' },
      { id: 'call-middle', name: 'middle', query: 'invalidating' },
      { id: 'call-last', name: 'last', query: 'last' },
    ]),
    terminalFinish({
      ...nonCompleted('needs_revision', 'Revision required.'),
      result: { markdown: 'Unpromoted partial result.', artifacts: [] },
      revisionRequest: {
        goalId: 'current',
        invalidatedAssumption: 'The resource exists.',
        requestedEffect: 'Replace the resource-dependent goal.',
      },
    }),
  ]);
  const harness = createHarness(graph, provider, [
    tool('first', async () => ({ stage: 'context' })),
    tool('middle', async () => ({ exists: false })),
    tool('last', async () => ({ stage: 'confirmation' })),
  ]);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  if (action.type !== 'transition') return;
  assert.equal(action.handler, 'schedule');
  assert.equal(node.status, 'needs_revision');
  assert.deepEqual(node.artifacts, []);
  assert.deepEqual(
    node.observations.map(({ toolName, output }) => ({
      toolName,
      output,
    })),
    [
      { toolName: 'first', output: '{"stage":"context"}' },
      { toolName: 'middle', output: '{"exists":false}' },
      { toolName: 'last', output: '{"stage":"confirmation"}' },
    ],
  );
  assert.equal(node.outcome?.revisionRequest?.goalId, 'current');
  assert.equal(node.observations.length, 3);
});

test('fails needs_revision when the node produced no observation', async () => {
  const node = createNode('current');
  const graph: Graph = { revision: 1, nodes: [node] };
  const provider = createProvider([
    terminalFinish({
      ...nonCompleted('needs_revision', 'Revision required.'),
      revisionRequest: {
        goalId: 'current',
        invalidatedAssumption: 'The plan assumption is invalid.',
        requestedEffect: 'Revise the plan.',
      },
    }),
  ]);
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'fail');
  if (action.type !== 'fail') return;
  assert.equal(node.status, 'running');
  assert.match(
    (action.error as Error).message,
    /requires a local observation/u,
  );
});

test('propagates provider and tool failures with their exact identity', async () => {
  const providerError = new Error('schema rejected');
  const providerNode = createNode('provider');
  const providerGraph: Graph = { revision: 1, nodes: [providerNode] };
  const provider = createProvider([providerError]);
  const providerHarness = createHarness(providerGraph, provider);

  const providerAction = await execution(
    state([providerGraph]),
    providerHarness.context,
    handlers(),
  );

  assert.equal(providerAction.type, 'fail');
  if (providerAction.type === 'fail') {
    assert.strictEqual(providerAction.error, providerError);
  }
  assert.equal(providerNode.status, 'running');

  const toolNode = createNode('tool', ['lookup']);
  const toolGraph: Graph = { revision: 1, nodes: [toolNode] };
  const toolProvider = createProvider([toolFinish('call-failure', 'lookup')]);
  const toolError = new Error('tool payload detail');
  const toolHarness = createHarness(toolGraph, toolProvider, [
    tool('lookup', async () => {
      throw toolError;
    }),
  ]);

  const toolAction = await execution(
    state([toolGraph]),
    toolHarness.context,
    handlers(),
  );

  assert.equal(toolAction.type, 'fail');
  if (toolAction.type === 'fail') {
    assert.strictEqual((toolAction.error as Error).cause, toolError);
  }
  assert.equal(toolNode.status, 'running');
});

test('waits for the whole concurrent wave and resolves semantic outcomes', async () => {
  const completedNode = createNode('completed');
  const blockedNode = createNode('blocked');
  const graph: Graph = { revision: 1, nodes: [completedNode, blockedNode] };
  let completionIndex = 0;
  const provider = createProvider([], (request) => {
    const current = completionIndex++;
    const terminal =
      current === 0
        ? terminalFinish(completed())
        : terminalFinish(nonCompleted('blocked', 'No useful action.'));
    return terminal(request);
  });
  const harness = createHarness(graph, provider);

  const action = await execution(state([graph]), harness.context, handlers());

  assert.equal(action.type, 'transition');
  assert.equal(completedNode.status, 'completed');
  assert.equal(blockedNode.status, 'blocked');
});

type Step =
  | ProviderFinished<unknown>
  | Error
  | ((request: ProviderRequest<unknown>) => ProviderFinished<unknown>);

function createProvider(
  steps: Step[],
  complete?: (request: ProviderRequest<unknown>) => ProviderFinished<unknown>,
) {
  const requests: ProviderRequest<unknown>[] = [];
  let index = 0;

  const provider = {
    metadata: {
      id: 'fake',
      name: 'Fake',
      baseUrl: 'https://fake.invalid',
    },
    complete: async (request: ProviderRequest<unknown>) => {
      requests.push(request);
      const selected = complete?.(request) ?? steps[index++];
      const step =
        typeof selected === 'function' ? selected(request) : selected;
      if (step instanceof Error) throw step;
      if (step === undefined) throw new Error('Missing fake provider step.');
      return step;
    },
  } as unknown as LlmProvider;

  return { provider, requests };
}

function createHarness(
  graph: Graph,
  fake: ReturnType<typeof createProvider>,
  tools: readonly Tool[] = [],
  skills: readonly Skill[] = [],
  requiredSkills: readonly Skill[] = [],
  maxTurns = 8,
) {
  const logs: unknown[] = [];
  const context: WorkflowContext = {
    input: 'Complete the request.',
    options: {
      logger: {
        info: (bindings: unknown, message: string) =>
          logs.push({ bindings, message }),
      } as never,
      providers: mosaicProviders(fake.provider),
      models: {
        planning: { model: 'default-model', effort: 'low' },
        revision: { model: 'default-model', effort: 'low' },
        execution: { model: 'default-model', effort: 'low' },
        reranker: 'reranker-model',
        embedder: 'embedder-model',
      },
      routing: {
        maxHintCandidates: 5,
        maxRetrievedCandidates: 5,
        maxSkills: 5,
      },
      execution: { maxTurns },
      revision: { max: 3 },
      skills: {
        required: requiredSkills,
        menu: [...requiredSkills, ...skills],
        retriever: {} as never,
      },
      tools: {
        required: [],
        menu: tools,
        retriever: {} as never,
      },
    },
  };

  void graph;
  return { context, logs };
}

function skill(name: string): Skill {
  return {
    name,
    description: `${name} description`,
    body: `${name} body`,
    allowedTools: [],
    indexText: `${name} | ${name} description |  | ${name} body`,
  };
}

function state(graphs: Graph[]): WorkflowState {
  return { graphs };
}

function handlers() {
  return {
    transition: (handler: string, state: WorkflowState) => ({
      type: 'transition' as const,
      handler,
      state,
    }),
    finish: () => ({ type: 'finish' as const, value: undefined }),
    fail: (error: unknown) => ({ type: 'fail' as const, error }),
  } as never;
}

function createNode(id: string, toolNames: readonly string[] = []): Node {
  return {
    id,
    goal: `Goal ${id}`,
    doneWhen: [`${id} is complete.`],
    dependsOn: [],
    status: 'ready',
    deliver: true,
    index: 0,
    candidates: [],
    bundle: {
      goalId: id,
      skills: [],
      selectionRationale: 'No skills are needed.',
    },
    tools: toolNames.map((name) => ({ name, description: `${name} tool` })),
    artifacts: [],
    observations: [],
    outcome: null,
    termination: null,
  };
}

function revisionTarget(id: string): Node {
  const target = createNode(id);
  target.status = 'needs_revision';
  target.outcome = {
    ...nonCompleted('needs_revision', 'Revision required.'),
    revisionRequest: {
      goalId: id,
      invalidatedAssumption: 'The old structure remains valid.',
      requestedEffect: 'Use a supported replacement.',
    },
  };
  return target;
}

function freshObservation(goalId: string, id: string) {
  return {
    id,
    goalId,
    toolName: 'inspect',
    callId: `call-${id}`,
    input: '{}',
    output: '{"corrected":true}',
  };
}

function completed(overrides: Record<string, unknown> = {}) {
  return {
    status: 'completed',
    criteria: [
      {
        criterionIndex: 0,
        satisfied: true,
        evidence: 'Criterion met.',
        observationIds: [] as string[],
      },
    ],
    result: { markdown: 'Completed.', artifacts: [] },
    revisionRequest: null,
    reason: null,
    ...overrides,
  };
}

function nonCompleted(
  status: 'needs_revision' | 'blocked' | 'failed',
  reason: string,
) {
  return {
    status,
    criteria: [
      {
        criterionIndex: 0,
        satisfied: false,
        evidence: 'Criterion unmet.',
        observationIds: [],
      },
    ],
    result: null,
    revisionRequest: null,
    reason,
  };
}

function toolFinish(id: string, name: string): ProviderFinished<unknown> {
  return toolCallsFinish([{ id, name, query: 'evidence' }]);
}

function toolCallsFinish(
  calls: readonly {
    readonly id: string;
    readonly name: string;
    readonly query: string;
  }[],
): ProviderFinished<unknown> {
  return {
    text: '',
    finishReason: 'tool_calls',
    toolCalls: calls.map(({ id, name, query }) => ({
      id,
      name,
      arguments: JSON.stringify({ query }),
    })),
  };
}

function terminalFinish(
  value: unknown,
): (request: ProviderRequest<unknown>) => ProviderFinished<unknown> {
  return (request) => {
    const terminal = request.tools?.find(
      ({ description }) =>
        description ===
        'Submit the final structured output and end the agent run.',
    );

    assert.ok(terminal);
    return {
      text: '',
      finishReason: 'tool_calls',
      toolCalls: [
        {
          id: 'call-structured-output',
          name: terminal.name,
          arguments: JSON.stringify(value),
        },
      ],
    };
  };
}

function observationIdFrom(request: ProviderRequest<unknown>): string {
  const content = request.messages
    .filter(({ role }) => role === 'tool')
    .map(({ content }) => (typeof content === 'string' ? content : ''))
    .join('\n');
  const match = content.match(/\b[0-9a-f]{6}\b/u);
  assert.ok(match);
  return match[0];
}

function correctionMessageFrom(request: ProviderRequest<unknown>): string {
  const correction = request.messages
    .filter(({ role }) => role === 'system')
    .map(({ content }) => (typeof content === 'string' ? content : ''))
    .find((content) => content.startsWith('# Structured output correction'));
  assert.ok(correction);
  return correction;
}

function tool(
  name: string,
  execute: Tool['execute'] = async () => ({ found: true }),
): Tool {
  const input = z.object({ query: z.string() });
  const output = z.unknown();

  return {
    name,
    description: `${name} tool`,
    input,
    output,
    definition: {
      name,
      description: `${name} tool`,
      inputSchema: {
        type: 'object',
        properties: { query: { type: 'string' } },
        required: ['query'],
        additionalProperties: false,
      },
      outputSchema: {},
      strict: true,
    },
    execute,
  };
}
