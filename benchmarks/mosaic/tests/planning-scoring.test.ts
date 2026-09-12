import assert from 'node:assert/strict';
import test from 'node:test';

import {
  planningCase,
  planningCases,
  planningGoldObservation,
  planningGraphFromObservation,
  scorePlanning,
  scorePlanningTransition,
} from '../src/composition/planning.js';

test('scores every canonical P0 and P1 observation as complete', () => {
  for (const benchmarkCase of planningCases) {
    const p0 = planningGoldObservation(benchmarkCase, 'p0');
    const p1 = planningGoldObservation(benchmarkCase, 'p1');

    assert.equal(scorePlanning(benchmarkCase, 'p0', p0).score, 1);

    assert.equal(scorePlanning(benchmarkCase, 'p1', p1).score, 1);

    const transition = scorePlanningTransition(benchmarkCase, p0, p1);

    assert.equal(transition.passed, true, benchmarkCase.id);

    assert.deepEqual(transition.regressions, [], benchmarkCase.id);

    assert.equal(
      transition.gain > 0,
      !['A', 'B'].includes(benchmarkCase.compositionClass),
      benchmarkCase.id,
    );
  }
});

test('reports a missing catalog behavior as an explicit P1 failure', () => {
  const benchmarkCase = planningCase('planning.software.c');
  const p0 = planningGoldObservation(benchmarkCase, 'p0');
  const p1 = planningGoldObservation(benchmarkCase, 'p1');
  const missing = 'software.check-compatibility';

  const incomplete = {
    nodes: p1.nodes.map((node) => ({
      ...node,
      behaviorIds: node.behaviorIds.filter((id) => id !== missing),
    })),
  };
  const transition = scorePlanningTransition(benchmarkCase, p0, incomplete);

  assert.equal(transition.p1.passed, false);

  assert.deepEqual(
    transition.p1.criteria.filter(({ passed }) => !passed).map(({ id }) => id),
    [`behavior:role.assess-compatibility:${missing}`],
  );
});

test('accepts a required role dependency through a transitive path', () => {
  const benchmarkCase = planningCase('planning.documents-finance.f');
  const p1 = planningGoldObservation(benchmarkCase, 'p1');

  const gather = p1.nodes.find((node) =>
    node.roleIds.includes('role.gather-sources'),
  )!;

  const report = p1.nodes.find((node) =>
    node.roleIds.includes('role.publish-financial-report'),
  )!;

  const transitive = {
    nodes: p1.nodes.map((node) =>
      node.id === report.id
        ? {
            ...node,
            dependsOn: node.dependsOn.filter((id) => id !== gather.id),
          }
        : node,
    ),
  };
  const score = scorePlanning(benchmarkCase, 'p1', transitive);

  assert.equal(score.passed, true);

  assert.equal(
    score.criteria.find(
      ({ id }) => id === 'dependency:dependency.sources-before-report',
    )?.passed,
    true,
  );
});

test('rejects semantic observations with IDs outside the case gold', () => {
  const benchmarkCase = planningCase('planning.artifacts.a');
  const observation = planningGoldObservation(benchmarkCase, 'p0');

  const invalid = {
    nodes: observation.nodes.map((node, index) =>
      index === 0 ? { ...node, behaviorIds: ['artifact.unknown'] } : node,
    ),
  };

  assert.throws(
    () => scorePlanning(benchmarkCase, 'p0', invalid),
    /unknown behavior ID/u,
  );
});

test('converts adjudicated observations into valid terminal plans', () => {
  const benchmarkCase = planningCase('planning.communications.f');
  const observation = planningGoldObservation(benchmarkCase, 'p1');
  const graph = planningGraphFromObservation(benchmarkCase, observation);
  const dependedOn = new Set(graph.nodes.flatMap(({ dependsOn }) => dependsOn));

  assert.equal(graph.nodes.length, observation.nodes.length);

  assert.ok(graph.nodes.some(({ deliver }) => deliver));

  assert.ok(
    graph.nodes.every(({ id, deliver }) => deliver === !dependedOn.has(id)),
  );
});
