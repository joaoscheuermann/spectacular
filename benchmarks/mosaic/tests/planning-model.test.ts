import assert from 'node:assert/strict';
import test from 'node:test';

import type { ProviderRequest } from 'llms';

import {
  createPlanningModelAdapter,
  planningCase,
  type PlanningGraph,
  PlanningGraphSchema,
  type PlanningModelEvent,
  type PlanningObservation,
  PlanningObservationSchema,
  retrievePlanningSkills,
} from '../src/composition/planning.js';
import { fakeProvider, finish } from './support/provider.js';

test('retrieves every positive lexical match without oracle fields or a gold-sized cutoff', () => {
  const graph = PlanningGraphSchema.parse({
    nodes: [
      {
        id: 'n01:analyze_alpha_beta',
        goal: 'Analyze alpha, beta, and gamma evidence.',
        doneWhen: ['The alpha beta gamma analysis is complete.'],
        dependsOn: [],
        deliver: true,
      },
    ],
  });

  const source = {
    request: 'Compare alpha, beta, and gamma.',
    catalog: [
      {
        id: 'skill.alpha',
        name: 'alpha-analysis',
        description: 'Analyzes alpha evidence.',
        body: 'Inspect alpha observations.',
        toolIds: [],
      },
      {
        id: 'skill.beta',
        name: 'beta-analysis',
        description: 'Analyzes beta evidence.',
        body: 'Inspect beta observations.',
        toolIds: [],
      },
      {
        id: 'skill.gamma',
        name: 'gamma-analysis',
        description: 'Analyzes gamma evidence.',
        body: 'Inspect gamma observations.',
        toolIds: [],
      },
      {
        id: 'skill.delta',
        name: 'delta-forecasting',
        description: 'Forecasts delta values.',
        body: 'Project a future delta series.',
        toolIds: [],
      },
    ],
  };
  const first = retrievePlanningSkills(source, graph);
  const second = retrievePlanningSkills(source, graph);

  assert.deepEqual(second, first);

  assert.deepEqual(
    new Set(first.map(({ skillId }) => skillId)),
    new Set(['skill.alpha', 'skill.beta', 'skill.gamma']),
  );

  assert.ok(
    first.every(
      ({ score, matchedTerms }) => score > 0 && matchedTerms.length > 0,
    ),
  );
});

