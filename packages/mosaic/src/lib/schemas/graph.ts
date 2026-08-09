import * as z from 'zod';
import { ToolMetadataSchema } from 'tool';

import { validateGraphSchema } from './graph-validations.js';
import { ArtifactSchema } from './artifact.js';
import { NodeOutcomeSchema } from './outcome.js';
import {
  OrderedBundleSchema,
  SkillCandidateSchema,
  validateRoutingTrace,
} from './routing.js';
import { RuntimeTerminationSchema } from './termination.js';

const NonEmptyStringSchema = z.string().trim().min(1);
const DependenciesSchema = z
  .array(NonEmptyStringSchema)
  .superRefine((dependencies, context) => {
    if (new Set(dependencies).size === dependencies.length) return;
    context.addIssue({
      code: 'custom',
      message: 'Dependencies must be unique.',
    });
  });

export const PlannedNodeSchema = z
  .object({
    id: NonEmptyStringSchema.describe(
      'Unique stable node identifier. Use `n<step>:<description>` (for example, `n01:explore_workspace`). Other nodes must copy this complete value into `dependsOn`; the `n01` prefix alone is not an ID.',
    ),
    goal: NonEmptyStringSchema.describe(
      'Observable result that must be true when the node completes.',
    ),
    doneWhen: z
      .array(NonEmptyStringSchema)
      .min(1)
      .describe('Non-empty observable completion criteria.'),
    dependsOn: DependenciesSchema.describe(
      "Complete `id` values of this node's direct prerequisites. Every value must exactly match another node's `id` in this `nodes` array, including the `:<description>` suffix (for example, `n01:explore_workspace`, never `n01`). Use `[]` when this node has no prerequisites.",
    ),
    deliver: z.boolean().describe('Whether this terminal result is delivered.'),
  })
  .strict();

const withGraphValidation = <Schema extends z.ZodType>(schema: Schema) =>
  schema.superRefine((data, context) => {
    const result = validateGraphSchema(
      data as { readonly nodes: readonly z.output<typeof PlannedNodeSchema>[] },
    );
    result.errors.forEach((message) =>
      context.addIssue({ code: 'custom', message, path: ['nodes'] }),
    );
  });

/** Strict model-owned planning output. Runtime scheduler fields are excluded. */
export const PlannedGraphSchema = withGraphValidation(
  z
    .object({
      nodes: z
        .array(PlannedNodeSchema)
        .min(1)
        .describe(
          'Complete non-empty plan graph. Node IDs must be unique, every dependency must use a complete node ID from this array, and dependencies must be acyclic. Mark at least one node with no dependents as deliverable, and do not mark a node as deliverable when another node depends on it.',
        ),
    })
    .strict(),
);

export const NodeSchema = PlannedNodeSchema.extend({
  status: z.enum([
    'pending',
    'ready',
    'running',
    'completed',
    'needs_revision',
    'blocked',
    'failed',
  ]),
  index: z.number().int().nonnegative(),
  candidates: z.array(SkillCandidateSchema),
  bundle: OrderedBundleSchema.nullable(),
  tools: z.array(ToolMetadataSchema),
  artifacts: z.array(ArtifactSchema),
  outcome: NodeOutcomeSchema.nullable(),
  termination: RuntimeTerminationSchema.nullable(),
}).strict();

export const GraphSchema = withGraphValidation(
  z
    .object({
      revision: z.number().int().safe().nonnegative(),
      nodes: z.array(NodeSchema).min(1),
    })
    .strict(),
).superRefine((graph, context) => {
  graph.nodes.forEach((node, index) => {
    validateRoutingTrace(node, context, ['nodes', index]);
    validateRuntimeState(node, index, context);
    validateRuntimeOwnership(node, graph.nodes, index, context);
  });
});

/** Validates a newly materialized graph before execution begins. */
export const StrictGraphSchema = GraphSchema.superRefine((graph, context) => {
  graph.nodes.forEach((node, index) => {
    if (node.status !== 'pending') {
      context.addIssue({
        code: 'custom',
        path: ['nodes', index, 'status'],
        message: 'Generated nodes must start pending.',
      });
    }
    if (node.index !== index) {
      context.addIssue({
        code: 'custom',
        path: ['nodes', index, 'index'],
        message: `Generated node index must be ${index}.`,
      });
    }
    for (const field of ['candidates', 'tools', 'artifacts'] as const) {
      if (node[field].length === 0) continue;
      context.addIssue({
        code: 'custom',
        path: ['nodes', index, field],
        message: `Generated node ${field} must be empty.`,
      });
    }
    if (
      node.bundle !== null ||
      node.outcome !== null ||
      node.termination !== null
    ) {
      context.addIssue({
        code: 'custom',
        path: ['nodes', index],
        message:
          'Generated node bundle, outcome, and termination must be null.',
      });
    }
  });
});

