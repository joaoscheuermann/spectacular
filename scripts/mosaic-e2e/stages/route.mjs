/*
Select execution guidance afresh for one node from the full indexed catalog.
Receives the runtime, search, request and node with fixed criteria. Searches using
request, goal and criteria, then evaluates complete candidate bodies against the
request and node. Returns kept skills without changing the core tool menu.
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

export const routeNode = async ({ runtime, search, request, node }) => {
  const criteria = node.acceptanceCriteria.map(({ statement }) => statement);
  const query = request + '\n\n' + node.goal + '\n' + criteria.join('\n');
  const candidates = await search(query);

  await runtime.record('retrieval.' + node.id, candidates);

  if (candidates.length === 0) {
    return [];
  }

  const input = [
    prompt.context(request, candidates),
    prompt.section('Goals', [node]),
  ].join('\n\n');

  const { decisions } = await runtime.complete({
    stage: 'route.' + node.id,
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
