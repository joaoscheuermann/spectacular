import { execute } from './stages/execute.mjs';
import { createSearch } from './stages/index.mjs';
import { generateInitialGoals } from './stages/p0.mjs';
import { retrieveSkills } from './stages/retrieval.mjs';

/** Use draft goals for retrieval, then execute the whole request in one session. */
export const run = async (runtime, { request, skills }) => {
  const p0 = await generateInitialGoals({ runtime, request });
  const search = await createSearch({ runtime, skills });

  const selectedSkills = await retrieveSkills({
    runtime,
    search,
    request,
    goals: p0.goals,
  });

  await runtime.record('initial-skills', selectedSkills);

  const result = await execute({
    runtime,
    request,
    skills: selectedSkills,
    search,
  });

  return {
    p0,
    selectedSkills: selectedSkills.map(({ name }) => name),
    ...result,
  };
};
