/*
Generate catalog-independent draft goals from the original request.
Receives the runtime and request; returns goals without criteria or dependencies.
**/

import { z } from 'zod';

const text = z.string().trim().min(1);
const goals = z.object({ goals: z.array(text).min(1) }).strict();

const system = `Decompose the request into a small set of observable outcome goals.
Preserve its requirements, constraints and deliverables. Do not invent scope,
choose skills or tools, or generate acceptance criteria.`;

export const generateInitialGoals = ({ runtime, request }) =>
  runtime.complete({
    stage: 'p0',
    system,
    input: `# Original request

${request}`,
    schema: goals,
  });
