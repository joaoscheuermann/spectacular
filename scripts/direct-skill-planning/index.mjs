#!/usr/bin/env node

import { relative, resolve } from 'node:path';
import { parseArgs } from 'node:util';

import {
  cancel,
  intro,
  isCancel,
  log,
  note,
  outro,
  select,
  text,
} from '@clack/prompts';
import { createFetchTransport, createUnifiedProvider } from 'llms';
import pino from 'pino';

import { expectedFor, loadCatalog, selectCases, tasksFor } from './case.mjs';
import { orientations, pairLabel, pairs, summarize } from './comparison.mjs';
import { compareWithJudge } from './judging.mjs';
import { printResults, retrievalTrace, showComparison } from './output.mjs';
import { generatePlans, indexSkills } from './planning.mjs';
import {
  providerAttempts,
  providerFailure,
  providerRetryDelaysMs,
} from './retry.mjs';
import { createCheckpointWriter, createRun, saveRun } from './state.mjs';

const defaults = {
  model: 'deepseek/deepseek-v4-pro',
  planningEffort: 'high',
  judgeModels: ['google/gemini-3.7-flash', 'openai/gpt-5.6-sol'],
  judgeEffort: 'medium',
  embeddingModel: 'voyageai/voyage-4-large',
  embeddingDimensions: 1024,
  rerankerModel: 'voyageai/rerank-2.5',
  rounds: 3,
  retrievalK: 20,
  topK: 10,
  minRerankerScore: 0.3,
};

const command = 'npm run llm:direct-skill-planning';
const checkpointDirectory = '.llm-lab/direct-skill-planning/runs';
const help = `Usage:
  ${command}
  ${command} -- --judge
  ${command} -- --judge --case "production migration"

Environment:
  LLM_LAB_ROUNDS       Positive integer (default: ${defaults.rounds})
  LLM_LAB_TOP_K        Positive integer (default: ${defaults.topK})
  LLM_LAB_MIN_RERANKER_SCORE  Non-negative number (default: ${defaults.minRerankerScore})
  LLM_LAB_MODEL        Planning model (default: ${defaults.model})
  LLM_LAB_JUDGE_MODEL  Primary judge model (default: ${defaults.judgeModels[0]})

Judges:
  Primary: ${defaults.judgeModels[0]}
  Secondary: ${defaults.judgeModels[1]}

Retrieval:
  Embeddings: ${defaults.embeddingModel}
  Vector search: victor, shortlist=min(catalog, ${defaults.retrievalK})
  Reranker: ${defaults.rerankerModel}, top-${defaults.topK}, score>=${defaults.minRerankerScore}

Reasoning:
  Planning: ${defaults.planningEffort}
  Judge: ${defaults.judgeEffort}

Reliability:
  Provider operations: ${providerAttempts} total attempts
  Retry delays: ${providerRetryDelaysMs.map((delay) => `${delay / 1_000}s`).join(', ')}`;

const action = (message, callback) => {
  log.step(message);
  return callback();
};

const configuredInteger = (name, fallback) => {
  const value = Number(process.env[name] ?? fallback);
  if (Number.isSafeInteger(value) && value > 0) return value;
  throw new Error(`${name} must be a positive integer.`);
};

const configuredModel = (name, fallback) =>
  process.env[name]?.trim() || fallback;

const configuredScore = (name, fallback) => {
  const value = Number(process.env[name] ?? fallback);
  if (Number.isFinite(value) && value >= 0) return value;
  throw new Error(`${name} must be a non-negative number.`);
};

