/*
Retrieve, rerank and select skills independently for each P0 goal.
Return the approved union deduplicated by name in first-accepted order.
**/

import { selectSkills } from './gate.mjs';

export const retrieveSkills = async ({ runtime, search, request, goals }) => {
  const selectedByName = new Map();

  for (const [index, goal] of goals.entries()) {
    const ranked = await search(request + '\n\n' + goal);

    await runtime.record('retrieval.p0.' + (index + 1), { goal, ranked });

    const selected = await selectSkills({
      runtime,
      request,
      goals: [goal],
      candidates: ranked,
      stage: 'gate.p0.' + (index + 1),
    });

    for (const skill of selected) {
      if (!selectedByName.has(skill.name)) {
        selectedByName.set(skill.name, skill);
      }
    }
  }

  return [...selectedByName.values()];
};
