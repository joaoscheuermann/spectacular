import {
  PlanningCaseSchema,
  PlanningGraphSchema,
  PlanningObservationSchema,
  planningPhaseAtoms,
  type PlanningCase,
  type PlanningGraph,
  type PlanningObservation,
  type PlanningPhaseCriteria,
} from './planning-schema.js';

export type PlanningPhase = 'p0' | 'p1';

export interface PlanningCriterionResult {
  readonly id: string;
  readonly kind:
    | 'role'
    | 'output'
    | 'behavior'
    | 'dependency'
    | 'forbidden-behavior'
    | 'node-count';
  readonly passed: boolean;
}

export interface PlanningScore {
  readonly phase: PlanningPhase;
  readonly passed: boolean;
  readonly score: number;
  readonly passedCount: number;
  readonly totalCount: number;
  readonly criteria: readonly PlanningCriterionResult[];
}

export interface PlanningTransitionScore {
  readonly p0: PlanningScore;
  readonly p1: PlanningScore;
  readonly p0AgainstP1: PlanningScore;
  readonly p1Retention: PlanningScore;
  readonly gain: number;
  readonly regressions: readonly string[];
  readonly passed: boolean;
}

/** Scores one adjudicated semantic observation against a case phase. */
export const scorePlanning = (
  benchmarkCase: PlanningCase,
  phase: PlanningPhase,
  observation: PlanningObservation,
): PlanningScore => {
  const current = PlanningCaseSchema.parse(benchmarkCase);
  const observed = validateObservation(current, observation);
  return scoreCriteria(phase, current, current.criteria[phase], observed);
};

/** Compares P0 and P1 while making gains and regressions separately visible. */
export const scorePlanningTransition = (
  benchmarkCase: PlanningCase,
  p0: PlanningObservation,
  p1: PlanningObservation,
): PlanningTransitionScore => {
  const current = PlanningCaseSchema.parse(benchmarkCase);
  const observedP0 = validateObservation(current, p0);
  const observedP1 = validateObservation(current, p1);
  const p0Score = scoreCriteria('p0', current, current.criteria.p0, observedP0);
  const p1Score = scoreCriteria('p1', current, current.criteria.p1, observedP1);
  const p0AgainstP1 = scoreCriteria(
    'p1',
    current,
    current.criteria.p1,
    observedP0,
  );
  const p1Retention = scoreCriteria(
    'p0',
    current,
    current.criteria.p0,
    observedP1,
  );
  const retained = new Map(
    p1Retention.criteria.map((criterion) => [criterion.id, criterion.passed]),
  );
  const regressions = p0Score.criteria
    .filter(({ id, passed }) => passed && retained.get(id) !== true)
    .map(({ id }) => id);

  return {
    p0: p0Score,
    p1: p1Score,
    p0AgainstP1,
    p1Retention,
    gain: rounded(p1Score.score - p0AgainstP1.score),
    regressions,
    passed: p0Score.passed && p1Score.passed && regressions.length === 0,
  };
};

/** Creates the canonical oracle observation for tests and diagnostic ceilings. */
export const planningGoldObservation = (
  benchmarkCase: PlanningCase,
  phase: PlanningPhase,
): PlanningObservation => {
  const current = PlanningCaseSchema.parse(benchmarkCase);
  const criteria = current.criteria[phase];
  const nodes = criteria.roles.map((role, index) => ({
    id: `n${String(index + 1).padStart(2, '0')}`,
    roleIds: [role.roleId],
    outputIds: [...role.outputIds],
    behaviorIds: [...role.behaviorIds],
    dependsOn: [] as string[],
  }));
  const byRole = new Map(
    nodes.flatMap((node) =>
      node.roleIds.map((roleId) => [roleId, node] as const),
    ),
  );
  const dependencies = new Map(
    current.gold.dependencies.map((dependency) => [dependency.id, dependency]),
  );
  criteria.dependencyIds.forEach((id) => {
    const dependency = dependencies.get(id);
    if (dependency === undefined) return;
    const before = byRole.get(dependency.beforeRoleId);
    const after = byRole.get(dependency.afterRoleId);
    if (
      before === undefined ||
      after === undefined ||
      before.id === after.id ||
      after.dependsOn.includes(before.id)
    )
      return;
    after.dependsOn.push(before.id);
  });
  return PlanningObservationSchema.parse({ nodes });
};

/** Converts an adjudicated observation into a valid plan for offline hook runs. */
export const planningGraphFromObservation = (
  benchmarkCase: PlanningCase,
  observation: PlanningObservation,
): PlanningGraph => {
  const current = PlanningCaseSchema.parse(benchmarkCase);
  const observed = validateObservation(current, observation);
  const roles = new Map(current.gold.roles.map((role) => [role.id, role]));
  const outputs = new Map(
    current.gold.outputs.map((output) => [output.id, output.description]),
  );
  const behaviors = new Map(
    current.gold.behaviors.map((behavior) => [
      behavior.id,
      behavior.description,
    ]),
  );
  const dependedOn = new Set(
    observed.nodes.flatMap(({ dependsOn }) => dependsOn),
  );

  return PlanningGraphSchema.parse({
    nodes: observed.nodes.map((node) => {
      const roleDescriptions = node.roleIds.map(
        (id) => roles.get(id)!.description,
      );
      const outputDescriptions = node.outputIds.map((id) => outputs.get(id)!);
      const behaviorDescriptions = node.behaviorIds.map(
        (id) => behaviors.get(id)!,
      );
      return {
        id: node.id,
        goal: [...roleDescriptions, ...outputDescriptions].join(' '),
        doneWhen:
          behaviorDescriptions.length > 0
            ? behaviorDescriptions
            : outputDescriptions,
        dependsOn: [...node.dependsOn],
        deliver: !dependedOn.has(node.id),
      };
    }),
  });
};

