/*
Evaluate the merged P0 candidate union against the request and all draft goals.
Receives the runtime, request, goals and candidate skill bodies; returns kept skills
in candidate order after one keep/drop judgment per candidate, or skips an empty set.
**/

import { z } from 'zod';

import * as prompt from '../context.mjs';
import { hasExactMembers } from '../validation.mjs';

const text = z.string().trim().min(1);

const skillDecision = z
  .object({
    name: text,
    decision: z.enum(['keep', 'drop']),
    reason: text,
  })
  .strict();

const selections = (skills) => {
  const expectedNames = skills.map(({ name }) => name);

  return z
    .object({
      decisions: z.array(skillDecision),
    })
    .strict()
    .refine(
      ({ decisions }) =>
        hasExactMembers(
          decisions.map(({ name }) => name),
          expectedNames,
        ),
      { message: 'Evaluate each supplied skill exactly once.' },
    );
};

const system = `Evaluate every supplied candidate skill exactly once.
Keep guidance that materially helps produce or verify the requested outcomes.
Drop irrelevant, redundant or conflicting guidance. The request defines scope;
a draft goal or skill must not introduce an unsupported requirement.
Return each candidate name with keep or drop and a concise reason.`;

export const selectSkills = async ({ runtime, request, goals, candidates }) => {
  if (candidates.length === 0) {
    return [];
  }

  const input = [
    prompt.context(request, candidates),
    prompt.section('Goals', goals),
  ].join('\n\n');

  const { decisions } = await runtime.complete({
    stage: 'gate',
    system,
    input,
    schema: selections(candidates),
  });

  const keptNames = new Set(
    decisions
      .filter(({ decision }) => decision === 'keep')
      .map(({ name }) => name),
  );

  return candidates.filter(({ name }) => keptNames.has(name));
};
