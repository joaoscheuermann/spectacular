#!/usr/bin/env node

import { readFile, readdir } from 'node:fs/promises';
import { basename, dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createFetchTransport, createUnifiedProvider } from 'llms';
import pino from 'pino';
import pretty from 'pino-pretty';
import { createVectorIndex } from 'victor';

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

const directory = dirname(fileURLToPath(import.meta.url));
const casesDirectory = join(directory, 'cases');
const skillsDirectory = join(casesDirectory, 'skills');
const logPath = join(directory, 'output.log');

const config = {
  model: 'qwen/qwen3.8-27b',
  effort: 'high',
  embeddingModel: 'voyageai/voyage-4-large',
  embeddingDimensions: 1024,
  rerankerModel: 'voyageai/rerank-2.5',
  retrievalK: 20,
  topK: 10,
};

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
      stream: pino.destination({ dest: logPath, mkdir: true, sync: true }),
    },
  ]),
);

const action = (message, callback) => {
  logger.info(message);
  return callback();
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

const completeGoals = async (provider, messages) => {
  const result = await provider.complete({
    model: config.model,
    effort: config.effort,
    messages,
    schema: goalsSchema,
    flags: { sensitiveOutput: true },
  });
  return result.structured.goals;
};

const createSkillIndex = async (provider, skills) => {
  const index = createVectorIndex({
    dimensions: config.embeddingDimensions,
    logger,
    embedding: (input) =>
      provider.embedding({
        model: config.embeddingModel,
        dimensions: config.embeddingDimensions,
        input,
        flags: { sensitiveOutput: true },
      }),
  });

  await action(`Indexing ${skills.length} local skills`, async () => {
    for (const skill of skills) await index.add(skill, ({ body }) => body);
  });
  return index;
};

const rankSkills = async (provider, index, objective, goal) => {
  const query = retrievalQuery(objective, goal);
  const shortlist = await index.search(query, config.retrievalK);
  const reranked = await provider.rerank({
    model: config.rerankerModel,
    query,
    documents: shortlist.map(({ data }) => data.body),
    topN: Math.min(config.topK, shortlist.length),
    flags: { sensitiveOutput: true },
  });

  return reranked.map(({ index: resultIndex, relevanceScore }, rank) => {
    const skill = shortlist[resultIndex]?.data;
    if (!skill) throw new Error('Reranker returned an unknown skill index.');
    return { ...skill, rank: rank + 1, score: relevanceScore };
  });
};

const retrieveSkills = (provider, index, current, p0) =>
  Promise.all(
    p0.map(async (goal, goalIndex) => {
      const ranked = await action(
        `${current.name}: querying and reranking goal ${goalIndex + 1}/${p0.length}`,
        () => rankSkills(provider, index, current.objective, goal),
      );
      logger.info(
        {
          case: current.name,
          goalIndex: goalIndex + 1,
          goal,
          ranked: ranked.map(({ name, rank, score }) => ({
            name,
            rank,
            score,
          })),
        },
        'Goal skills reranked',
      );
      return { goal, ranked };
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

  return { name: skill.name, ...result.structured };
};

const reviseGoals = (provider, current, p0, skills) => {
  if (skills.length === 0) return Promise.resolve([...p0]);

  return completeGoals(
    provider,
    messages(p1System, p1User(current.objective, p0, skills)),
  );
};

const runCase = async (provider, index, current) => {
  const p0 = await action(`${current.name}: generating P0`, () =>
    completeGoals(provider, messages(p0System, p0User(current.objective))),
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

  const p1 = await action(`${current.name}: generating P1`, () =>
    reviseGoals(provider, current, p0, selected),
  );
  const selectedExpected = current.skills.expected.filter((name) =>
    keptNames.has(name),
  );
  const metrics = {
    selectedExpected: selectedExpected.length,
    expected: current.skills.expected.length,
    coverage: selectedExpected.length / current.skills.expected.length,
    missing: current.skills.expected.filter((name) => !keptNames.has(name)),
  };
  const result = {
    name: current.name,
    objective: current.objective,
    p0,
    retrieval: byGoal.map(({ goal, ranked }) => ({
      goal,
      ranked: ranked.map(({ name, rank, score }) => ({ name, rank, score })),
    })),
    bundle: { decisions, selected: selected.map(({ name }) => name) },
    p1,
    metrics,
  };
  logger.info({ result }, 'Case completed');
  return result;
};

const main = async () => {
  const [cases, skills] = await Promise.all([loadCases(), loadSkills()]);
  const names = new Set(skills.map(({ name }) => name));
  for (const current of cases) {
    for (const expected of current.skills.expected) {
      if (!names.has(expected)) {
        throw new Error(`Unknown expected skill: ${expected}`);
      }
    }
  }

  const provider = createProvider();
  const index = await createSkillIndex(provider, skills);
  const results = await Promise.all(
    cases.map((current) => runCase(provider, index, current)),
  );
  const selectedExpected = results.reduce(
    (total, result) => total + result.metrics.selectedExpected,
    0,
  );
  const expected = results.reduce(
    (total, result) => total + result.metrics.expected,
    0,
  );
  logger.info(
    {
      cases: results.length,
      selectedExpected,
      expected,
      coverage: selectedExpected / expected,
    },
    'Run completed',
  );
};

try {
  await main();
} catch (error) {
  logger.error({ err: error }, 'Run failed');
  process.exitCode = 1;
}
