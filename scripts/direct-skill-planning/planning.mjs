import { AsyncLocalStorage } from 'node:async_hooks';

import { createVectorIndex } from 'victor';
import * as z from 'zod';

import { goalsSystem, goalsUser } from '../full-skill-vs-hints/prompts.mjs';
import {
  directSystem,
  goalRankingQuery,
  requestRankingQuery,
  revisionSystem,
  skillDocument,
} from './prompts.mjs';
import { retryProvider } from './retry.mjs';

const goalsSchema = z
  .object({ goals: z.array(z.string().trim().min(1)).min(1) })
  .strict();

const completeGoals = async (deps, system, user, context) => {
  const result = await retryProvider(
    () =>
      deps.provider.complete({
        model: deps.config.model,
        effort: deps.config.planningEffort,
        messages: [
          { role: 'system', content: system },
          { role: 'user', content: user },
        ],
        schema: goalsSchema,
        flags: { sensitiveOutput: true },
      }),
    {
      ...context,
      operation: 'planning_completion',
      model: deps.config.model,
      onFailure: deps.onProviderFailure,
    },
  );

  return result.structured.goals;
};

export const indexSkills = async (
  provider,
  config,
  logger,
  catalog,
  onProviderFailure,
) => {
  const embeddingContext = new AsyncLocalStorage();
  const vectors = createVectorIndex({
    dimensions: config.embeddingDimensions,
    logger,
    embedding: async (input) => {
      const context = embeddingContext.getStore() ?? {};
      const result = await retryProvider(
        () =>
          provider.embedding({
            model: config.embeddingModel,
            input,
            dimensions: config.embeddingDimensions,
            flags: { sensitiveOutput: true },
          }),
        {
          ...context,
          operation: context.operation ?? 'embedding',
          model: config.embeddingModel,
          onFailure: onProviderFailure,
        },
      );
      return result.embedding;
    },
  });

  for (const skill of catalog) {
    await embeddingContext.run(
      { operation: 'catalog_embedding', skill: skill.name },
      () => vectors.add(skill, skillDocument),
    );
  }

  return {
    search(query, limit, context) {
      return embeddingContext.run(context, () => vectors.search(query, limit));
    },
  };
};

const normalizeRanking = (results, candidates, limit) => {
  if (results.length !== limit) {
    throw new Error(
      `Reranker returned ${results.length} results for top-${limit}.`,
    );
  }

  const seen = new Set();
  const ranked = results.map(({ index, relevanceScore }) => {
    if (
      !Number.isSafeInteger(index) ||
      index < 0 ||
      index >= candidates.length ||
      !Number.isFinite(relevanceScore) ||
      seen.has(index)
    ) {
      throw new Error('Reranker returned an invalid ranking.');
    }

    seen.add(index);
    return {
      skill: candidates[index].data,
      vectorScore: candidates[index].score,
      rerankerScore: relevanceScore,
    };
  });

  return ranked
    .sort(
      (left, right) =>
        right.rerankerScore - left.rerankerScore ||
        left.skill.name.localeCompare(right.skill.name),
    )
    .map((entry, rank) => ({ ...entry, rank: rank + 1 }));
};

const rankSkills = async ({
  provider,
  vectors,
  config,
  query,
  context = {},
  onProviderFailure,
}) => {
  const shortlist = await vectors.search(query, config.retrievalK, {
    ...context,
    operation: 'query_embedding',
  });
  const { results } = await retryProvider(
    () =>
      provider.rerank({
        model: config.rerankerModel,
        query,
        documents: shortlist.map(({ data }) => skillDocument(data)),
        topN: config.topK,
        flags: { sensitiveOutput: true },
      }),
    {
      ...context,
      operation: 'rerank',
      model: config.rerankerModel,
      onFailure: onProviderFailure,
    },
  );
  const reranked = normalizeRanking(results, shortlist, config.topK);
  const selected = reranked.filter(
    ({ rerankerScore }) => rerankerScore >= config.minRerankerScore,
  );

  return { shortlist, reranked, selected };
};

const rankByGoal = (deps, current, round, goals) =>
  Promise.all(
    goals.map(async (goal, index) => ({
      goal,
      ...(await rankSkills({
        ...deps,
        query: goalRankingQuery(current.objective, goal),
        context: {
          caseName: current.name,
          round,
          retrieval: 'goal',
          goalIndex: index + 1,
        },
      })),
    })),
  );

const uniqueSkills = (byGoal) => {
  const seen = new Set();
  return byGoal.flatMap(({ selected }) =>
    selected.flatMap(({ skill }) => {
      if (seen.has(skill.name)) return [];
      seen.add(skill.name);
      return [skill];
    }),
  );
};

const bodies = (skills) => skills.map(({ body }) => body);

const generateDirect = (deps, current, round, skills, arm) =>
  completeGoals(deps, directSystem(bodies(skills)), current.objective, {
    caseName: current.name,
    round,
    arm,
  });

const revise = (deps, current, round, p0, skills, arm) =>
  completeGoals(
    deps,
    revisionSystem(current.objective, bodies(skills)),
    goalsUser(p0),
    { caseName: current.name, round, arm },
  );

export const generatePlans = async (deps, current, round) => {
  const prefix = `${current.name} ${round}/${deps.config.rounds}`;
  const [requestRanking, p0] = await Promise.all([
    deps.action(`${prefix}: retrieving skills from request`, () =>
      rankSkills({
        ...deps,
        query: requestRankingQuery(current.objective),
        context: {
          caseName: current.name,
          round,
          retrieval: 'request',
        },
      }),
    ),
    deps.action(`${prefix}: generating P0`, () =>
      completeGoals(deps, goalsSystem, current.objective, {
        caseName: current.name,
        round,
        arm: 'p0',
      }),
    ),
  ]);

  const directPromise = deps.action(`${prefix}: generating Direct`, () =>
    generateDirect(
      deps,
      current,
      round,
      requestRanking.selected.map(({ skill }) => skill),
      'direct',
    ),
  );
  const requestP1Promise = deps.action(`${prefix}: generating Request P1`, () =>
    revise(
      deps,
      current,
      round,
      p0,
      requestRanking.selected.map(({ skill }) => skill),
      'requestP1',
    ),
  );
  const byGoalPromise = deps.action(
    `${prefix}: retrieving skills per P0 goal`,
    () => rankByGoal(deps, current, round, p0),
  );
  const goalSkillsPromise = byGoalPromise.then(uniqueSkills);
  const directGoalPromise = goalSkillsPromise.then((skills) =>
    deps.action(`${prefix}: generating Direct Goal`, () =>
      generateDirect(deps, current, round, skills, 'directGoal'),
    ),
  );
  const goalP1Promise = goalSkillsPromise.then((skills) =>
    deps.action(`${prefix}: generating Goal P1`, () =>
      revise(deps, current, round, p0, skills, 'goalP1'),
    ),
  );
  const [direct, requestP1, byGoal, directGoal, goalP1] = await Promise.all([
    directPromise,
    requestP1Promise,
    byGoalPromise,
    directGoalPromise,
    goalP1Promise,
  ]);

  return {
    requestRanking,
    p0,
    byGoal,
    direct,
    directGoal,
    requestP1,
    goalP1,
  };
};
