import { generateCriteria } from './stages/criteria.mjs';
import { summarize } from './stages/delivery.mjs';
import { selectSkills } from './stages/gate.mjs';
import { createSearch } from './stages/index.mjs';
import { generateInitialGoals } from './stages/p0.mjs';
import { generatePlan } from './stages/p1.mjs';
import { retrieveCandidates } from './stages/retrieval.mjs';
import { executePlan } from './stages/schedule.mjs';

/** Run the experiment's stages with their explicit input contexts. */
export const run = async (runtime, { request, skills }) => {
  const p0 = await generateInitialGoals({ runtime, request });
  const search = await createSearch({ runtime, skills });

  const candidates = await retrieveCandidates({
    runtime,
    search,
    request,
    goals: p0.goals,
  });

  const selectedSkills = await selectSkills({
    runtime,
    request,
    goals: p0.goals,
    candidates,
  });
  const p1 = await generatePlan({ runtime, request, skills: selectedSkills });

  const nodes = await generateCriteria({
    runtime,
    request,
    skills: selectedSkills,
    plan: p1,
  });

  await runtime.record('graph', nodes);

  const results = await executePlan({ runtime, search, request, nodes });

  return {
    p0,
    p1,
    selectedSkills: selectedSkills.map(({ name }) => name),
    ...summarize({ nodes, results }),
  };
};
