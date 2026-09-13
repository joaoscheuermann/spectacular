/* Execute the original request in one conversation with initial and on-demand skills. */
import { z } from 'zod';

import { createSkillSearch } from './search.mjs';

const text = z.string().trim().min(1);

const submission = z
  .object({
    status: z.enum(['completed', 'blocked']),
    result: text,
  })
  .strict();

const system = `Complete the request in the supplied workspace. Skills guide methods, not scope.
Use search_skills when guidance is missing. Inspect actual inputs and outputs;
preserve shell failures and check exit codes, stderr and truncation.
Once all requirements are verified, stop using tools and submit completed.
Check again only for a failure, contradiction or requirement without evidence.
Correct fixable problems; if blocked after reasonable alternatives, state the obstacle.
Return a Markdown result with output paths, verification evidence and unresolved work.`;

export const execute = async ({ runtime, request, skills, search }) => {
  const searches = [];

  const executor = runtime.agent({
    stage: 'execute',
    system: `${system}

# Selected skills

${skills.map(({ name, body }) => `## ${name}\n\n${body}`).join('\n\n')}`,
    profile: 'executionModel',
    tools: true,
    extraTools: [createSkillSearch({ runtime, request, search, searches })],
  });

  const input = `# Original request

${request}

${runtime.environment}`;
  const result = await executor.complete(input, submission);

  return runtime.record('direct', {
    ...result,
    delivery: result.status === 'completed' ? result.result : null,
    searches,
    observations: executor.observations(),
    metrics: {
      skillSearches: searches.length,
      observations: executor.observations().length,
    },
  });
};
