/*
Evaluate candidates against the request and the supplied goal or current need.
Receives the runtime, request, goals and candidate skill bodies; returns kept skills
in candidate order after one keep/drop judgment per candidate, or skips an empty set.
**/

import { z } from 'zod';

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

const system = `For each candidate, return its exact name, keep/drop and a brief reason.
Keep guidance useful to produce or verify the supplied goal or current need;
drop irrelevant, redundant or conflicting guidance. Relevance elsewhere is insufficient.
The original request bounds scope; goals and skills cannot add requirements.`;

export const selectSkills = async ({
  runtime,
  request,
  goals,
  candidates,
  stage = 'gate',
}) => {
  if (candidates.length === 0) {
    return [];
  }

  const input = `# Original request

${request}

# Candidate skills

${candidates.map(({ name, body }) => `## ${name}\n\n${body}`).join('\n\n')}

# Goals

${goals.map((goal, index) => `${index + 1}. ${goal}`).join('\n\n')}`;

  const { decisions } = await runtime.complete({
    stage,
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