const scoreCriteria = (
  phase: PlanningPhase,
  benchmarkCase: PlanningCase,
  expected: PlanningPhaseCriteria,
  observation: PlanningObservation,
): PlanningScore => {
  const nodesByRole = new Map<string, PlanningObservation['nodes'][number][]>();
  observation.nodes.forEach((node) =>
    node.roleIds.forEach((roleId) => {
      const nodes = nodesByRole.get(roleId) ?? [];
      nodes.push(node);
      nodesByRole.set(roleId, nodes);
    }),
  );
  const results: PlanningCriterionResult[] = [];
  expected.roles.forEach(({ roleId, outputIds, behaviorIds }) => {
    const nodes = nodesByRole.get(roleId) ?? [];
    const observedOutputs = new Set(nodes.flatMap(({ outputIds: ids }) => ids));
    const observedBehaviors = new Set(
      nodes.flatMap(({ behaviorIds: ids }) => ids),
    );
    results.push({
      id: `role:${roleId}`,
      kind: 'role',
      passed: nodes.length > 0,
    });
    outputIds.forEach((id) =>
      results.push({
        id: `output:${roleId}:${id}`,
        kind: 'output',
        passed: observedOutputs.has(id),
      }),
    );
    behaviorIds.forEach((id) =>
      results.push({
        id: `behavior:${roleId}:${id}`,
        kind: 'behavior',
        passed: observedBehaviors.has(id),
      }),
    );
  });

  const dependencies = new Map(
    benchmarkCase.gold.dependencies.map((dependency) => [
      dependency.id,
      dependency,
    ]),
  );
  expected.dependencyIds.forEach((id) => {
    const dependency = dependencies.get(id)!;
    const before = nodesByRole.get(dependency.beforeRoleId) ?? [];
    const after = nodesByRole.get(dependency.afterRoleId) ?? [];
    results.push({
      id: `dependency:${id}`,
      kind: 'dependency',
      passed: after.some((later) =>
        before.some(
          (earlier) =>
            (dependency.allowSameNode && later.id === earlier.id) ||
            dependsTransitively(observation, later.id, earlier.id),
        ),
      ),
    });
  });

  const observedBehaviors = new Set(
    observation.nodes.flatMap(({ behaviorIds }) => behaviorIds),
  );
  expected.forbiddenBehaviorIds.forEach((id) =>
    results.push({
      id: `forbidden:${id}`,
      kind: 'forbidden-behavior',
      passed: !observedBehaviors.has(id),
    }),
  );
  results.push({
    id: 'node-count',
    kind: 'node-count',
    passed:
      observation.nodes.length >= expected.nodeCount.min &&
      observation.nodes.length <= expected.nodeCount.max,
  });

  const expectedAtoms = planningPhaseAtoms(expected);
  const actualAtoms = new Set(results.map(({ id }) => id));
  if (
    expectedAtoms.size !== actualAtoms.size ||
    [...expectedAtoms].some((atom) => !actualAtoms.has(atom))
  ) {
    throw new Error(
      'Planning scorer did not materialize every expected criterion.',
    );
  }
  const passedCount = results.filter(({ passed }) => passed).length;
  const score = rounded(passedCount / results.length);
  return {
    phase,
    passed: passedCount === results.length,
    score,
    passedCount,
    totalCount: results.length,
    criteria: results,
  };
};

const validateObservation = (
  benchmarkCase: PlanningCase,
  observation: PlanningObservation,
): PlanningObservation => {
  const parsed = PlanningObservationSchema.parse(observation);
  const roles = new Set(benchmarkCase.gold.roles.map(({ id }) => id));
  const outputs = new Set(benchmarkCase.gold.outputs.map(({ id }) => id));
  const behaviors = new Set(benchmarkCase.gold.behaviors.map(({ id }) => id));
  parsed.nodes.forEach((node) => {
    for (const [values, known, label] of [
      [node.roleIds, roles, 'role'],
      [node.outputIds, outputs, 'output'],
      [node.behaviorIds, behaviors, 'behavior'],
    ] as const) {
      const unknown = values.find((id) => !known.has(id));
      if (unknown !== undefined) {
        throw new TypeError(
          `Planning observation contains an unknown ${label} ID.`,
        );
      }
    }
  });
  return parsed;
};

const dependsTransitively = (
  observation: PlanningObservation,
  nodeId: string,
  dependencyId: string,
): boolean => {
  const byId = new Map(observation.nodes.map((node) => [node.id, node]));
  const pending = [...(byId.get(nodeId)?.dependsOn ?? [])];
  const visited = new Set<string>();
  while (pending.length > 0) {
    const current = pending.shift()!;
    if (current === dependencyId) return true;
    if (visited.has(current)) continue;
    visited.add(current);
    pending.push(...(byId.get(current)?.dependsOn ?? []));
  }
  return false;
};

const rounded = (value: number): number => Number(value.toFixed(12));
