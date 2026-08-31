#!/usr/bin/env node

import { readFile, readdir } from 'node:fs/promises';
import { basename, dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createFetchTransport, createUnifiedProvider } from 'llms';
import pino from 'pino';
import pretty from 'pino-pretty';
import {
  createHybridSearch,
  createLexicalIndex,
  createVectorIndex,
} from 'victor';

import {
  gateSystem,
  gateUser,
  p0System,
  p0User,
  p1System,
  p1User,
  retrievalQuery,
} from './prompt.mjs';
import { caseSchema, gateSchema, goalsSchema } from './schemas.mjs';
import { messages } from './utils.mjs';
import { aggregateMetrics, createOutput, scoreBundle } from './output.mjs';

const directory = dirname(fileURLToPath(import.meta.url));
const casesDirectory = join(directory, 'cases');
const skillsDirectory = join(casesDirectory, 'skills');

const config = {
  model: 'qwen/qwen3.8-27b',
  effort: 'high',
  embeddingModel: 'voyageai/voyage-4-large',
  embeddingDimensions: 1024,
  rerankerModel: 'voyageai/rerank-2.5',
  retrievalK: 20,
  minVectorScore: 0.3,
  topK: 10,
  retry: { attempts: 5, delayMs: 15_000, backoffMultiplier: 2 },
};

const output = await createOutput({ directory, config });

const logger = pino(
  { level: 'info' },
  pino.multistream([
    {
      stream: pretty({
        colorize: process.stdout.isTTY,
        destination: process.stdout,
        sync: true,
      }),
    },
    {
      stream: pino.destination({
        dest: output.logPath,
        mkdir: true,
        sync: true,
      }),
    },
  ]),
);

const recordUsage = (operation, model, usage) =>
  logger.info(
    output.recordProviderUsage({ operation, model, usage }),
    'Provider usage recorded',
  );

const wait = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const action = async (
  message,
  callback,
  { attempts = 1, delayMs = 0, backoffMultiplier = 1 } = {},
) => {
  if (!Number.isSafeInteger(attempts) || attempts < 1) {
    throw new TypeError('Action attempts must be a positive safe integer.');
  }
  if (!Number.isSafeInteger(delayMs) || delayMs < 0) {
    throw new TypeError('Action delay must be a non-negative safe integer.');
  }
  if (!Number.isSafeInteger(backoffMultiplier) || backoffMultiplier < 1) {
    throw new TypeError(
      'Action backoff multiplier must be a positive safe integer.',
    );
  }

  logger.info(message);
  let attempt = 1;

  while (true) {
    try {
      return await callback();
    } catch (error) {
      if (attempt === attempts) throw error;
      const nextDelayMs = delayMs * backoffMultiplier ** (attempt - 1);
      logger.warn(
        { action: message, attempt, attempts, nextDelayMs },
        'Action failed; retrying',
      );
      await wait(nextDelayMs);
      attempt += 1;
    }
  }
};

const createProvider = () => {
  const apiKey = process.env.OPENROUTER_API_KEY?.trim();
  if (!apiKey) throw new Error('OPENROUTER_API_KEY is required.');

  return createUnifiedProvider({
    transport: createFetchTransport(),
    apiKey,
    logger,
  });
};

