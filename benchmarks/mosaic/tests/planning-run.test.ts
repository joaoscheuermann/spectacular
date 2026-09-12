import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { ProviderRequest } from 'llms';

import {
  planningCase,
  planningGoldObservation,
  planningGraphFromObservation,
} from '../src/composition/planning.js';
import { runPlanningBenchmark } from '../src/composition/planning-run.js';
import { fakeProvider, finish } from './support/provider.js';

test('persists one paid controlled case with its closed identity and traces', async () => {
  const benchmarkCase = planningCase('planning.artifacts.a');

  const p0 = planningGraphFromObservation(
    benchmarkCase,
    planningGoldObservation(benchmarkCase, 'p0'),
  );
  const observation = planningGoldObservation(benchmarkCase, 'p0');
  let responseIndex = 0;

  const fake = fakeProvider((request) =>
    structuredFinish(
      request,
      request.messages.some(
        ({ content }) =>
          typeof content === 'string' &&
          content.includes('Classify the semantic content of a plan'),
      )
        ? observation
        : p0,
      responseIndex++,
    ),
  );
  const root = await mkdtemp(join(tmpdir(), 'planning-run-'));
  const outputDir = join(root, 'campaign');
  const progress: { readonly type: string }[] = [];

  try {
    const options = {
      profile: { provider: fake.provider, model: 'offline-model' },
      outputDir,
      caseIds: [benchmarkCase.id],
      maxTurns: 3,
      progress: (event: { readonly type: string }) => {
        progress.push(event);
      },
    };
    const summary = await runPlanningBenchmark(options);

    const manifest = JSON.parse(
      await readFile(join(outputDir, 'run.json'), 'utf8'),
    ) as { readonly judge: string; readonly caseIds: readonly string[] };

    assert.equal(summary.caseCount, 1);

    assert.ok(summary.modelCallCount >= 3);

    assert.deepEqual(manifest.caseIds, [benchmarkCase.id]);

    assert.equal(manifest.judge, 'same-model-rubric-observation');

    assert.equal(summary.metrics.conditions.length, 4);

    assert.equal(progress[0]?.type, 'run.started');

    assert.ok(progress.some(({ type }) => type === 'case.started'));

    assert.ok(progress.some(({ type }) => type === 'model.call'));

    assert.ok(progress.some(({ type }) => type === 'structured.attempt'));

    assert.ok(progress.some(({ type }) => type === 'case.completed'));

    assert.equal(progress.at(-1)?.type, 'run.completed');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects unknown cases before creating output', async () => {
  const root = await mkdtemp(join(tmpdir(), 'planning-run-invalid-'));
  const fake = fakeProvider();

  try {
    await assert.rejects(
      runPlanningBenchmark({
        profile: { provider: fake.provider, model: 'offline-model' },
        outputDir: join(root, 'campaign'),
        caseIds: ['unknown'],
      }),
      /Unknown planning case/,
    );

    assert.equal(fake.requests.length, 0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
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
