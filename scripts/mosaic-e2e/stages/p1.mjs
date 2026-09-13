/*
Synthesize a fresh dependency graph from the request, globally kept and always-available skill bodies.
Receives the runtime, request and skills; never consumes P0 or generates criteria.
**/

import { z } from 'zod';

import { hasDuplicates, ordered } from '../validation.mjs';

const text = z.string().trim().min(1);

const node = z
  .object({
    id: text,
    goal: text,
    dependencies: z.array(text),
    deliver: z.boolean(),
  })
  .strict();

const validateGraph = ({ nodes }, context) => {
  try {
    ordered(nodes);
  } catch (error) {
    context.addIssue({ code: 'custom', message: error.message });
  }

  const prerequisiteIds = new Set(
    nodes.flatMap(({ dependencies }) => dependencies),
  );
  const hasDeliverable = nodes.some(({ deliver }) => deliver);

  const hasNonterminalDeliverable = nodes.some(
    ({ id, deliver }) => deliver && prerequisiteIds.has(id),
  );

  const hasRepeatedDependency = nodes.some(({ dependencies }) =>
    hasDuplicates(dependencies),
  );

  if (!hasDeliverable || hasNonterminalDeliverable || hasRepeatedDependency) {
    context.addIssue({
      code: 'custom',
      message: 'Require terminal deliverables and unique dependencies.',
    });
  }
};

const graph = z
  .object({
    nodes: z.array(node).min(1),
  })
  .strict()
  .superRefine(validateGraph);

const system = `Produce P1: a complete, executable goal graph covering the original request.
Every goal must name a concrete required result and the constraints needed to produce it.
Never return placeholders, dummy nodes, TODOs or a promise to plan later.
Cover every requested deliverable and requirement across the goals. A single node is
valid only if its goal describes the complete requested outcome, not a generic label.
Use the fewest nodes that preserve this coverage. Keep coupled choices together;
split only results whose prerequisites can be completed before their dependents.
Use unique IDs, existing non-duplicate dependencies and no cycles. Mark at least
one terminal user-facing result deliverable; only terminal nodes may be deliverable.
Skills guide methods without expanding the request. Describe outcomes, not tool calls.
Before submitting, check coverage and replace any vague or unfinished goal.
Do not execute the task or generate acceptance criteria.`;

export const generatePlan = ({ runtime, request, skills }) =>
  runtime.complete({
    stage: 'p1',
    system,
    input: `# Original request

${request}

# Selected skills

${skills.map(({ name, body }) => `## ${name}\n\n${body}`).join('\n\n')}

# Always-available skills

${runtime.alwaysAvailableSkills.map(({ name, body }) => `## ${name}\n\n${body}`).join('\n\n')}`,
    schema: graph,
  });
