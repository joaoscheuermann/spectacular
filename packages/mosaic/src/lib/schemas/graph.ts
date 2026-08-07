import * as z from 'zod';
import { ToolMetadataSchema } from 'tool';

import type { NodeSkillSelection } from '../types/node-skill-selection.js';
import { validateGraphSchema } from './graph-validations.js';
import { ObservationSchema, RevisionRequestSchema } from './revision.js';

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
    id: NonEmptyStringSchema.describe('Unique stable node identifier.'),
    goal: NonEmptyStringSchema.describe(
      'Observable result that must be true when the node completes.',
    ),
    doneWhen: z
      .array(NonEmptyStringSchema)
      .min(1)
      .describe('Non-empty observable completion criteria.'),
    dependsOn: DependenciesSchema.describe('Direct prerequisite node IDs.'),
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
  z.object({ nodes: z.array(PlannedNodeSchema).min(1) }).strict(),
);

export const NodeArtifactsSchema = z
  .object({
    mime: NonEmptyStringSchema.describe('MIME type of the artifact.'),
    data: z.string(),
  })
  .strict();

export const NodeSkillSelectionSchema = z
  .object({
    skill: NonEmptyStringSchema,
    rationale: NonEmptyStringSchema.max(500),
  })
  .strict() satisfies z.ZodType<NodeSkillSelection>;

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
  skills: z.array(NodeSkillSelectionSchema),
  tools: z.array(ToolMetadataSchema),
  artifacts: z.array(NodeArtifactsSchema),
  observations: z.array(ObservationSchema),
  revisionRequest: RevisionRequestSchema.nullable(),
}).strict();

export const GraphSchema = withGraphValidation(
  z.object({ nodes: z.array(NodeSchema).min(1) }).strict(),
);

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
    for (const field of [
      'skills',
      'tools',
      'artifacts',
      'observations',
    ] as const) {
      if (node[field].length === 0) continue;
      context.addIssue({
        code: 'custom',
        path: ['nodes', index, field],
        message: `Generated node ${field} must be empty.`,
      });
    }
    if (node.revisionRequest !== null) {
      context.addIssue({
        code: 'custom',
        path: ['nodes', index, 'revisionRequest'],
        message: 'Generated node revisionRequest must be null.',
      });
    }
  });
});

export type PlannedGraph = z.output<typeof PlannedGraphSchema>;

/** Adds scheduler-owned fields deterministically to model planning output. */
export const materializeGraph = (plan: PlannedGraph) =>
  StrictGraphSchema.parse({
    nodes: plan.nodes.map((node, index) => ({
      ...node,
      status: 'pending' as const,
      index,
      skills: [],
      tools: [],
      artifacts: [],
      observations: [],
      revisionRequest: null,
    })),
  });
