import type { MosaicOptions } from 'mosaic';
import {
  mosaic as evaluationMosaic,
  type ExecutionResult,
  type MosaicEvaluationHooks,
} from 'mosaic/evaluation';

import {
  PlanningCaseSchema,
  PlanningGraphSchema,
  type PlanningCase,
  type PlanningGraph,
  type PlanningObservation,
} from './planning-schema.js';
import { planningObservationSchema } from './planning-observation-schema.js';
import {
  scorePlanningTransition,
  type PlanningTransitionScore,
} from './planning-scoring.js';

export const planningConditions = [
  'no-hints',
  'gold',
  'retrieved',
  'distractor',
] as const;

export type PlanningCondition = (typeof planningConditions)[number];
export type PlanningSkill = PlanningCase['catalog'][number];

export interface PlanningRevisionInput {
  readonly case: PlanningCase;
  readonly condition: Exclude<PlanningCondition, 'no-hints'>;
  readonly p0: PlanningGraph;
  readonly evidence: readonly PlanningSkill[];
}

export interface PlanningObservationInput {
  readonly case: PlanningCase;
  readonly phase: 'p0' | 'p1';
  readonly condition: PlanningCondition | null;
  readonly graph: PlanningGraph;
}

export interface PlanningRunAdapter {
  /** Produces P0 once; every condition receives this exact validated plan. */
  readonly initialPlan: (
    benchmarkCase: PlanningCase,
  ) => PlanningGraph | Promise<PlanningGraph>;
  /** Produces P1 from an explicit, condition-owned skill evidence set. */
  readonly revise: (
    input: PlanningRevisionInput,
  ) => PlanningGraph | 'unchanged' | Promise<PlanningGraph | 'unchanged'>;
  /** Maps a model graph to adjudicated semantic role/output/behavior labels. */
  readonly observe: (
    input: PlanningObservationInput,
  ) => PlanningObservation | Promise<PlanningObservation>;
  /** Supplies the frozen retrieval result used only by `retrieved`. */
  readonly retrieve: (input: {
    readonly case: PlanningCase;
    readonly p0: PlanningGraph;
  }) => readonly string[] | Promise<readonly string[]>;
}

export interface PlanningConditionResult {
  readonly condition: PlanningCondition;
  readonly evidenceSkillIds: readonly string[];
  readonly p0: PlanningGraph;
  readonly p1: PlanningGraph;
  readonly observations: {
    readonly p0: PlanningObservation;
    readonly p1: PlanningObservation;
  };
  readonly score: PlanningTransitionScore;
}

export interface PlanningRunOptions {
  readonly case: PlanningCase;
  readonly mosaic: MosaicOptions;
  readonly adapter: PlanningRunAdapter;
  readonly conditions?: readonly PlanningCondition[];
}

/**
 * Runs controlled P0-to-P1 conditions through the validated Mosaic engine.
 * Planning, routing, and execution provider boundaries are all intercepted, so
 * this runner itself makes no model, embedding, reranking, tool, or paid calls.
 */
export const runPlanningConditions = async ({
  case: inputCase,
  mosaic,
  adapter,
  conditions = planningConditions,
}: PlanningRunOptions): Promise<readonly PlanningConditionResult[]> => {
  const benchmarkCase = PlanningCaseSchema.parse(inputCase);
  const selected = validateConditions(conditions);
  const observationSchema = planningObservationSchema(benchmarkCase);
  const p0 = PlanningGraphSchema.parse(
    await adapter.initialPlan(benchmarkCase),
  );
  const p0Observation = observationSchema.parse(
    await adapter.observe({
      case: benchmarkCase,
      phase: 'p0',
      condition: null,
      graph: p0,
    }),
  );
  assertObservationGraph(p0, p0Observation, 'p0');
  const retrievedIds = selected.includes('retrieved')
    ? validateEvidenceIds(
        benchmarkCase,
        await adapter.retrieve({ case: benchmarkCase, p0 }),
      )
    : [];

  const results: PlanningConditionResult[] = [];
  for (const condition of selected) {
    const evidence = evidenceFor(benchmarkCase, condition, retrievedIds);
    let p1: PlanningGraph | undefined;
    const hooks = createPlanningConditionHooks({
      case: benchmarkCase,
      condition,
      p0,
      evidence,
      revise: adapter.revise,
      onP1: (graph) => {
        p1 = graph;
      },
    });
    const result = await evaluationMosaic(mosaic, { hooks }).prompt(
      benchmarkCase.request,
    );
    if (result.status !== 'completed' || p1 === undefined) {
      throw new Error(`Planning condition did not complete: ${condition}`);
    }
    const p1Observation = observationSchema.parse(
      await adapter.observe({
        case: benchmarkCase,
        phase: 'p1',
        condition,
        graph: p1,
      }),
    );
    assertObservationGraph(p1, p1Observation, `p1/${condition}`);
    results.push({
      condition,
      evidenceSkillIds: evidence.map(({ id }) => id),
      p0,
      p1,
      observations: { p0: p0Observation, p1: p1Observation },
      score: scorePlanningTransition(
        benchmarkCase,
        p0Observation,
        p1Observation,
      ),
    });
  }
  return results;
};

export interface PlanningConditionHookOptions {
  readonly case: PlanningCase;
  readonly condition: PlanningCondition;
  readonly p0: PlanningGraph;
  readonly evidence: readonly PlanningSkill[];
  readonly revise: PlanningRunAdapter['revise'];
  readonly onP1?: (graph: PlanningGraph) => void | Promise<void>;
}

