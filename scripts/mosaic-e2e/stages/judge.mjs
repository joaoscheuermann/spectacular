/*
Judge one candidate against the node's fixed criteria with a fresh tool-enabled agent.
Receives the runtime, node, attempt number and submission. Supplies only the current
goal, fixed criteria and candidate result. Permits independent inspection and validates
criterion coverage and references to this judge's own observations.
**/

import { z } from 'zod';

import * as prompt from '../context.mjs';
import { hasDuplicates, hasExactMembers } from '../validation.mjs';

const text = z.string().trim().min(1);

const criterionEvaluation = z
  .object({
    criterionId: text,
    satisfied: z.boolean(),
    evidence: text,
    observationIds: z.array(text),
  })
  .strict();

const validateJudgment = (judgment, expected, context) => {
  const { decision, evaluations } = judgment;
  const evaluatedIds = evaluations.map(({ criterionId }) => criterionId);
  const allSatisfied = evaluations.every(({ satisfied }) => satisfied);

  if (!hasExactMembers(evaluatedIds, expected.criteriaIds)) {
    context.addIssue({
      code: 'custom',
      message: 'Evaluate every criterion exactly once.',
    });
  }

  if ((decision === 'accept') !== allSatisfied) {
    context.addIssue({
      code: 'custom',
      message: 'Accept if and only if every criterion is satisfied.',
    });
  }

  const hasInvalidReferences = evaluations.some(
    ({ observationIds }) =>
      hasDuplicates(observationIds) ||
      observationIds.some((id) => !expected.observationIds.has(id)),
  );

  if (hasInvalidReferences) {
    context.addIssue({
      code: 'custom',
      message: 'Cite only available observation IDs, without duplicates.',
    });
  }
};

/** Read the ledger at validation time so the judge can cite its latest tool calls. */
const judgment = (criteria, observations) =>
  z
    .object({
      decision: z.enum(['accept', 'continue', 'needs_revision', 'blocked']),
      evaluations: z.array(criterionEvaluation),
      feedback: text,
    })
    .strict()
    .superRefine((value, context) => {
      const expected = {
        criteriaIds: criteria.map(({ id }) => id),
        observationIds: new Set(observations().map(({ id }) => id)),
      };

      validateJudgment(value, expected, context);
    });

const system = `Evaluate whether the current node's fixed acceptance criteria are satisfied.
Use the current goal, its criteria and the candidate result. Inspect actual files
with the available core tools when needed. Only inspect and compute checks; do not repair, create,
delete or modify task artifacts. Executor claims and file paths alone are not proof.
Evaluate every criterion exactly once with its exact ID and concise evidence.
Cite only observation IDs from your own tool results for tool-dependent claims.
References mentioned in the candidate are not your evidence. An empty list is valid
only for claims directly verifiable in the supplied text.
Consider nonzero exit codes, stderr, timeouts and truncated outputs. Missing proof means
unsatisfied. Choose accept only when all criteria are satisfied. Otherwise choose continue
for correctable work, needs_revision for an invalid planning assumption, or blocked for
a demonstrated impossibility. Give actionable feedback; never rewrite the criteria.`;

export const judgeSubmission = async ({
  runtime,
  node,
  attempt,
  submission,
}) => {
  const judge = runtime.agent({
    stage: 'judge.' + node.id + '.' + attempt,
    system: `${system}\n\n${runtime.environment}`,
    profile: 'judgeModel',
    tools: true,
  });

  const input = [
    prompt.section('Current node', {
      id: node.id,
      goal: node.goal,
      acceptanceCriteria: node.acceptanceCriteria,
    }),
    prompt.section('Candidate result', submission.result),
  ].join('\n\n');

  const evaluation = await judge.complete(
    input,
    judgment(node.acceptanceCriteria, judge.observations),
  );

  return { evaluation, observations: judge.observations() };
};