const configuration = (catalog) => {
  const topK = configuredInteger('LLM_LAB_TOP_K', defaults.topK);
  const retrievalK = Math.min(catalog.length, defaults.retrievalK);
  if (topK > retrievalK) {
    throw new Error('LLM_LAB_TOP_K cannot exceed the vector shortlist size.');
  }
  const judgeModels = [
    configuredModel('LLM_LAB_JUDGE_MODEL', defaults.judgeModels[0]),
    defaults.judgeModels[1],
  ];
  if (new Set(judgeModels).size !== judgeModels.length) {
    throw new Error('Judge models must be unique.');
  }

  return {
    ...defaults,
    model: configuredModel('LLM_LAB_MODEL', defaults.model),
    judgeModels,
    rounds: configuredInteger('LLM_LAB_ROUNDS', defaults.rounds),
    topK,
    minRerankerScore: configuredScore(
      'LLM_LAB_MIN_RERANKER_SCORE',
      defaults.minRerankerScore,
    ),
    retrievalK,
  };
};

const createProvider = (logger) => {
  const apiKey = process.env.OPENROUTER_API_KEY?.trim();
  if (!apiKey) throw new Error('OPENROUTER_API_KEY is required.');

  return createUnifiedProvider({
    transport: createFetchTransport(),
    apiKey,
    logger,
  });
};

const askJudgment = async ({
  current,
  round,
  rounds,
  pair,
  orientation,
  options,
}) => {
  showComparison(current, round, rounds, pair, orientation, options);
  const choice = await select({
    message:
      'Which option correctly integrates relevant evidence while preserving the user objective?',
    options: [
      { value: 'a', label: 'Option A' },
      { value: 'b', label: 'Option B' },
      { value: 'both', label: 'Both integrate correctly' },
      { value: 'neither', label: 'Neither integrates correctly' },
    ],
  });
  if (isCancel(choice)) return null;

  const rationale = await text({
    message: 'Rationale',
    placeholder: 'Optional',
  });
  if (isCancel(rationale)) return null;
  return { choice, rationale };
};

const compare = async ({
  deps,
  current,
  round,
  mode,
  plans,
  pair,
  expectedSkills,
}) => {
  const options = orientations(plans, pair);

  if (mode === 'judge') {
    return Promise.all(
      deps.config.judgeModels.map((judgeModel) =>
        compareWithJudge({
          deps,
          current,
          round,
          pair,
          expectedSkills,
          options,
          judgeModel,
        }),
      ),
    );
  }

  const responses = [];
  for (const [index, currentOptions] of options.entries()) {
    const response = await askJudgment({
      current,
      round,
      rounds: deps.config.rounds,
      pair,
      orientation: index + 1,
      options: currentOptions,
    });
    if (response === null) return null;
    responses.push(response);
  }

  return [
    {
      judgeModel: null,
      status: 'completed',
      ...summarize(pair, options, responses),
    },
  ];
};

const evaluate = async (deps, current, round, mode) => {
  const plans = await generatePlans(deps, current, round);
  const expectedSkills = expectedFor(deps.catalog, current);
  let comparisons;

  if (mode === 'judge') {
    const comparisonsByPair = await Promise.all(
      pairs.map((pair) =>
        compare({
          deps,
          current,
          round,
          mode,
          plans,
          pair,
          expectedSkills,
        }),
      ),
    );
    comparisons = comparisonsByPair.flat();
  } else {
    comparisons = [];
    for (const pair of pairs) {
      const pairComparisons = await compare({
        deps,
        current,
        round,
        mode,
        plans,
        pair,
        expectedSkills,
      });
      if (pairComparisons === null) return null;
      comparisons.push(...pairComparisons);
    }
  }

  return {
    name: current.name,
    round,
    p0: plans.p0,
    direct: plans.direct,
    directGoal: plans.directGoal,
    requestP1: plans.requestP1,
    goalP1: plans.goalP1,
    comparisons,
    retrieval: retrievalTrace(plans),
  };
};