/** Creates the provider-free hooks used by one controlled planning arm. */
export const createPlanningConditionHooks = ({
  case: inputCase,
  condition,
  p0: inputP0,
  evidence,
  revise,
  onP1,
}: PlanningConditionHookOptions): MosaicEvaluationHooks => {
  const benchmarkCase = PlanningCaseSchema.parse(inputCase);
  const p0 = PlanningGraphSchema.parse(inputP0);
  const selectedEvidence = evidenceFor(
    benchmarkCase,
    condition,
    evidence.map(({ id }) => id),
  );

  return {
    initialPlan: async ({ request }) => {
      assertRequest(benchmarkCase, request);
      return p0;
    },
    feedbackPlan: async ({ request, graph }) => {
      assertRequest(benchmarkCase, request);
      const materializedP0 = projectGraph(graph);
      if (stableJson(materializedP0) !== stableJson(p0)) {
        throw new Error('Mosaic P0 does not match the shared controlled plan.');
      }
      if (condition === 'no-hints') {
        await onP1?.(materializedP0);
        return 'unchanged';
      }
      const revised = await revise({
        case: benchmarkCase,
        condition,
        p0,
        evidence: selectedEvidence,
      });
      const p1 =
        revised === 'unchanged'
          ? materializedP0
          : PlanningGraphSchema.parse(revised);
      await onP1?.(p1);
      return revised === 'unchanged' ? 'unchanged' : p1;
    },
    routing: async ({ node }) => ({
      candidates: [],
      bundle: {
        goalId: node.id,
        skills: [],
        selectionRationale:
          'Controlled planning benchmark bypasses downstream skill routing.',
      },
    }),
    execution: planningExecutionStub,
  };
};

/** Completes every planned node without tools or provider activity. */
export const planningExecutionStub: NonNullable<
  MosaicEvaluationHooks['execution']
> = async ({ node }): Promise<ExecutionResult> => ({
  decision: {
    status: 'completed',
    criteria: node.doneWhen.map((_, criterionIndex) => ({
      criterionIndex,
      satisfied: true,
      evidence: 'Satisfied by the controlled planning execution stub.',
      observationIds: [],
    })),
    result: {
      markdown: `Controlled planning node completed: ${node.id}`,
      artifacts: [],
    },
    revisionRequest: null,
    reason: null,
  },
  observations: [],
});

const evidenceFor = (
  benchmarkCase: PlanningCase,
  condition: PlanningCondition,
  retrievedIds: readonly string[],
): readonly PlanningSkill[] => {
  const ids =
    condition === 'no-hints'
      ? []
      : condition === 'gold'
        ? benchmarkCase.gold.relevantSkillIds
        : condition === 'retrieved'
          ? validateEvidenceIds(benchmarkCase, retrievedIds)
          : benchmarkCase.gold.distractorSkillIds.slice(
              0,
              Math.max(1, benchmarkCase.gold.relevantSkillIds.length),
            );
  const byId = new Map(benchmarkCase.catalog.map((skill) => [skill.id, skill]));
  return ids.map((id) => byId.get(id)!);
};

const validateEvidenceIds = (
  benchmarkCase: PlanningCase,
  values: readonly string[],
): readonly string[] => {
  if (new Set(values).size !== values.length) {
    throw new TypeError('Planning evidence skill IDs must be unique.');
  }
  const known = new Set(benchmarkCase.catalog.map(({ id }) => id));
  if (values.some((id) => !known.has(id))) {
    throw new TypeError('Planning evidence contains an unknown skill ID.');
  }
  return [...values];
};

const validateConditions = (
  values: readonly PlanningCondition[],
): readonly PlanningCondition[] => {
  const known = new Set<PlanningCondition>(planningConditions);
  if (
    values.length === 0 ||
    new Set(values).size !== values.length ||
    values.some((value) => !known.has(value))
  ) {
    throw new TypeError(
      'Planning conditions must be non-empty, known, and unique.',
    );
  }
  return [...values];
};

const projectGraph = (
  graph: Parameters<
    NonNullable<MosaicEvaluationHooks['feedbackPlan']>
  >[0]['graph'],
): PlanningGraph =>
  PlanningGraphSchema.parse({
    nodes: graph.nodes.map(({ id, goal, doneWhen, dependsOn, deliver }) => ({
      id,
      goal,
      doneWhen,
      dependsOn,
      deliver,
    })),
  });

const assertRequest = (benchmarkCase: PlanningCase, request: string): void => {
  if (request === benchmarkCase.request) return;
  throw new Error('Planning hook request does not match its controlled case.');
};

const stableJson = (value: unknown): string => JSON.stringify(value);

const assertObservationGraph = (
  graph: PlanningGraph,
  observation: PlanningObservation,
  phase: string,
): void => {
  const graphNodes = new Map(
    graph.nodes.map(({ id, dependsOn }) => [id, [...dependsOn].sort()]),
  );
  if (
    graphNodes.size !== observation.nodes.length ||
    observation.nodes.some(
      ({ id, dependsOn }) =>
        stableJson(graphNodes.get(id)) !== stableJson([...dependsOn].sort()),
    )
  ) {
    throw new Error(
      `Planning observation does not preserve the ${phase} graph identity.`,
    );
  }
};
