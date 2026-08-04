import * as z from 'zod';
import { validateGraphSchema } from './validations.js';

/**
 * Validation schema for one goal node.
 */
export const NodeSchema = z.object({
  id: z
    .string()
    .describe(
      'A unique identifier for this node within the plan. Ex: node_01, node_02',
    ),

  goal: z
    .string()
    .describe(
      'The result that must be true when this node is complete. ' +
        'Describe an outcome, not steps, tool calls, skill names, or implementation details.',
    ),

  doneWhen: z
    .array(z.string())
    .describe(
      'Specific, observable conditions that are jointly sufficient to determine ' +
        'whether the goal is complete. Describe conditions, not execution steps.',
    ),

  dependsOn: z
    .array(z.string())
    .describe(
      'The IDs of direct prerequisite nodes. Use an empty array when there are none. ' +
        'Every referenced ID must exist, and dependencies must remain acyclic.',
    ),

  status: z
    .enum([
      'pending',
      'ready',
      'running',
      'completed',
      'needs_revision',
      'blocked',
      'failed',
    ])
    .describe(
      "The node's current execution state. It must be 'pending' in the initial plan.",
    ),

  deliver: z
    .boolean()
    .describe(
      "Whether this node's completed result must be returned to the user. " +
        'Set true only for user-facing terminal results.',
    ),

  index: z.number().describe(`index of the node in the graph, starting from 0`),
});

/**
 * Validation schema for a directed acyclic plan of goal nodes.
 */
export const GraphSchema = z.object({
  nodes: z
    .array(NodeSchema)
    .describe(
      'The goal nodes in the plan. IDs must be unique, dependencies must reference ' +
        'existing nodes and form a DAG, and at least one terminal node must be deliverable.',
    ),

  revision: z
    .string()
    .describe(
      "The identifier of the current plan revision. Use 'P0' for the initial plan.",
    ),
});

/**
 * Structural schema with graph-level invariant validation.
 */
export const StrictGraphSchema = GraphSchema.superRefine((data, ctx) => {
  const invariantCheck = validateGraphSchema(data);

  if (!invariantCheck.success) {
    for (const errorMessage of invariantCheck.errors) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: errorMessage,
        path: ['nodes'],
      });
    }
  }
});