const loadCases = async () => {
  const files = (await readdir(casesDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && extname(entry.name) === '.json')
    .sort((left, right) => left.name.localeCompare(right.name));

  return Promise.all(
    files.map(async ({ name }) =>
      caseSchema.parse(
        JSON.parse(await readFile(join(casesDirectory, name), 'utf8')),
      ),
    ),
  );
};

const loadSkills = async () => {
  const files = (await readdir(skillsDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && extname(entry.name) === '.md')
    .sort((left, right) => left.name.localeCompare(right.name));

  return Promise.all(
    files.map(async ({ name }) => {
      const body = (await readFile(join(skillsDirectory, name), 'utf8')).trim();
      if (!body) throw new Error(`Skill is empty: ${name}`);
      return { name: basename(name, '.md'), body };
    }),
  );
};

const validateClassifications = (cases, skills) => {
  const catalog = new Set(skills.map(({ name }) => name));

  for (const current of cases) {
    const classified = new Set([
      ...current.skills.expected,
      ...current.skills.useful,
      ...Object.keys(current.skills.noise),
    ]);
    const unknown = [...classified].find((name) => !catalog.has(name));
    if (unknown) throw new Error(`Unknown classified skill: ${unknown}`);

    const missing = skills
      .map(({ name }) => name)
      .filter((name) => !classified.has(name));
    if (missing.length > 0) {
      throw new Error(
        `Unclassified skills for ${current.name}: ${missing.join(', ')}`,
      );
    }
  }
};

const completeGoals = async (provider, messages, operation) => {
  const result = await provider.complete({
    model: config.model,
    effort: config.effort,
    messages,
    schema: goalsSchema,
    flags: { sensitiveOutput: true },
  });
  recordUsage(operation, config.model, result.usage);
  return result.structured.goals;
};

const createSkillIndexes = (provider, skills) =>
  action(
    `Indexing ${skills.length} local skills`,
    async () => {
      const lexical = createLexicalIndex({ logger });
      const vector = createVectorIndex({
        dimensions: config.embeddingDimensions,
        logger,
        embedding: async (input) => {
          const result = await provider.embedding({
            model: config.embeddingModel,
            dimensions: config.embeddingDimensions,
            input,
            flags: { sensitiveOutput: true },
          });
          recordUsage('embedding', config.embeddingModel, result.usage);
          return result.embedding;
        },
      });

      for (const skill of skills) {
        await lexical.add(skill, ({ name, body }) => `${name}\n${body}`);
        await vector.add(skill, ({ body }) => body);
      }
      return { lexical, vector };
    },
    config.retry,
  );

const fixedRanking = (results) => ({
  search: (_query, topK) => Promise.resolve(results.slice(0, topK)),
});

const rankResults = (results) =>
  results.map(({ data, score }, index) => ({
    data,
    rank: index + 1,
    score,
  }));

const trace = (results) =>
  results.map(({ data, rank, score }) => ({
    name: data.name,
    rank,
    score,
  }));

const rankSkills = async (provider, indexes, objective, goal) => {
  const query = retrievalQuery(objective, goal);
  const [lexical, vectorScored] = await Promise.all([
    indexes.lexical.search(query, config.retrievalK).then(rankResults),
    indexes.vector.search(query, config.retrievalK).then(rankResults),
  ]);
  const semantic = vectorScored.filter(
    ({ score }) => score >= config.minVectorScore,
  );
  const vector = {
    threshold: config.minVectorScore,
    scored: trace(vectorScored),
    removed: vectorScored
      .filter(({ score }) => score < config.minVectorScore)
      .map(({ data, rank, score }) => ({ name: data.name, rank, score })),
  };
  const hybridSearch = createHybridSearch({
    lexical: fixedRanking(lexical),
    semantic: fixedRanking(semantic),
    key: ({ name }) => name,
    logger,
  });
  const shortlist = rankResults(
    await hybridSearch.search(query, config.retrievalK),
  );
  const hybrid = { scored: trace(shortlist) };
  if (shortlist.length === 0) {
    return {
      lexical: { scored: trace(lexical) },
      vector,
      hybrid,
      reranker: null,
      ranked: [],
    };
  }

  const lexicalByName = new Map(
    lexical.map((entry) => [entry.data.name, entry]),
  );
  const vectorByName = new Map(
    vectorScored.map((entry) => [entry.data.name, entry]),
  );
  const input = {
    query,
    topN: Math.min(config.topK, shortlist.length),
    candidates: shortlist.map(({ data, rank, score }, index) => ({
      index,
      name: data.name,
      hybridRank: rank,
      hybridScore: score,
      lexicalRank: lexicalByName.get(data.name)?.rank ?? null,
      lexicalScore: lexicalByName.get(data.name)?.score ?? null,
      vectorRank: vectorByName.get(data.name)?.rank ?? null,
      vectorScore: vectorByName.get(data.name)?.score ?? null,
    })),
  };
  const result = await provider.rerank({
    model: config.rerankerModel,
    query,
    documents: shortlist.map(({ data }) => data.body),
    topN: input.topN,
    flags: { sensitiveOutput: true },
  });
  recordUsage('rerank', config.rerankerModel, result.usage);

  const reranked = result.results.map(
    ({ index: resultIndex, relevanceScore }, rank) => {
      const skill = shortlist[resultIndex]?.data;
      if (!skill) throw new Error('Reranker returned an unknown skill index.');
      return {
        index: resultIndex,
        skill,
        rank: rank + 1,
        score: relevanceScore,
      };
    },
  );
  const selected = reranked.map(({ skill, rank, score }) => ({
    ...skill,
    rank,
    score,
  }));
  return {
    lexical: { scored: trace(lexical) },
    vector,
    hybrid,
    reranker: {
      input,
      output: reranked.map(({ index, skill, rank, score }) => ({
        index,
        name: skill.name,
        rank,
        score,
      })),
      usage: result.usage ?? null,
    },
    ranked: selected,
  };
};

const retrieveSkills = (provider, index, current, p0) =>
  Promise.all(
    p0.map(async (goal, goalIndex) => {
      const retrieval = await action(
        `${current.name}: querying and reranking goal ${goalIndex + 1}/${p0.length}`,
        () => rankSkills(provider, index, current.objective, goal),
        config.retry,
      );
      logger.info(
        {
          case: current.name,
          goalIndex: goalIndex + 1,
          goal,
          lexical: retrieval.lexical,
          vector: retrieval.vector,
          hybrid: retrieval.hybrid,
          reranker: retrieval.reranker,
          ranked: retrieval.ranked.map(({ name, rank, score }) => ({
            name,
            rank,
            score,
          })),
        },
        'Goal skills reranked',
      );
      return { goal, ...retrieval };
    }),
  );

const evaluateSkill = async (provider, current, goal, skill) => {
  const result = await provider.complete({
    model: config.model,
    effort: config.effort,
    messages: messages(gateSystem, gateUser(current.objective, goal, skill)),
    schema: gateSchema,
    flags: { sensitiveOutput: true },
  });
  recordUsage('gate', config.model, result.usage);

  return { name: skill.name, ...result.structured };
};

const reviseGoals = (provider, current, p0, skills) => {
  if (skills.length === 0) return Promise.resolve([...p0]);

  return completeGoals(
    provider,
    messages(p1System, p1User(current.objective, p0, skills)),
    'p1',
  );
};

const runCase = async (provider, index, current) => {
  const p0 = await action(
    `${current.name}: generating P0`,
    () =>
      completeGoals(
        provider,
        messages(p0System, p0User(current.objective)),
        'p0',
      ),
    config.retry,
  );
  logger.info({ case: current.name, p0 }, 'P0 generated');

  const byGoal = await retrieveSkills(provider, index, current, p0);
  const decisions = await Promise.all(
    byGoal.flatMap(({ goal, ranked }, goalIndex) =>
      ranked.map(async (skill) => ({
        goalIndex: goalIndex + 1,
        goal,
        ...(await action(
          `${current.name}: filtering ${skill.name} for goal ${goalIndex + 1}/${p0.length}`,
          () => evaluateSkill(provider, current, goal, skill),
          config.retry,
        )),
      })),
    ),
  );
  const keptNames = new Set(
    decisions
      .filter(({ decision }) => decision === 'keep')
      .map(({ name }) => name),
  );
  const candidates = [
    ...new Map(
      byGoal.flatMap(({ ranked }) =>
        ranked.map((skill) => [skill.name, skill]),
      ),
    ).values(),
  ];
  const selected = candidates.filter(({ name }) => keptNames.has(name));
  logger.info(
    {
      case: current.name,
      decisions,
      selected: selected.map(({ name }) => name),
    },
    'Goal skills filtered and merged',
  );

  const p1 = await action(
    `${current.name}: generating P1`,
    () => reviseGoals(provider, current, p0, selected),
    config.retry,
  );
  const metrics = scoreBundle(current, candidates, selected);
  const result = {
    name: current.name,
    objective: current.objective,
    p0,
    retrieval: byGoal.map(
      ({ goal, lexical, vector, hybrid, reranker, ranked }) => ({
        goal,
        lexical,
        vector,
        hybrid,
        reranker,
        ranked: ranked.map(({ name, rank, score }) => ({ name, rank, score })),
      }),
    ),
    bundle: { decisions, selected: selected.map(({ name }) => name) },
    p1,
    metrics,
  };
  logger.info({ result }, 'Case completed');
  return result;
};

const main = async () => {
  const [cases, skills] = await Promise.all([loadCases(), loadSkills()]);
  validateClassifications(cases, skills);
  logger.info(
    { runId: output.id, config, identity: output.identity },
    'Run started',
  );

  const provider = createProvider();
  const index = await createSkillIndexes(provider, skills);
  const results = await Promise.all(
    cases.map((current) => runCase(provider, index, current)),
  );
  const metrics = aggregateMetrics(results);
  const providerUsage = output.providerUsage();
  await output.complete(results, metrics);
  logger.info(
    { runId: output.id, cases: results.length, metrics, providerUsage },
    'Run completed',
  );
};

try {
  await main();
} catch (error) {
  logger.error({ err: error }, 'Run failed');
  await output.fail();
  process.exitCode = 1;
}