test('runs planning, revision, and observation through explicit structured agent calls', async () => {
  const benchmarkCase = planningCase('planning.software.c');

  const p0: PlanningGraph = {
    nodes: [
      {
        id: 'n01:assess_api_contract',
        goal: 'Assess the supplied API contract change.',
        doneWhen: [
          'The changed input and missing-value behavior are identified.',
        ],
        dependsOn: [],
        deliver: true,
      },
    ],
  };

  const p1: PlanningGraph = {
    nodes: [
      {
        ...p0.nodes[0],
        doneWhen: [
          ...p0.nodes[0].doneWhen,
          'Compatibility is compared at each affected call-site boundary.',
        ],
      },
    ],
  };

  const observation: PlanningObservation = {
    nodes: [
      {
        id: 'n01:assess_api_contract',
        roleIds: ['role.assess-compatibility'],
        outputIds: ['output.compatibility-assessment'],
        behaviorIds: [
          'software.identify-contract-change',
          'software.check-compatibility',
        ],
        dependsOn: [],
      },
    ],
  };
  const outputs: readonly unknown[] = [p0, p1, observation];
  let outputIndex = 0;

  const fake = fakeProvider((request) => {
    const output = outputs[outputIndex++];

    assert.notEqual(output, undefined);

    return structuredFinish(request, output, outputIndex);
  });
  const events: PlanningModelEvent[] = [];

  const adapter = createPlanningModelAdapter({
    provider: fake.provider,
    model: 'offline-model',
    effort: 'none',
    maxTurns: 3,
    onEvent: (event) => {
      events.push(event);
    },
  });

  const evidence = benchmarkCase.catalog.filter(
    ({ id }) => id === 'skill.compatibility-analysis',
  );

  assert.deepEqual(await adapter.initialPlan(benchmarkCase), p0);

  assert.deepEqual(
    await adapter.revise({
      case: benchmarkCase,
      condition: 'retrieved',
      p0,
      evidence,
    }),
    p1,
  );

  assert.deepEqual(
    await adapter.observe({
      case: benchmarkCase,
      phase: 'p1',
      condition: 'retrieved',
      graph: p1,
    }),
    observation,
  );

  const retrieved = await adapter.retrieve({ case: benchmarkCase, p0 });

  assert.ok(retrieved.includes('skill.compatibility-analysis'));

  assert.equal(fake.requests.length, 3);

  assert.equal(outputIndex, 3);

  assert.ok(fake.requests.every(({ schema }) => schema === undefined));

  assert.ok(fake.requests.every(({ tools }) => tools?.length === 1));

  const prompts = fake.requests.map(userPrompt);
  const initialMessages = requestText(fake.requests[0]);
  const revisionMessages = requestText(fake.requests[1]);

  assert.ok(prompts.every((prompt) => prompt.startsWith('#')));

  assert.ok(prompts.every((prompt) => !prompt.trimStart().startsWith('{')));

  assert.ok(!initialMessages.includes('compatibility-analysis'));

  assert.ok(!initialMessages.includes('role.assess-compatibility'));

  assert.ok(!initialMessages.includes('output.compatibility-assessment'));

  assert.ok(!initialMessages.includes('software.check-compatibility'));

  assert.ok(prompts[1].includes('# Skill evidence'));

  assert.ok(prompts[1].includes(evidence[0].body));

  assert.ok(!revisionMessages.includes('retrieved'));

  assert.ok(!revisionMessages.includes('behaviorIds'));

  assert.ok(!revisionMessages.includes('relevance'));

  assert.ok(!revisionMessages.includes('role.assess-compatibility'));

  assert.ok(!revisionMessages.includes('output.compatibility-assessment'));

  assert.ok(!revisionMessages.includes('software.check-compatibility'));

  assert.ok(prompts[2].includes('# Semantic label catalog'));

  assert.ok(prompts[2].includes('role.assess-compatibility'));

  assert.equal(events.filter(({ type }) => type === 'model.call').length, 3);

  assert.equal(
    events.filter(({ type }) => type === 'model.completed').length,
    3,
  );

  assert.equal(
    events.filter(({ type }) => type === 'retrieval.completed').length,
    1,
  );

  assert.deepEqual(await adapter.initialPlan(benchmarkCase), p0);

  assert.deepEqual(
    await adapter.observe({
      case: benchmarkCase,
      phase: 'p1',
      condition: 'gold',
      graph: p1,
    }),
    observation,
  );

  assert.equal(fake.requests.length, 3);

  assert.deepEqual(
    events.flatMap((event) =>
      event.type === 'cache.hit' ? [event.operation] : [],
    ),
    ['initial-plan', 'observation'],
  );
});

test('reports every structured repair provider call', async () => {
  const benchmarkCase = planningCase('planning.artifacts.a');

  const valid: PlanningGraph = {
    nodes: [
      {
        id: 'n01:write_summary',
        goal: 'Write the requested summary.',
        doneWhen: ['The summary contains the supplied facts.'],
        dependsOn: [],
        deliver: true,
      },
    ],
  };
  let attempt = 0;

  const fake = fakeProvider((request) => {
    attempt += 1;

    return structuredFinish(
      request,
      attempt === 1 ? { nodes: [] } : valid,
      attempt,
    );
  });
  const events: PlanningModelEvent[] = [];

  const adapter = createPlanningModelAdapter({
    provider: fake.provider,
    model: 'offline-model',
    maxTurns: 3,
    onEvent: (event) => {
      events.push(event);
    },
  });

  assert.deepEqual(await adapter.initialPlan(benchmarkCase), valid);

  assert.equal(fake.requests.length, 2);

  assert.deepEqual(
    events.flatMap((event) =>
      event.type === 'model.call' ? [event.call] : [],
    ),
    [1, 2],
  );

  assert.deepEqual(
    events.flatMap((event) =>
      event.type === 'structured.attempt' ? [event.runtimeAccepted] : [],
    ),
    [false, true],
  );

  const completed = events.find(({ type }) => type === 'model.completed');

  assert.equal(completed?.type, 'model.completed');

  if (completed?.type === 'model.completed') {assert.equal(completed.calls, 2);}
});

