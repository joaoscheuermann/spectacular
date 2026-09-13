/*
Assign fixed acceptance criteria with one call per node using the request, kept skills and full P1 graph.
Receives the runtime, request, skills and plan; returns topologically ordered nodes
with stable criterion IDs, without changing the planner-owned graph fields.
**/

import { z } from 'zod';

import * as prompt from '../context.mjs';
import { ordered } from '../validation.mjs';

const text = z.string().trim().min(1);

const criteriaSchema = z
  .object({
    criteria: z.array(text).min(1),
  })
  .strict();

const system = `Define acceptance criteria only for the current node.
Use the full plan as context. Do not modify the graph. Return at least one criterion.
Cover every semantic claim of its goal with concrete, observable completion conditions.
Respect the original request; skills may add applicable verification methods, not scope.
Each condition must be verifiable when this node ends using its result and ancestors.
Do not require evidence from descendants. Avoid redundant criteria and vague quality claims.`;

export const generateCriteria = async ({ runtime, request, skills, plan }) => {
  const context = [
    prompt.context(request, skills),
    prompt.section('Plan', plan),
  ].join('\n\n');
  const nodes = [];

  for (const node of ordered(plan.nodes)) {
    const { criteria } = await runtime.complete({
      stage: 'criteria.' + node.id,
      system,
      profile: 'criteriaModel',
      input: [context, prompt.section('Current node', node)].join('\n\n'),
      schema: criteriaSchema,
    });

    nodes.push({
      ...node,
      acceptanceCriteria: criteria.map((statement, index) => ({
        id: node.id + ':c' + (index + 1),
        statement,
      })),
    });
  }

  return nodes;
};
