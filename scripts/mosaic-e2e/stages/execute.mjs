/*
Execute the current goal with the original request and Markdown goal list as context,
with selected skills in the system prompt.
Give the judge only the current goal, fixed criteria and submitted result. Keep one
executor history across bounded submissions, ask a fresh judge and apply feedback.
Returns attempts, final status, candidate Markdown and all local tool observations.
**/

import { z } from 'zod';

import { feedbackInput } from './feedback.mjs';
import { judgeSubmission } from './judge.mjs';

const text = z.string().trim().min(1);
const submissionSchema = z.object({ result: text }).strict();

const system = `Complete and verify only the current goal in the workspace.
Use the original request and goal list as context, respecting the request's constraints.
Keep implementation and verification scoped to the current goal; leave other goals to their own execution.
Reuse and preserve earlier goals' results, including intermediate artifacts needed downstream.

Once the current goal is verified, submit Markdown with its output paths and verification evidence.
If you cannot complete it, report the specific unresolved obstacle.`;

const finalStatus = (decision) => {
  if (decision === 'accept') {
    return 'completed';
  }

  if (decision === 'continue') {
    return 'attempt_limit';
  }

  return decision;
};

/** Keep one executor history and fixed criteria across the bounded feedback loop. */
export const executeNode = async ({
  runtime,
  request,
  goals,
  node,
  skills,
}) => {
  const executor = runtime.agent({
    stage: 'execute.' + node.id,
    system: `${system}

${runtime.environment}

# Selected skills

${skills.map(({ name, body }) => `## ${name}\n\n${body}`).join('\n\n')}`,
    profile: 'executionModel',
    tools: true,
  });
  const attempts = [];
  const judgeObservations = [];

  let input = `# Original request

${request}

# Goals

${goals.map((goal) => `- ${goal.replace(/\n/g, '\n  ')}`).join('\n')}

# Current goal

${node.goal}`;

  for (let attempt = 1; attempt <= runtime.config.maxAttempts; attempt++) {
    const submission = await executor.complete(input, submissionSchema);

    const judgment = await judgeSubmission({
      runtime,
      node,
      attempt,
      submission,
    });
    const { evaluation, observations } = judgment;

    attempts.push({ attempt, submission, evaluation });

    judgeObservations.push(...observations);

    if (evaluation.decision !== 'continue') {
      break;
    }

    input = feedbackInput({ evaluation, observations: judgeObservations });
  }

  const lastAttempt = attempts.at(-1);

  return runtime.record('node.' + node.id, {
    ...node,
    skills: skills.map(({ name }) => name),
    attempts,
    status: finalStatus(lastAttempt.evaluation.decision),
    result: lastAttempt.submission.result,
    observations: [...executor.observations(), ...judgeObservations],
  });
};