const main = async () => {
  const { values } = parseArgs({
    options: {
      case: { type: 'string' },
      help: { type: 'boolean', short: 'h' },
      judge: { type: 'boolean' },
    },
    strict: true,
  });
  if (values.help === true) {
    console.log(help);
    return;
  }

  const catalog = await loadCatalog();
  const config = configuration(catalog);
  const selected = selectCases(values.case);
  const mode = values.judge === true ? 'judge' : 'human';
  let run = createRun({
    mode,
    model: config.model,
    planningEffort: config.planningEffort,
    judgeModels: config.judgeModels,
    judgeEffort: config.judgeEffort,
    embeddingModel: config.embeddingModel,
    embeddingDimensions: config.embeddingDimensions,
    rerankerModel: config.rerankerModel,
    rounds: config.rounds,
    retrievalK: config.retrievalK,
    topK: config.topK,
    minRerankerScore: config.minRerankerScore,
    providerAttempts,
    providerRetryDelaysMs,
    comparisonPairs: pairs,
    catalog: catalog.map(({ name, source }) => ({
      name,
      source: source ?? 'local',
    })),
    cases: selected,
  });
  const path = resolve(checkpointDirectory, `${run.id}.json`);
  const displayPath = relative(process.cwd(), path) || path;
  const checkpoint = createCheckpointWriter(path, run);
  const onProviderFailure = async (failure) => {
    const event = await checkpoint.recordProviderFailure(failure);
    log.warn(`Provider attempt failed: ${JSON.stringify(event)}`);
  };

  await saveRun(path, run);
  intro(
    mode === 'judge'
      ? `Direct skill planning judges · ${config.judgeModels.join(' + ')}`
      : 'Direct skill planning: Direct × Direct Goal × Request P1 × Goal P1',
  );
  note(`${displayPath}\n0 evaluations saved.`, 'Run checkpoint');

  try {
    const logger = pino({
      enabled: true,
      transport: { target: 'pino-pretty' },
    });
    const provider = createProvider(logger);
    const vectors = await action('Indexing skill catalog', () =>
      indexSkills(provider, config, logger, catalog, onProviderFailure),
    );
    const deps = {
      provider,
      vectors,
      config,
      catalog,
      action,
      onProviderFailure,
    };
    const tasks = tasksFor(selected, config.rounds);

    if (mode === 'judge') {
      const evaluations = await Promise.allSettled(
        tasks.map(async ({ current, round }, index) => {
          log.info(`${current.name}: round ${round}/${config.rounds}`);
          const result = await evaluate(deps, current, round, mode);
          await checkpoint.saveResult(index, result);
          return result;
        }),
      );
      const results = evaluations.flatMap((evaluation) =>
        evaluation.status === 'fulfilled' ? [evaluation.value] : [],
      );
      const failure = evaluations.find(
        (evaluation) => evaluation.status === 'rejected',
      );
      if (failure !== undefined) throw failure.reason;
      const hasJudgeFailures = results.some((result) =>
        result.comparisons.some(
          (comparison) => comparison.status !== 'completed',
        ),
      );
      run = await checkpoint.finish(
        hasJudgeFailures ? 'completed_with_failures' : 'completed',
      );
    } else {
      for (const [index, { current, round }] of tasks.entries()) {
        log.info(`${current.name}: round ${round}/${config.rounds}`);
        const result = await evaluate(deps, current, round, mode);
        if (result === null) {
          run = await checkpoint.finish('paused');
          cancel(`Run paused. Results saved to ${displayPath}.`);
          return;
        }
        await checkpoint.saveResult(index, result);
      }
      run = await checkpoint.finish('completed');
    }

    printResults(run);
    outro(
      run.status === 'completed'
        ? `Run completed. Results saved to ${displayPath}.`
        : `Run completed with judge failures. Results saved to ${displayPath}.`,
    );
  } catch (error) {
    const failure = providerFailure(error) ?? { code: 'unexpected_error' };
    run = await checkpoint.finish('paused', failure);
    cancel(
      `Run paused after ${JSON.stringify(failure)}. Results saved to ${displayPath}.`,
    );
    process.exitCode = 1;
  }
};

await main();