test('repairs an observation that uses labels outside the case catalog', async () => {
  const benchmarkCase = planningCase('planning.software.c');

  const graph: PlanningGraph = {
    nodes: [
      {
        id: 'n01:assess_api_contract',
        goal: 'Assess the supplied API contract change.',
        doneWhen: [
          'The changed input and missing-value behavior are identified.',
        ],
        dependsOn: [],
        deliver: true,
      },
    ],
  };

  const valid: PlanningObservation = {
    nodes: [
      {
        id: graph.nodes[0].id,
        roleIds: ['role.assess-compatibility'],
        outputIds: ['output.compatibility-assessment'],
        behaviorIds: ['software.identify-contract-change'],
        dependsOn: [],
      },
    ],
  };
  let attempt = 0;

  const fake = fakeProvider((request) => {
    attempt += 1;

    return structuredFinish(
      request,
      attempt === 1
        ? {
            nodes: [
              {
                ...valid.nodes[0],
                roleIds: ['role.invented'],
              },
            ],
          }
        : valid,
      attempt,
    );
  });
  const events: PlanningModelEvent[] = [];

  const adapter = createPlanningModelAdapter({
    provider: fake.provider,
    model: 'offline-model',
    maxTurns: 3,
    onEvent: (event) => {
      events.push(event);
    },
  });

  assert.deepEqual(
    await adapter.observe({
      case: benchmarkCase,
      phase: 'p0',
      condition: null,
      graph,
    }),
    valid,
  );

  assert.equal(fake.requests.length, 2);

  assert.deepEqual(
    events.flatMap((event) =>
      event.type === 'structured.attempt' ? [event.runtimeAccepted] : [],
    ),
    [false, true],
  );
});

test('accepts underscore-bearing Mosaic node IDs in semantic observations', () => {
  const observation = PlanningObservationSchema.parse({
    nodes: [
      {
        id: 'n01:inspect_api_contract',
        roleIds: ['role.inspect'],
        outputIds: [],
        behaviorIds: [],
        dependsOn: [],
      },
      {
        id: 'n02:report_api_result',
        roleIds: ['role.report'],
        outputIds: [],
        behaviorIds: [],
        dependsOn: ['n01:inspect_api_contract'],
      },
    ],
  });

  assert.equal(observation.nodes[0].id, 'n01:inspect_api_contract');

  assert.deepEqual(observation.nodes[1].dependsOn, [
    'n01:inspect_api_contract',
  ]);
});

const structuredFinish = (
  request: ProviderRequest<unknown>,
  value: unknown,
  index: number,
) => {
  const terminal = request.tools?.find(
    ({ description }) =>
      description ===
      'Submit the final structured output and end the agent run.',
  );

  assert.ok(terminal);

  return finish('', [
    {
      id: `call-${index}`,
      name: terminal.name,
      arguments: JSON.stringify(value),
    },
  ]);
};

const userPrompt = (request: ProviderRequest<unknown>): string => {
  const message = [...request.messages]
    .reverse()
    .find(({ role }) => role === 'user');

  assert.equal(typeof message?.content, 'string');

  return message!.content as string;
};

const requestText = (request: ProviderRequest<unknown>): string =>
  request.messages
    .flatMap(({ content }) => (typeof content === 'string' ? [content] : []))
    .join('\n');