export type PlannedGraph = z.output<typeof PlannedGraphSchema>;

/** Adds scheduler-owned fields deterministically to model planning output. */
export const materializeGraph = (plan: PlannedGraph, revision: number) =>
  StrictGraphSchema.parse({
    revision,
    nodes: plan.nodes.map((node, index) => ({
      ...node,
      status: 'pending' as const,
      index,
      candidates: [],
      bundle: null,
      tools: [],
      artifacts: [],
      outcome: null,
      termination: null,
    })),
  });

/** Validates ordered runtime snapshots and their contiguous revision sequence. */
export const GraphHistorySchema = z
  .array(GraphSchema)
  .superRefine((graphs, context) => {
    graphs.forEach((graph, index) => {
      if (graph.revision === index) return;
      context.addIssue({
        code: 'custom',
        path: [index, 'revision'],
        message: `Graph revision must be ${index}.`,
      });
    });
  });

type RuntimeNode = z.output<typeof NodeSchema>;

const validateRuntimeState = (
  node: RuntimeNode,
  index: number,
  context: z.RefinementCtx,
): void => {
  const path = ['nodes', index] as const;
  if (['pending', 'ready', 'running'].includes(node.status)) {
    if (node.outcome === null && node.termination === null) return;
  } else if (node.status === 'completed') {
    if (node.outcome?.status === 'completed' && node.termination === null)
      return;
  } else if (node.status === 'needs_revision') {
    if (node.outcome?.status === 'needs_revision' && node.termination === null)
      return;
  } else if (node.status === 'failed') {
    if (node.outcome?.status === 'failed' && node.termination === null) return;
  } else {
    const modelBlocked =
      node.outcome?.status === 'blocked' && node.termination === null;
    const turnBlocked =
      node.outcome === null && node.termination?.type === 'turn_limit';
    const dependencyBlocked =
      node.outcome === null && node.termination?.type === 'dependency';
    const revisionBlocked =
      node.outcome?.status === 'needs_revision' &&
      node.termination?.type === 'revision_limit';
    if (modelBlocked || turnBlocked || dependencyBlocked || revisionBlocked)
      return;
  }

  context.addIssue({
    code: 'custom',
    path: [...path, 'status'],
    message: 'Node status, outcome, and termination are inconsistent.',
  });
};

const validateRuntimeOwnership = (
  node: RuntimeNode,
  nodes: readonly RuntimeNode[],
  index: number,
  context: z.RefinementCtx,
): void => {
  if (
    node.outcome?.revisionRequest !== null &&
    node.outcome?.revisionRequest !== undefined &&
    node.outcome.revisionRequest.goalId !== node.id
  ) {
    context.addIssue({
      code: 'custom',
      path: ['nodes', index, 'outcome', 'revisionRequest', 'goalId'],
      message: `Revision goalId must be ${node.id}.`,
    });
  }

  const observations = [
    ...(node.outcome?.observations ?? []),
    ...(node.termination?.type === 'turn_limit'
      ? node.termination.observations
      : []),
  ];
  if (observations.some(({ goalId }) => goalId !== node.id)) {
    context.addIssue({
      code: 'custom',
      path: ['nodes', index],
      message: `Every observation must belong to node ${node.id}.`,
    });
  }

  if (node.termination?.type !== 'dependency') return;
  const expected = node.dependsOn.filter((id) => {
    const dependency = nodes.find((candidate) => candidate.id === id);
    return dependency?.status !== 'completed';
  });
  if (sameItems(node.termination.dependencyIds, expected)) return;
  context.addIssue({
    code: 'custom',
    path: ['nodes', index, 'termination', 'dependencyIds'],
    message:
      'Dependency termination must name direct non-completed dependencies.',
  });
};

const sameItems = (
  left: readonly string[],
  right: readonly string[],
): boolean =>
  left.length === right.length &&
  left.every((item, index) => item === right[index]);
