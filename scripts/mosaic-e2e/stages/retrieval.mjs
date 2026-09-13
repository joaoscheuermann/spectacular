/*
Retrieve and rerank skills independently for each P0 goal using request plus goal.
Receives the runtime, indexed search and request/goals; records each ranking and
returns the union by skill name in first-seen order. No generative prompt is used.
**/

export const retrieveCandidates = async ({
  runtime,
  search,
  request,
  goals,
}) => {
  const candidatesByName = new Map();

  for (const goal of goals) {
    const ranked = await search(request + '\n\n' + goal);

    await runtime.record('retrieval.p0', { goal, ranked });

    for (const skill of ranked) {
      candidatesByName.set(skill.name, skill);
    }
  }

  return [...candidatesByName.values()];
};
